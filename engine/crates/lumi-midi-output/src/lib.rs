//! Provider-neutral MIDI source control and fail-silent SoundSwitch sequencing.

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const MIDI_SOURCE_NAME: &str = "Lumi Virtual MIDI";
pub const MIDI_CLOCK_SOURCE_NAME: &str = "Lumi Clock";
pub const MIDI_CHANNEL: u8 = 16;
const MIDI_CHANNEL_ZERO_BASED: u8 = MIDI_CHANNEL - 1;
const BANK_NOTE_BASE: u8 = 60;
const AUTOLOOP_NOTE_BASE: u8 = 64;
pub const BANK_SETTLE_DELAY: Duration = Duration::from_millis(50);
const REALTIME_LATE_DISPATCH_THRESHOLD: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiAddressKind {
    Bank,
    Autoloop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiAddress {
    kind: MidiAddressKind,
    number: u8,
    note: u8,
}

impl MidiAddress {
    pub const BANK_ONE: Self = Self {
        kind: MidiAddressKind::Bank,
        number: 1,
        note: BANK_NOTE_BASE,
    };

    pub const fn bank(number: u8) -> Option<Self> {
        if number >= 1 && number <= 4 {
            Some(Self {
                kind: MidiAddressKind::Bank,
                number,
                note: BANK_NOTE_BASE + number - 1,
            })
        } else {
            None
        }
    }

    pub const fn autoloop(number: u8) -> Option<Self> {
        if number >= 1 && number <= 32 {
            Some(Self {
                kind: MidiAddressKind::Autoloop,
                number,
                note: AUTOLOOP_NOTE_BASE + number - 1,
            })
        } else {
            None
        }
    }

    pub const fn kind(self) -> MidiAddressKind {
        self.kind
    }

    pub const fn number(self) -> u8 {
        self.number
    }

    pub const fn note(self) -> u8 {
        self.note
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiMessage {
    bytes: [u8; 3],
    length: u8,
}

impl MidiMessage {
    pub const fn note_on(channel: u8, note: u8, velocity: u8) -> Option<Self> {
        if channel < 16 && note < 128 && velocity < 128 {
            Some(Self {
                bytes: [0x90 | channel, note, velocity],
                length: 3,
            })
        } else {
            None
        }
    }

    pub const fn note_off(channel: u8, note: u8) -> Option<Self> {
        if channel < 16 && note < 128 {
            Some(Self {
                bytes: [0x80 | channel, note, 0],
                length: 3,
            })
        } else {
            None
        }
    }

    pub const fn bytes(self) -> [u8; 3] {
        self.bytes
    }

    pub const fn length(self) -> u8 {
        self.length
    }

    pub const fn clock() -> Self {
        Self::system_realtime(0xf8)
    }

    pub const fn start() -> Self {
        Self::system_realtime(0xfa)
    }

    pub const fn continue_playback() -> Self {
        Self::system_realtime(0xfb)
    }

    pub const fn stop() -> Self {
        Self::system_realtime(0xfc)
    }

    pub const fn song_position(position_16th: u16) -> Option<Self> {
        if position_16th < 16_384 {
            Some(Self {
                bytes: [
                    0xf2,
                    (position_16th & 0x7f) as u8,
                    ((position_16th >> 7) & 0x7f) as u8,
                ],
                length: 3,
            })
        } else {
            None
        }
    }

    const fn system_realtime(status: u8) -> Self {
        Self {
            bytes: [status, 0, 0],
            length: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiClockState {
    Stopped,
    Ready,
    Running,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidiClockStatus {
    pub state: MidiClockState,
    pub source_name: &'static str,
    pub bpm_milli: Option<u32>,
    pub sent_tick_count: u64,
    pub sent_transport_count: u64,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
}

impl Default for MidiClockStatus {
    fn default() -> Self {
        Self {
            state: MidiClockState::Stopped,
            source_name: MIDI_CLOCK_SOURCE_NAME,
            bpm_milli: None,
            sent_tick_count: 0,
            sent_transport_count: 0,
            last_event: None,
            last_error: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiClockSync {
    pub bpm_milli: u32,
    pub playing: bool,
    pub song_position_16th: u16,
    pub delay_to_next_tick: Duration,
    pub rephase: bool,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum MidiClockError {
    #[error("the MIDI clock worker is unavailable")]
    WorkerUnavailable,
    #[error("the MIDI clock provider failed: {0}")]
    Provider(String),
    #[error("the MIDI clock BPM is outside the supported range")]
    InvalidTempo,
    #[error("the MIDI clock song position exceeds the MIDI 1.0 range")]
    InvalidSongPosition,
}

enum ClockWorkerCommand {
    Publish(mpsc::Sender<Result<(), String>>),
    Stop(mpsc::Sender<()>),
    Synchronize(MidiClockSync),
    Shutdown,
}

/// Owns a dedicated MIDI Clock source and drift-correcting worker. The worker
/// is independent from UI rendering and receives only authoritative transport
/// anchors from the engine.
pub struct MidiClockController<P>
where
    P: MidiSourceProvider + 'static,
{
    commands: mpsc::SyncSender<ClockWorkerCommand>,
    worker: Option<JoinHandle<()>>,
    status: Arc<Mutex<MidiClockStatus>>,
    last_sync: Option<MidiClockSync>,
    provider: PhantomData<fn() -> P>,
}

impl<P> MidiClockController<P>
where
    P: MidiSourceProvider + 'static,
{
    pub fn new(factory: impl FnOnce() -> P + Send + 'static) -> Self {
        let (commands, receiver) = mpsc::sync_channel(16);
        let status = Arc::new(Mutex::new(MidiClockStatus::default()));
        let worker_status = Arc::clone(&status);
        let worker = thread::Builder::new()
            .name("lumi-midi-clock".to_owned())
            .spawn(move || {
                let provider = factory();
                run_clock_worker(provider, receiver, &worker_status);
            })
            .ok();
        Self {
            commands,
            worker,
            status,
            last_sync: None,
            provider: PhantomData,
        }
    }

    pub fn publish(&mut self) -> Result<(), MidiClockError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(ClockWorkerCommand::Publish(reply))
            .map_err(|_| MidiClockError::WorkerUnavailable)?;
        let result = response
            .recv()
            .map_err(|_| MidiClockError::WorkerUnavailable)?
            .map_err(MidiClockError::Provider);
        if result.is_ok() {
            self.last_sync = None;
        }
        result
    }

    pub fn stop(&mut self) -> Result<(), MidiClockError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(ClockWorkerCommand::Stop(reply))
            .map_err(|_| MidiClockError::WorkerUnavailable)?;
        response
            .recv()
            .map_err(|_| MidiClockError::WorkerUnavailable)?;
        self.last_sync = None;
        Ok(())
    }

    pub fn synchronize(&mut self, mut sync: MidiClockSync) -> Result<(), MidiClockError> {
        if !(20_000..=300_000).contains(&sync.bpm_milli) {
            return Err(MidiClockError::InvalidTempo);
        }
        if sync.song_position_16th >= 16_384 {
            return Err(MidiClockError::InvalidSongPosition);
        }
        let transport_changed = self
            .last_sync
            .is_none_or(|previous| previous.playing != sync.playing);
        let tempo_changed = self
            .last_sync
            .is_none_or(|previous| previous.bpm_milli != sync.bpm_milli);
        if sync.playing && transport_changed {
            sync.rephase = true;
        }
        if !(transport_changed || tempo_changed || sync.rephase) {
            self.last_sync = Some(sync);
            return Ok(());
        }
        self.commands
            .send(ClockWorkerCommand::Synchronize(sync))
            .map_err(|_| MidiClockError::WorkerUnavailable)?;
        self.last_sync = Some(sync);
        Ok(())
    }

    pub fn status(&self) -> MidiClockStatus {
        self.status
            .lock()
            .map_or_else(|_| MidiClockStatus::default(), |status| status.clone())
    }
}

impl<P> Drop for MidiClockController<P>
where
    P: MidiSourceProvider + 'static,
{
    fn drop(&mut self) {
        let _ = self.commands.send(ClockWorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_clock_worker<P>(
    mut provider: P,
    receiver: mpsc::Receiver<ClockWorkerCommand>,
    shared_status: &Arc<Mutex<MidiClockStatus>>,
) where
    P: MidiSourceProvider,
{
    let mut published = false;
    let mut playing = false;
    let mut tick_interval = Duration::from_millis(20);
    let mut next_tick: Option<Instant> = None;

    loop {
        let command = if let Some(deadline) = next_tick.filter(|_| published && playing) {
            receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        } else {
            receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match command {
            Ok(ClockWorkerCommand::Publish(reply)) => {
                let result = if published {
                    Ok(())
                } else {
                    provider
                        .publish(MIDI_CLOCK_SOURCE_NAME)
                        .map_err(|error| error.to_string())
                };
                if result.is_ok() {
                    published = true;
                    update_clock_status(shared_status, |status| {
                        status.state = MidiClockState::Ready;
                        status.last_event = Some(
                            "Lumi Clock published; waiting for Live Local Playback".to_owned(),
                        );
                        status.last_error = None;
                    });
                } else if let Err(error) = &result {
                    update_clock_status(shared_status, |status| {
                        status.last_error = Some(error.clone());
                    });
                }
                let _ = reply.send(result);
            }
            Ok(ClockWorkerCommand::Stop(reply)) => {
                if published && playing {
                    let _ = provider.send(&[MidiMessage::stop()]);
                }
                provider.stop();
                published = false;
                playing = false;
                next_tick = None;
                update_clock_status(shared_status, |status| {
                    status.state = MidiClockState::Stopped;
                    status.bpm_milli = None;
                    status.last_event = Some("Lumi Clock stopped".to_owned());
                });
                let _ = reply.send(());
            }
            Ok(ClockWorkerCommand::Synchronize(sync)) => {
                if !published {
                    continue;
                }
                tick_interval = midi_clock_tick_interval(sync.bpm_milli);
                if !sync.playing {
                    if playing && provider.send(&[MidiMessage::stop()]).is_ok() {
                        increment_transport_count(shared_status);
                    }
                    playing = false;
                    next_tick = None;
                    update_clock_status(shared_status, |status| {
                        status.state = MidiClockState::Ready;
                        status.bpm_milli = Some(sync.bpm_milli);
                        status.last_event = Some("Local Playback clock paused".to_owned());
                    });
                    continue;
                }
                if !playing || sync.rephase {
                    let transport_result = if sync.song_position_16th == 0 {
                        provider.send(&[MidiMessage::start()])
                    } else if let Some(position) =
                        MidiMessage::song_position(sync.song_position_16th)
                    {
                        provider.send(&[position, MidiMessage::continue_playback()])
                    } else {
                        continue;
                    };
                    if let Err(error) = transport_result {
                        update_clock_status(shared_status, |status| {
                            status.last_error = Some(error.to_string());
                        });
                        playing = false;
                        next_tick = None;
                        continue;
                    }
                    increment_transport_count(shared_status);
                    playing = true;
                    next_tick = Some(Instant::now() + sync.delay_to_next_tick);
                }
                update_clock_status(shared_status, |status| {
                    status.state = MidiClockState::Running;
                    status.bpm_milli = Some(sync.bpm_milli);
                    status.last_event = Some(format!(
                        "Clock running at {:.3} BPM · song position {}",
                        f64::from(sync.bpm_milli) / 1_000.0,
                        sync.song_position_16th
                    ));
                    status.last_error = None;
                });
            }
            Ok(ClockWorkerCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                if published && playing {
                    let _ = provider.send(&[MidiMessage::stop()]);
                }
                provider.stop();
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Err(error) = provider.send(&[MidiMessage::clock()]) {
                    provider.stop();
                    published = false;
                    update_clock_status(shared_status, |status| {
                        status.state = MidiClockState::Stopped;
                        status.last_event = Some("Lumi Clock failed closed".to_owned());
                        status.last_error =
                            Some(format!("MIDI Clock tick could not be sent: {error}"));
                    });
                    playing = false;
                    next_tick = None;
                    continue;
                }
                update_clock_status(shared_status, |status| {
                    status.sent_tick_count = status.sent_tick_count.saturating_add(1);
                });
                let now = Instant::now();
                let mut following = next_tick.unwrap_or(now) + tick_interval;
                while following <= now {
                    following += tick_interval;
                }
                next_tick = Some(following);
            }
        }
    }
}

fn midi_clock_tick_interval(bpm_milli: u32) -> Duration {
    let nanos = 60_000_000_000_000_u64 / (u64::from(bpm_milli) * 24);
    Duration::from_nanos(nanos.max(1))
}

fn update_clock_status(
    shared_status: &Arc<Mutex<MidiClockStatus>>,
    update: impl FnOnce(&mut MidiClockStatus),
) {
    if let Ok(mut status) = shared_status.lock() {
        update(&mut status);
    }
}

fn increment_transport_count(shared_status: &Arc<Mutex<MidiClockStatus>>) {
    update_clock_status(shared_status, |status| {
        status.sent_transport_count = status.sent_transport_count.saturating_add(1);
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidiSourceStatus {
    pub state: MidiSourceState,
    pub source_name: &'static str,
    pub sent_pulse_count: u64,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
    pub active_bank: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiSourceState {
    Stopped,
    Ready,
}

const REALTIME_COMMAND_CAPACITY: usize = 64;
const REALTIME_SCHEDULE_CAPACITY: usize = 128;
const REALTIME_LATENCY_SAMPLE_CAPACITY: usize = 2_048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealtimeMidiStatus {
    pub source: MidiSourceStatus,
    pub queue_capacity: usize,
    pub queue_depth: usize,
    pub queue_high_water: usize,
    pub scheduled_count: u64,
    pub emitted_count: u64,
    pub cancelled_count: u64,
    pub saturation_count: u64,
    pub latency_sample_count: usize,
    pub latency_p50_micros: u64,
    pub latency_p95_micros: u64,
    pub latency_p99_micros: u64,
    pub latency_max_micros: u64,
    pub last_scheduled_action: Option<RealtimeMidiActionKind>,
    pub last_scheduled_number: Option<u8>,
    pub last_scheduled_lead_micros: Option<u64>,
    pub last_emitted_action: Option<RealtimeMidiActionKind>,
    pub last_emitted_number: Option<u8>,
    pub last_dispatch_lateness_micros: Option<u64>,
    pub late_dispatch_count: u64,
}

impl Default for RealtimeMidiStatus {
    fn default() -> Self {
        Self {
            source: MidiSourceStatus {
                state: MidiSourceState::Stopped,
                source_name: MIDI_SOURCE_NAME,
                sent_pulse_count: 0,
                last_event: None,
                last_error: None,
                active_bank: None,
            },
            queue_capacity: REALTIME_COMMAND_CAPACITY,
            queue_depth: 0,
            queue_high_water: 0,
            scheduled_count: 0,
            emitted_count: 0,
            cancelled_count: 0,
            saturation_count: 0,
            latency_sample_count: 0,
            latency_p50_micros: 0,
            latency_p95_micros: 0,
            latency_p99_micros: 0,
            latency_max_micros: 0,
            last_scheduled_action: None,
            last_scheduled_number: None,
            last_scheduled_lead_micros: None,
            last_emitted_action: None,
            last_emitted_number: None,
            last_dispatch_lateness_micros: None,
            late_dispatch_count: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealtimeMidiActionKind {
    Bank,
    Autoloop,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum RealtimeMidiError {
    #[error("the realtime MIDI lane is unavailable")]
    WorkerUnavailable,
    #[error("the realtime MIDI lane is saturated")]
    QueueSaturated,
    #[error("the MIDI provider failed: {0}")]
    Provider(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RealtimeMidiAction {
    SelectBank(u8),
    TriggerAutoloop(u8),
}

#[derive(Clone, Copy, Debug)]
struct ScheduledRealtimeMidiAction {
    generation: u64,
    deadline: Instant,
    action: RealtimeMidiAction,
}

enum RealtimeMidiCommand {
    Publish(mpsc::Sender<Result<(), String>>),
    Stop(mpsc::Sender<()>),
    SendLearnPulse(MidiAddress, mpsc::Sender<Result<(), String>>),
    SelectBank(u8, mpsc::Sender<Result<(), String>>),
    TriggerAutoloop(u8, mpsc::Sender<Result<(), String>>),
    TriggerSequence(u8, u8, mpsc::Sender<Result<(), String>>),
    SetGeneration(u64),
    Schedule(ScheduledRealtimeMidiAction),
    CancelAll,
    Shutdown,
}

/// Dedicated bounded execution lane for sparse, show-critical Bank and
/// AutoLoop messages. The provider is constructed and owned on this lane's
/// thread, so CoreMIDI calls and deadline waits never run on the engine command,
/// SQLite or UI snapshot path.
pub struct RealtimeMidiController<P>
where
    P: MidiSourceProvider + 'static,
{
    commands: mpsc::SyncSender<RealtimeMidiCommand>,
    worker: Option<JoinHandle<()>>,
    status: Arc<Mutex<RealtimeMidiStatus>>,
    provider: PhantomData<fn() -> P>,
}

impl<P> RealtimeMidiController<P>
where
    P: MidiSourceProvider + 'static,
{
    pub fn new(factory: impl FnOnce() -> P + Send + 'static) -> Self {
        let (commands, receiver) = mpsc::sync_channel(REALTIME_COMMAND_CAPACITY);
        let status = Arc::new(Mutex::new(RealtimeMidiStatus::default()));
        let worker_status = Arc::clone(&status);
        let worker = thread::Builder::new()
            .name("lumi-realtime-midi".to_owned())
            .spawn(move || run_realtime_midi_worker(factory(), receiver, &worker_status))
            .ok();
        Self {
            commands,
            worker,
            status,
            provider: PhantomData,
        }
    }

    pub fn publish(&self) -> Result<(), RealtimeMidiError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(RealtimeMidiCommand::Publish(reply))
            .map_err(|_| RealtimeMidiError::WorkerUnavailable)?;
        response
            .recv()
            .map_err(|_| RealtimeMidiError::WorkerUnavailable)?
            .map_err(RealtimeMidiError::Provider)
    }

    pub fn stop(&self) {
        let (reply, response) = mpsc::channel();
        if self.commands.send(RealtimeMidiCommand::Stop(reply)).is_ok() {
            let _ = response.recv();
        }
    }

    pub fn send_learn_pulse(&self) -> Result<(), RealtimeMidiError> {
        self.send_address_learn_pulse(MidiAddress::BANK_ONE)
    }

    pub fn send_address_learn_pulse(&self, address: MidiAddress) -> Result<(), RealtimeMidiError> {
        self.synchronous(|reply| RealtimeMidiCommand::SendLearnPulse(address, reply))
    }

    pub fn select_bank(&self, bank_number: u8) -> Result<(), RealtimeMidiError> {
        self.synchronous(|reply| RealtimeMidiCommand::SelectBank(bank_number, reply))
    }

    pub fn trigger_autoloop_button(&self, autoloop_number: u8) -> Result<(), RealtimeMidiError> {
        self.synchronous(|reply| RealtimeMidiCommand::TriggerAutoloop(autoloop_number, reply))
    }

    pub fn trigger_autoloop(
        &self,
        bank_number: u8,
        autoloop_number: u8,
    ) -> Result<(), RealtimeMidiError> {
        self.synchronous(|reply| {
            RealtimeMidiCommand::TriggerSequence(bank_number, autoloop_number, reply)
        })
    }

    pub fn set_generation(&self, generation: u64) -> Result<(), RealtimeMidiError> {
        self.try_send(RealtimeMidiCommand::SetGeneration(generation))
    }

    pub fn schedule_bank(
        &self,
        generation: u64,
        bank_number: u8,
        deadline: Instant,
    ) -> Result<(), RealtimeMidiError> {
        self.schedule(
            generation,
            deadline,
            RealtimeMidiAction::SelectBank(bank_number),
        )
    }

    pub fn schedule_autoloop(
        &self,
        generation: u64,
        autoloop_number: u8,
        deadline: Instant,
    ) -> Result<(), RealtimeMidiError> {
        self.schedule(
            generation,
            deadline,
            RealtimeMidiAction::TriggerAutoloop(autoloop_number),
        )
    }

    pub fn cancel_all(&self) -> Result<(), RealtimeMidiError> {
        self.try_send(RealtimeMidiCommand::CancelAll)
    }

    pub fn status(&self) -> RealtimeMidiStatus {
        self.status
            .lock()
            .map_or_else(|_| RealtimeMidiStatus::default(), |status| status.clone())
    }

    fn synchronous(
        &self,
        command: impl FnOnce(mpsc::Sender<Result<(), String>>) -> RealtimeMidiCommand,
    ) -> Result<(), RealtimeMidiError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(command(reply))
            .map_err(|_| RealtimeMidiError::WorkerUnavailable)?;
        response
            .recv()
            .map_err(|_| RealtimeMidiError::WorkerUnavailable)?
            .map_err(RealtimeMidiError::Provider)
    }

    fn schedule(
        &self,
        generation: u64,
        deadline: Instant,
        action: RealtimeMidiAction,
    ) -> Result<(), RealtimeMidiError> {
        self.try_send(RealtimeMidiCommand::Schedule(ScheduledRealtimeMidiAction {
            generation,
            deadline,
            action,
        }))
    }

    fn try_send(&self, command: RealtimeMidiCommand) -> Result<(), RealtimeMidiError> {
        // Reserve the diagnostic queue slot before publishing the command.
        // Incrementing after `try_send` races the worker's decrement and can
        // leave a false non-zero depth forever when the worker is faster than
        // the calling thread.
        update_realtime_status(&self.status, |status| {
            status.queue_depth = status.queue_depth.saturating_add(1);
            status.queue_high_water = status.queue_high_water.max(status.queue_depth);
        });
        match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                update_realtime_status(&self.status, |status| {
                    status.queue_depth = status.queue_depth.saturating_sub(1);
                    status.saturation_count = status.saturation_count.saturating_add(1);
                    status.source.last_error =
                        Some("Realtime MIDI lane saturated; output failed closed".to_owned());
                });
                Err(RealtimeMidiError::QueueSaturated)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                update_realtime_status(&self.status, |status| {
                    status.queue_depth = status.queue_depth.saturating_sub(1);
                });
                Err(RealtimeMidiError::WorkerUnavailable)
            }
        }
    }
}

impl<P> Drop for RealtimeMidiController<P>
where
    P: MidiSourceProvider + 'static,
{
    fn drop(&mut self) {
        let _ = self.commands.send(RealtimeMidiCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub trait MidiSourceProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn publish(&mut self, source_name: &str) -> Result<(), Self::Error>;
    fn stop(&mut self);
    fn send(&mut self, messages: &[MidiMessage]) -> Result<(), Self::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum MidiOutputError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error("the virtual MIDI source is not published")]
    SourceNotPublished,
    #[error("the MIDI pulse counter overflowed")]
    PulseCounterOverflow,
    #[error("the requested bank or AutoLoop address is invalid")]
    InvalidAddress,
    #[error("the MIDI provider failed: {0}")]
    Provider(E),
}

pub struct MidiOutputController<P>
where
    P: MidiSourceProvider,
{
    provider: P,
    state: MidiSourceState,
    sent_pulse_count: u64,
    last_event: Option<String>,
    last_error: Option<String>,
    active_bank: Option<u8>,
}

impl<P> MidiOutputController<P>
where
    P: MidiSourceProvider,
{
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            state: MidiSourceState::Stopped,
            sent_pulse_count: 0,
            last_event: None,
            last_error: None,
            active_bank: None,
        }
    }

    pub fn publish(&mut self) -> Result<(), MidiOutputError<P::Error>> {
        if let Err(error) = self.provider.publish(MIDI_SOURCE_NAME) {
            self.state = MidiSourceState::Stopped;
            self.active_bank = None;
            self.last_error = Some(error.to_string());
            return Err(MidiOutputError::Provider(error));
        }
        self.state = MidiSourceState::Ready;
        self.active_bank = None;
        self.last_error = None;
        self.last_event = Some("Virtual MIDI source published; no MIDI sent".to_owned());
        Ok(())
    }

    pub fn stop(&mut self) {
        self.provider.stop();
        self.state = MidiSourceState::Stopped;
        self.active_bank = None;
        self.last_event = Some("Virtual MIDI source stopped".to_owned());
    }

    pub fn send_learn_pulse(&mut self) -> Result<(), MidiOutputError<P::Error>> {
        self.send_address_learn_pulse(MidiAddress::BANK_ONE)
    }

    pub fn send_address_learn_pulse(
        &mut self,
        address: MidiAddress,
    ) -> Result<(), MidiOutputError<P::Error>> {
        self.send_address_pulse(address)?;
        let target = match address.kind() {
            MidiAddressKind::Bank => "Bank",
            MidiAddressKind::Autoloop => "AutoLoop",
        };
        self.last_event = Some(format!(
            "{target} {} learn pulse sent · Ch {} · Note {} · Note Off included",
            address.number(),
            MIDI_CHANNEL,
            address.note()
        ));
        Ok(())
    }

    pub fn trigger_autoloop(
        &mut self,
        bank_number: u8,
        autoloop_number: u8,
    ) -> Result<(), MidiOutputError<P::Error>> {
        self.trigger_autoloop_with_wait(bank_number, autoloop_number, std::thread::sleep)
    }

    fn trigger_autoloop_with_wait<F>(
        &mut self,
        bank_number: u8,
        autoloop_number: u8,
        wait: F,
    ) -> Result<(), MidiOutputError<P::Error>>
    where
        F: FnOnce(Duration),
    {
        let bank = MidiAddress::bank(bank_number).ok_or(MidiOutputError::InvalidAddress)?;
        let autoloop =
            MidiAddress::autoloop(autoloop_number).ok_or(MidiOutputError::InvalidAddress)?;
        // Reassert the bank for every phrase. A physical controller can change
        // SoundSwitch's active bank between Lumi cues, so cached selection is
        // not authoritative. The engine schedules this pulse before the phrase.
        self.send_address_pulse(bank)?;
        self.active_bank = Some(bank_number);
        self.last_event = Some(format!(
            "Bank {bank_number} selected · waiting {} ms for SoundSwitch",
            BANK_SETTLE_DELAY.as_millis()
        ));
        wait(BANK_SETTLE_DELAY);
        self.send_address_pulse(autoloop)?;
        self.last_event = Some(format!(
            "Triggered Bank {bank_number} → AutoLoop {autoloop_number} · Ch {MIDI_CHANNEL} · Notes {} → {} · {} ms bank gap",
            bank.note(),
            autoloop.note(),
            BANK_SETTLE_DELAY.as_millis()
        ));
        Ok(())
    }

    /// Selects a SoundSwitch Bank without blocking for its settling interval.
    /// The engine realtime scheduler owns the later AutoLoop deadline.
    pub fn select_bank(&mut self, bank_number: u8) -> Result<(), MidiOutputError<P::Error>> {
        let bank = MidiAddress::bank(bank_number).ok_or(MidiOutputError::InvalidAddress)?;
        self.send_address_pulse(bank)?;
        self.active_bank = Some(bank_number);
        self.last_event = Some(format!(
            "Pre-armed Bank {bank_number} · Ch {MIDI_CHANNEL} · Note {}",
            bank.note()
        ));
        Ok(())
    }

    /// Emits only the AutoLoop button pulse. Callers must prove that the Bank
    /// has already satisfied `BANK_SETTLE_DELAY`.
    pub fn trigger_autoloop_button(
        &mut self,
        autoloop_number: u8,
    ) -> Result<(), MidiOutputError<P::Error>> {
        let autoloop =
            MidiAddress::autoloop(autoloop_number).ok_or(MidiOutputError::InvalidAddress)?;
        self.send_address_pulse(autoloop)?;
        self.last_event = Some(format!(
            "Triggered AutoLoop {autoloop_number} · Ch {MIDI_CHANNEL} · Note {} · Bank pre-armed",
            autoloop.note()
        ));
        Ok(())
    }

    #[must_use]
    pub const fn active_bank(&self) -> Option<u8> {
        self.active_bank
    }

    fn send_address_pulse(
        &mut self,
        address: MidiAddress,
    ) -> Result<(), MidiOutputError<P::Error>> {
        if self.state != MidiSourceState::Ready {
            return Err(MidiOutputError::SourceNotPublished);
        }
        let note_on = MidiMessage::note_on(MIDI_CHANNEL_ZERO_BASED, address.note(), 100)
            .ok_or(MidiOutputError::SourceNotPublished)?;
        let note_off = MidiMessage::note_off(MIDI_CHANNEL_ZERO_BASED, address.note())
            .ok_or(MidiOutputError::SourceNotPublished)?;
        let next_count = self
            .sent_pulse_count
            .checked_add(1)
            .ok_or(MidiOutputError::PulseCounterOverflow)?;
        if let Err(error) = self.provider.send(&[note_on, note_off]) {
            self.provider.stop();
            self.state = MidiSourceState::Stopped;
            self.active_bank = None;
            self.last_error = Some(error.to_string());
            self.last_event = Some("Virtual MIDI source failed closed".to_owned());
            return Err(MidiOutputError::Provider(error));
        }
        self.sent_pulse_count = next_count;
        self.last_error = None;
        Ok(())
    }

    pub fn status(&self) -> MidiSourceStatus {
        MidiSourceStatus {
            state: self.state,
            source_name: MIDI_SOURCE_NAME,
            sent_pulse_count: self.sent_pulse_count,
            last_event: self.last_event.clone(),
            last_error: self.last_error.clone(),
            active_bank: self.active_bank,
        }
    }
}

fn run_realtime_midi_worker<P>(
    provider: P,
    receiver: mpsc::Receiver<RealtimeMidiCommand>,
    shared_status: &Arc<Mutex<RealtimeMidiStatus>>,
) where
    P: MidiSourceProvider,
{
    let mut controller = MidiOutputController::new(provider);
    let mut generation = 0_u64;
    let mut scheduled: Vec<ScheduledRealtimeMidiAction> = Vec::new();
    let mut latencies = VecDeque::with_capacity(REALTIME_LATENCY_SAMPLE_CAPACITY);

    loop {
        let timeout = scheduled
            .iter()
            .filter(|item| item.generation == generation)
            .map(|item| item.deadline)
            .min()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()));
        let command = timeout.map_or_else(
            || {
                receiver
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            },
            |duration| receiver.recv_timeout(duration),
        );
        match command {
            Ok(command) => {
                update_realtime_status(shared_status, |status| {
                    status.queue_depth = status.queue_depth.saturating_sub(1);
                });
                match command {
                    RealtimeMidiCommand::Publish(reply) => {
                        let result = controller.publish().map_err(|error| error.to_string());
                        publish_controller_status(shared_status, &controller);
                        let _ = reply.send(result);
                    }
                    RealtimeMidiCommand::Stop(reply) => {
                        let cancelled = scheduled.len() as u64;
                        scheduled.clear();
                        controller.stop();
                        update_realtime_status(shared_status, |status| {
                            status.cancelled_count =
                                status.cancelled_count.saturating_add(cancelled);
                        });
                        publish_controller_status(shared_status, &controller);
                        let _ = reply.send(());
                    }
                    RealtimeMidiCommand::SendLearnPulse(address, reply) => {
                        let result = controller
                            .send_address_learn_pulse(address)
                            .map_err(|error| error.to_string());
                        publish_controller_status(shared_status, &controller);
                        let _ = reply.send(result);
                    }
                    RealtimeMidiCommand::SelectBank(bank, reply) => {
                        let result = controller
                            .select_bank(bank)
                            .map_err(|error| error.to_string());
                        publish_controller_status(shared_status, &controller);
                        let _ = reply.send(result);
                    }
                    RealtimeMidiCommand::TriggerAutoloop(autoloop, reply) => {
                        let result = controller
                            .trigger_autoloop_button(autoloop)
                            .map_err(|error| error.to_string());
                        publish_controller_status(shared_status, &controller);
                        let _ = reply.send(result);
                    }
                    RealtimeMidiCommand::TriggerSequence(bank, autoloop, reply) => {
                        let result = controller
                            .trigger_autoloop(bank, autoloop)
                            .map_err(|error| error.to_string());
                        publish_controller_status(shared_status, &controller);
                        let _ = reply.send(result);
                    }
                    RealtimeMidiCommand::SetGeneration(next) => {
                        generation = next;
                        let before = scheduled.len();
                        scheduled.retain(|item| item.generation == generation);
                        let cancelled = before.saturating_sub(scheduled.len()) as u64;
                        update_realtime_status(shared_status, |status| {
                            status.cancelled_count =
                                status.cancelled_count.saturating_add(cancelled);
                        });
                    }
                    RealtimeMidiCommand::Schedule(item) => {
                        if item.generation != generation {
                            update_realtime_status(shared_status, |status| {
                                status.cancelled_count = status.cancelled_count.saturating_add(1);
                            });
                        } else if scheduled.len() >= REALTIME_SCHEDULE_CAPACITY {
                            update_realtime_status(shared_status, |status| {
                                status.saturation_count = status.saturation_count.saturating_add(1);
                                status.source.last_error = Some(
                                    "Realtime MIDI deadline capacity exceeded; output failed closed"
                                        .to_owned(),
                                );
                            });
                        } else {
                            let (kind, number) = realtime_action_identity(item.action);
                            let lead = item.deadline.saturating_duration_since(Instant::now());
                            scheduled.push(item);
                            update_realtime_status(shared_status, |status| {
                                status.scheduled_count = status.scheduled_count.saturating_add(1);
                                status.last_scheduled_action = Some(kind);
                                status.last_scheduled_number = Some(number);
                                status.last_scheduled_lead_micros =
                                    Some(u64::try_from(lead.as_micros()).unwrap_or(u64::MAX));
                            });
                        }
                    }
                    RealtimeMidiCommand::CancelAll => {
                        let cancelled = scheduled.len() as u64;
                        scheduled.clear();
                        update_realtime_status(shared_status, |status| {
                            status.cancelled_count =
                                status.cancelled_count.saturating_add(cancelled);
                        });
                    }
                    RealtimeMidiCommand::Shutdown => {
                        controller.stop();
                        publish_controller_status(shared_status, &controller);
                        return;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                let mut due = Vec::new();
                scheduled.retain(|item| {
                    if item.generation == generation && item.deadline <= now {
                        due.push(*item);
                        false
                    } else {
                        true
                    }
                });
                due.sort_by_key(|item| item.deadline);
                for item in due {
                    let (kind, number) = realtime_action_identity(item.action);
                    let result = match item.action {
                        RealtimeMidiAction::SelectBank(bank) => controller.select_bank(bank),
                        RealtimeMidiAction::TriggerAutoloop(autoloop) => {
                            controller.trigger_autoloop_button(autoloop)
                        }
                    };
                    let elapsed = Instant::now().saturating_duration_since(item.deadline);
                    let micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
                    if latencies.len() == REALTIME_LATENCY_SAMPLE_CAPACITY {
                        latencies.pop_front();
                    }
                    latencies.push_back(micros);
                    update_realtime_status(shared_status, |status| {
                        if result.is_ok() {
                            status.emitted_count = status.emitted_count.saturating_add(1);
                            status.last_emitted_action = Some(kind);
                            status.last_emitted_number = Some(number);
                        }
                        status.last_dispatch_lateness_micros = Some(micros);
                        if elapsed > REALTIME_LATE_DISPATCH_THRESHOLD {
                            status.late_dispatch_count =
                                status.late_dispatch_count.saturating_add(1);
                        }
                        update_latency_distribution(status, &latencies);
                    });
                    publish_controller_status(shared_status, &controller);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                controller.stop();
                publish_controller_status(shared_status, &controller);
                return;
            }
        }
    }
}

const fn realtime_action_identity(action: RealtimeMidiAction) -> (RealtimeMidiActionKind, u8) {
    match action {
        RealtimeMidiAction::SelectBank(number) => (RealtimeMidiActionKind::Bank, number),
        RealtimeMidiAction::TriggerAutoloop(number) => (RealtimeMidiActionKind::Autoloop, number),
    }
}

fn publish_controller_status<P>(
    shared_status: &Arc<Mutex<RealtimeMidiStatus>>,
    controller: &MidiOutputController<P>,
) where
    P: MidiSourceProvider,
{
    update_realtime_status(shared_status, |status| {
        status.source = controller.status();
    });
}

fn update_realtime_status(
    shared_status: &Arc<Mutex<RealtimeMidiStatus>>,
    update: impl FnOnce(&mut RealtimeMidiStatus),
) {
    if let Ok(mut status) = shared_status.lock() {
        update(&mut status);
    }
}

fn update_latency_distribution(status: &mut RealtimeMidiStatus, latencies: &VecDeque<u64>) {
    let mut sorted: Vec<_> = latencies.iter().copied().collect();
    sorted.sort_unstable();
    status.latency_sample_count = sorted.len();
    status.latency_p50_micros = percentile(&sorted, 50);
    status.latency_p95_micros = percentile(&sorted, 95);
    status.latency_p99_micros = percentile(&sorted, 99);
    status.latency_max_micros = sorted.last().copied().unwrap_or(0);
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let index = (sorted.len() - 1).saturating_mul(percentile) / 100;
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingProvider {
        published: bool,
        messages: Vec<MidiMessage>,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("recording provider failed")]
    struct RecordingError;

    impl MidiSourceProvider for RecordingProvider {
        type Error = RecordingError;

        fn publish(&mut self, source_name: &str) -> Result<(), Self::Error> {
            self.published = source_name == MIDI_SOURCE_NAME;
            Ok(())
        }

        fn stop(&mut self) {
            self.published = false;
        }

        fn send(&mut self, messages: &[MidiMessage]) -> Result<(), Self::Error> {
            self.messages.extend_from_slice(messages);
            Ok(())
        }
    }

    #[test]
    fn learn_pulse_is_fail_silent_until_published() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());

        assert!(matches!(
            controller.send_learn_pulse(),
            Err(MidiOutputError::SourceNotPublished)
        ));
        assert_eq!(controller.status().sent_pulse_count, 0);
    }

    #[test]
    fn learn_pulse_always_contains_note_on_and_note_off() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        assert!(controller.send_learn_pulse().is_ok());

        assert_eq!(controller.provider.messages.len(), 2);
        assert_eq!(controller.provider.messages[0].bytes(), [0x9f, 60, 100]);
        assert_eq!(controller.provider.messages[1].bytes(), [0x8f, 60, 0]);
        assert_eq!(controller.status().sent_pulse_count, 1);
    }

    #[test]
    fn four_banks_and_thirty_two_autoloops_have_stable_unique_notes() {
        let bank_notes = (1..=4)
            .filter_map(MidiAddress::bank)
            .map(MidiAddress::note)
            .collect::<Vec<_>>();
        let autoloop_notes = (1..=32)
            .filter_map(MidiAddress::autoloop)
            .map(MidiAddress::note)
            .collect::<Vec<_>>();

        assert_eq!(bank_notes, [60, 61, 62, 63]);
        assert_eq!(autoloop_notes.first(), Some(&64));
        assert_eq!(autoloop_notes.last(), Some(&95));
        assert!(bank_notes.iter().all(|note| !autoloop_notes.contains(note)));
    }

    #[test]
    fn address_learn_pulse_uses_the_requested_autoloop_note() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        let Some(address) = MidiAddress::autoloop(32) else {
            panic!("AutoLoop 32 must have a MIDI address");
        };
        assert!(controller.send_address_learn_pulse(address).is_ok());

        assert_eq!(controller.provider.messages[0].bytes(), [0x9f, 95, 100]);
        assert_eq!(controller.provider.messages[1].bytes(), [0x8f, 95, 0]);
    }

    #[test]
    fn runtime_trigger_selects_bank_then_autoloop_after_settle_delay() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        let mut observed_delay = None;

        assert!(
            controller
                .trigger_autoloop_with_wait(1, 1, |delay| observed_delay = Some(delay))
                .is_ok()
        );

        assert_eq!(observed_delay, Some(BANK_SETTLE_DELAY));
        assert_eq!(controller.provider.messages.len(), 4);
        assert_eq!(controller.provider.messages[0].bytes(), [0x9f, 60, 100]);
        assert_eq!(controller.provider.messages[1].bytes(), [0x8f, 60, 0]);
        assert_eq!(controller.provider.messages[2].bytes(), [0x9f, 64, 100]);
        assert_eq!(controller.provider.messages[3].bytes(), [0x8f, 64, 0]);
        assert_eq!(controller.status().sent_pulse_count, 2);
        assert!(
            controller
                .status()
                .last_event
                .as_deref()
                .is_some_and(|event| event.contains("Bank 1 → AutoLoop 1"))
        );
    }

    #[test]
    fn runtime_trigger_reasserts_the_bank_for_parallel_manual_control() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        assert!(controller.trigger_autoloop_with_wait(2, 3, |_| {}).is_ok());
        let mut observed_delay = None;

        assert!(
            controller
                .trigger_autoloop_with_wait(2, 4, |delay| observed_delay = Some(delay))
                .is_ok()
        );

        assert_eq!(observed_delay, Some(BANK_SETTLE_DELAY));
        assert_eq!(controller.provider.messages.len(), 8);
        assert_eq!(controller.provider.messages[4].bytes(), [0x9f, 61, 100]);
        assert_eq!(controller.provider.messages[6].bytes(), [0x9f, 67, 100]);
        assert_eq!(controller.status().sent_pulse_count, 4);
        assert_eq!(controller.status().active_bank, Some(2));
    }

    #[test]
    fn realtime_scheduler_can_split_bank_prearm_from_autoloop_pulse() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());

        assert!(controller.select_bank(4).is_ok());
        assert_eq!(controller.provider.messages.len(), 2);
        assert_eq!(controller.provider.messages[0].bytes(), [0x9f, 63, 100]);
        assert_eq!(controller.active_bank(), Some(4));

        assert!(controller.trigger_autoloop_button(32).is_ok());
        assert_eq!(controller.provider.messages.len(), 4);
        assert_eq!(controller.provider.messages[2].bytes(), [0x9f, 95, 100]);
        assert_eq!(controller.status().sent_pulse_count, 2);
    }

    #[test]
    fn realtime_lane_emits_deadlines_without_caller_polling_and_records_distribution() {
        let lane = RealtimeMidiController::new(RecordingProvider::default);
        assert!(lane.publish().is_ok());
        assert!(lane.set_generation(7).is_ok());
        let now = Instant::now();
        assert!(
            lane.schedule_bank(7, 2, now + Duration::from_millis(10))
                .is_ok()
        );
        assert!(
            lane.schedule_autoloop(7, 13, now + Duration::from_millis(65))
                .is_ok()
        );

        let deadline = Instant::now() + Duration::from_secs(1);
        while lane.status().emitted_count < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        let status = lane.status();
        assert_eq!(status.emitted_count, 2);
        assert_eq!(status.source.sent_pulse_count, 2);
        assert_eq!(status.source.active_bank, Some(2));
        assert_eq!(status.latency_sample_count, 2);
        assert!(status.latency_p95_micros <= 20_000, "{status:?}");
    }

    #[test]
    fn realtime_lane_generation_change_cancels_stale_output() {
        let lane = RealtimeMidiController::new(RecordingProvider::default);
        assert!(lane.publish().is_ok());
        assert!(lane.set_generation(10).is_ok());
        assert!(
            lane.schedule_autoloop(10, 31, Instant::now() + Duration::from_millis(80))
                .is_ok()
        );
        assert!(lane.set_generation(11).is_ok());
        thread::sleep(Duration::from_millis(120));

        let status = lane.status();
        assert_eq!(status.source.sent_pulse_count, 0);
        assert_eq!(status.emitted_count, 0);
        assert_eq!(status.cancelled_count, 1);
    }

    #[test]
    fn realtime_lane_keeps_due_phrase_pulse_when_next_phrase_is_prepared() {
        let lane = RealtimeMidiController::new(RecordingProvider::default);
        assert!(lane.publish().is_ok());
        assert!(lane.set_generation(12).is_ok());
        let now = Instant::now();
        assert!(
            lane.schedule_autoloop(12, 7, now + Duration::from_millis(30))
                .is_ok()
        );
        thread::sleep(Duration::from_millis(25));
        assert!(
            lane.schedule_bank(12, 2, now + Duration::from_millis(120))
                .is_ok()
        );

        let deadline = Instant::now() + Duration::from_secs(1);
        while lane.status().emitted_count < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        let status = lane.status();
        assert_eq!(status.emitted_count, 2);
        assert_eq!(status.cancelled_count, 0);
        assert_eq!(status.source.sent_pulse_count, 2);
        assert_eq!(
            status.last_emitted_action,
            Some(RealtimeMidiActionKind::Bank)
        );
        assert_eq!(status.last_emitted_number, Some(2));
    }

    #[test]
    fn realtime_lane_fails_closed_when_deadline_capacity_is_exceeded() {
        let lane = RealtimeMidiController::new(RecordingProvider::default);
        assert!(lane.set_generation(21).is_ok());
        let distant_deadline = Instant::now() + Duration::from_secs(60);

        for index in 0..=REALTIME_SCHEDULE_CAPACITY {
            assert!(
                lane.schedule_autoloop(21, (index % 32 + 1) as u8, distant_deadline)
                    .is_ok()
            );
            let accepted_deadline = Instant::now() + Duration::from_secs(1);
            while lane.status().queue_depth > 0 && Instant::now() < accepted_deadline {
                thread::yield_now();
            }
        }

        let observed_deadline = Instant::now() + Duration::from_secs(1);
        while lane.status().saturation_count == 0 && Instant::now() < observed_deadline {
            thread::yield_now();
        }
        let status = lane.status();
        assert_eq!(status.scheduled_count, REALTIME_SCHEDULE_CAPACITY as u64);
        assert_eq!(status.saturation_count, 1);
        assert!(
            status
                .source
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("failed closed"))
        );
    }

    #[test]
    fn runtime_trigger_selects_a_new_bank_after_the_bank_changes() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());
        assert!(controller.publish().is_ok());
        assert!(controller.trigger_autoloop_with_wait(1, 1, |_| {}).is_ok());
        let mut observed_delay = None;

        assert!(
            controller
                .trigger_autoloop_with_wait(3, 1, |delay| observed_delay = Some(delay))
                .is_ok()
        );

        assert_eq!(observed_delay, Some(BANK_SETTLE_DELAY));
        assert_eq!(controller.provider.messages.len(), 8);
        assert_eq!(controller.provider.messages[4].bytes(), [0x9f, 62, 100]);
        assert_eq!(controller.status().active_bank, Some(3));
    }

    #[test]
    fn runtime_trigger_is_fail_silent_until_source_is_published() {
        let mut controller = MidiOutputController::new(RecordingProvider::default());

        assert!(matches!(
            controller.trigger_autoloop_with_wait(1, 1, |_| {}),
            Err(MidiOutputError::SourceNotPublished)
        ));
        assert!(controller.provider.messages.is_empty());
        assert_eq!(controller.status().sent_pulse_count, 0);
    }

    #[derive(Clone, Default)]
    struct SharedRecordingProvider {
        state: Arc<Mutex<SharedRecordingState>>,
    }

    #[derive(Default)]
    struct SharedRecordingState {
        published_name: Option<String>,
        messages: Vec<MidiMessage>,
        fail_clock: bool,
    }

    impl MidiSourceProvider for SharedRecordingProvider {
        type Error = RecordingError;

        fn publish(&mut self, source_name: &str) -> Result<(), Self::Error> {
            if let Ok(mut state) = self.state.lock() {
                state.published_name = Some(source_name.to_owned());
            }
            Ok(())
        }

        fn stop(&mut self) {
            if let Ok(mut state) = self.state.lock() {
                state.published_name = None;
            }
        }

        fn send(&mut self, messages: &[MidiMessage]) -> Result<(), Self::Error> {
            if let Ok(mut state) = self.state.lock() {
                if state.fail_clock
                    && messages
                        .iter()
                        .any(|message| message.bytes()[0] == MidiMessage::clock().bytes()[0])
                {
                    return Err(RecordingError);
                }
                state.messages.extend_from_slice(messages);
            }
            Ok(())
        }
    }

    #[test]
    fn midi_clock_publishes_transport_ticks_and_pause_on_a_dedicated_source() {
        let shared = Arc::new(Mutex::new(SharedRecordingState::default()));
        let factory_state = Arc::clone(&shared);
        let mut clock = MidiClockController::new(move || SharedRecordingProvider {
            state: factory_state,
        });
        assert!(clock.publish().is_ok());
        assert!(
            clock
                .synchronize(MidiClockSync {
                    bpm_milli: 120_000,
                    playing: true,
                    song_position_16th: 0,
                    delay_to_next_tick: Duration::ZERO,
                    rephase: true,
                })
                .is_ok()
        );

        let deadline = Instant::now() + Duration::from_millis(300);
        while clock.status().sent_tick_count < 3 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(clock.status().sent_tick_count >= 3);
        assert!(
            clock
                .synchronize(MidiClockSync {
                    bpm_milli: 120_000,
                    playing: false,
                    song_position_16th: 0,
                    delay_to_next_tick: Duration::ZERO,
                    rephase: false,
                })
                .is_ok()
        );
        let pause_deadline = Instant::now() + Duration::from_millis(100);
        while clock.status().state == MidiClockState::Running && Instant::now() < pause_deadline {
            thread::sleep(Duration::from_millis(5));
        }

        let messages = shared
            .lock()
            .map(|state| state.messages.clone())
            .unwrap_or_default();
        assert_eq!(
            messages.first().map(|message| message.bytes()[0]),
            Some(0xfa)
        );
        assert!(
            messages
                .iter()
                .filter(|message| message.bytes()[0] == 0xf8)
                .count()
                >= 3
        );
        assert_eq!(
            messages.last().map(|message| message.bytes()[0]),
            Some(0xfc)
        );
        assert_eq!(clock.status().state, MidiClockState::Ready);
    }

    #[test]
    fn song_position_uses_the_midi_one_fourteenth_bit_encoding() {
        let position = MidiMessage::song_position(1_025)
            .unwrap_or_else(|| panic!("test song position must fit MIDI 1.0"));
        assert_eq!(position.bytes(), [0xf2, 1, 8]);
        assert_eq!(position.length(), 3);
        assert_eq!(MidiMessage::clock().length(), 1);
        assert!(MidiMessage::song_position(16_384).is_none());
    }

    #[test]
    fn midi_clock_send_failure_unpublishes_and_requires_explicit_recovery() {
        let shared = Arc::new(Mutex::new(SharedRecordingState {
            fail_clock: true,
            ..SharedRecordingState::default()
        }));
        let factory_state = Arc::clone(&shared);
        let mut clock = MidiClockController::new(move || SharedRecordingProvider {
            state: factory_state,
        });
        assert!(clock.publish().is_ok());
        assert!(
            clock
                .synchronize(MidiClockSync {
                    bpm_milli: 120_000,
                    playing: true,
                    song_position_16th: 0,
                    delay_to_next_tick: Duration::ZERO,
                    rephase: true,
                })
                .is_ok()
        );

        let deadline = Instant::now() + Duration::from_millis(300);
        while clock.status().last_error.is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(clock.status().state, MidiClockState::Stopped);
        assert!(clock.status().last_error.is_some());
        assert!(
            shared
                .lock()
                .is_ok_and(|state| state.published_name.is_none())
        );

        if let Ok(mut state) = shared.lock() {
            state.fail_clock = false;
        }
        assert!(clock.publish().is_ok());
        assert_eq!(clock.status().state, MidiClockState::Ready);
        assert!(clock.status().last_error.is_none());
    }
}
