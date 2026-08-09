//! Beat Link Trigger MIDI adapter.
//!
//! BLT expressions publish an atomic, versioned CC frame per player. This
//! crate owns the wire format and translates complete frames into Lumi's
//! provider-neutral deck observations. It has no knowledge of SwiftUI,
//! SoundSwitch, physical decks, or CoreMIDI endpoint discovery.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{
    DeckId, DeckObservation, DeckSourceStatus, DomainEvent, KeyMode, MonotonicTime, MusicalKey,
    ObservationEnvelope, PitchClass, SourceId, SourceSequence, TrackId, TrackLoadId, TrackMetadata,
};
use lumi_midi_coremidi::MidiChannelVoiceMessage;
use thiserror::Error;

pub const PROTOCOL_NAME: &str = "BLT MIDI Deck Frame";
pub const PROTOCOL_VERSION: u8 = 4;

const SOURCE_ID: u64 = 30;
const CONTROL_CHANGE_STATUS: u8 = 0xb;
const FLAGS_CC: u8 = 16;
const REKORDBOX_ID_CC: u8 = 17;
const SOURCE_PLAYER_CC: u8 = 21;
const SOURCE_SLOT_CC: u8 = 22;
const BPM_MILLI_CC: u8 = 23;
const BEAT_CC: u8 = 26;
const DURATION_SECONDS_CC: u8 = 29;
const FRAME_SEQUENCE_CC: u8 = 32;
const EFFECTIVE_BPM_MILLI_CC: u8 = 33;
const SIMULATOR_SIGNATURE_CC: u8 = 36;
const PLAYBACK_POSITION_MILLIS_CC: u8 = 41;
const COMMIT_CC: u8 = 119;

const FLAG_LOADED: u8 = 1;
const FLAG_PLAYING: u8 = 2;
const FLAG_MASTER: u8 = 4;
const FLAG_ON_AIR: u8 = 8;
const FLAG_POSITION_KNOWN: u8 = 16;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FrameAssembler {
    present_fields: u32,
    flags: u8,
    rekordbox_id: [u8; 4],
    source_player: u8,
    source_slot: u8,
    track_bpm_milli: [u8; 3],
    beat: [u8; 3],
    duration_seconds: [u8; 3],
    frame_sequence: u8,
    effective_bpm_milli: [u8; 3],
    simulator_signature: [u8; 5],
    playback_position_millis: [u8; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeckFrame {
    loaded: bool,
    playing: bool,
    master: bool,
    on_air: bool,
    rekordbox_id: u32,
    source_player: u8,
    source_slot: u8,
    track_bpm_milli: u32,
    effective_bpm_milli: u32,
    beat: u32,
    duration_seconds: u32,
    frame_sequence: u8,
    simulator_signature: u32,
    playback_position_millis: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LoadedDeck {
    identity: u64,
    track_load_id: TrackLoadId,
    beat: u32,
    effective_bpm_milli: u32,
    playing: bool,
    media_identity: BltTrackIdentity,
    playback_position_millis: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BltTrackIdentity {
    pub rekordbox_id: u32,
    pub source_player: u8,
    pub source_slot: u8,
    pub simulator_signature: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BltTransportSnapshot {
    pub position_millis: Option<u32>,
    pub effective_bpm_milli: u32,
    pub playing: bool,
}

pub struct BltMidiDeckSourceProvider {
    source_id: SourceId,
    sequence: u64,
    next_track_load_id: u64,
    assemblers: [FrameAssembler; 2],
    last_frame_received_at: [Option<Instant>; 2],
    last_frame_sequences: [Option<u8>; 2],
    decks: BTreeMap<DeckId, LoadedDeck>,
    leader_deck_id: Option<DeckId>,
    pending_events: Vec<DomainEvent>,
    committed_frame_count: u64,
    ignored_message_count: u64,
    duplicate_frame_count: u64,
    last_frame: Option<(DeckId, DeckFrame)>,
    ready_emitted: bool,
}

impl BltMidiDeckSourceProvider {
    pub fn new(at: MonotonicTime) -> Result<Self, BltMidiError> {
        let mut provider = Self {
            source_id: SourceId::new(SOURCE_ID),
            sequence: 0,
            next_track_load_id: 1,
            assemblers: [FrameAssembler::default(); 2],
            last_frame_received_at: [None; 2],
            last_frame_sequences: [None, None],
            decks: BTreeMap::new(),
            leader_deck_id: None,
            pending_events: Vec::new(),
            committed_frame_count: 0,
            ignored_message_count: 0,
            duplicate_frame_count: 0,
            last_frame: None,
            ready_emitted: false,
        };
        provider.emit(
            at,
            DeckObservation::SourceStatusChanged {
                status: DeckSourceStatus::Starting,
            },
        )?;
        Ok(provider)
    }

    pub fn ingest(
        &mut self,
        message: MidiChannelVoiceMessage,
        at: MonotonicTime,
    ) -> Result<(), BltMidiError> {
        if message.status != CONTROL_CHANGE_STATUS || !(1..=2).contains(&message.channel) {
            self.ignored_message_count = self.ignored_message_count.saturating_add(1);
            return Ok(());
        }
        let index = usize::from(message.channel - 1);
        if message.data_one == COMMIT_CC {
            if message.data_two != PROTOCOL_VERSION {
                self.ignored_message_count = self.ignored_message_count.saturating_add(1);
                return Ok(());
            }
            let assembler = std::mem::take(&mut self.assemblers[index]);
            let Ok(frame) = decode_frame(assembler) else {
                self.ignored_message_count = self.ignored_message_count.saturating_add(1);
                return Ok(());
            };
            if self.last_frame_sequences[index] == Some(frame.frame_sequence) {
                self.duplicate_frame_count = self.duplicate_frame_count.saturating_add(1);
                return Ok(());
            }
            self.last_frame_sequences[index] = Some(frame.frame_sequence);
            self.last_frame_received_at[index] = Some(Instant::now());
            self.committed_frame_count = self.committed_frame_count.saturating_add(1);
            let deck_id = DeckId::new(message.channel);
            self.apply_frame(deck_id, frame, at)?;
            self.last_frame = Some((deck_id, frame));
            return Ok(());
        }
        if write_field(
            &mut self.assemblers[index],
            message.data_one,
            message.data_two,
        ) {
            self.assemblers[index].present_fields |=
                1_u32 << u32::from(message.data_one - FLAGS_CC);
        } else {
            self.ignored_message_count = self.ignored_message_count.saturating_add(1);
        }
        Ok(())
    }

    /// Removes decks whose BLT heartbeat disappeared. The virtual MIDI
    /// destination remains published when BLT quits, so endpoint state alone
    /// cannot distinguish a stopped source from a paused deck.
    pub fn expire_stale(
        &mut self,
        now: Instant,
        maximum_age: Duration,
        at: MonotonicTime,
    ) -> Result<(), BltMidiError> {
        for index in 0..self.last_frame_received_at.len() {
            let Some(last_received_at) = self.last_frame_received_at[index] else {
                continue;
            };
            if now.saturating_duration_since(last_received_at) <= maximum_age {
                continue;
            }
            self.last_frame_received_at[index] = None;
            self.last_frame_sequences[index] = None;
            self.assemblers[index] = FrameAssembler::default();
            let deck_id = DeckId::new(if index == 0 { 1 } else { 2 });
            if let Some(previous) = self.decks.remove(&deck_id) {
                self.emit(
                    at,
                    DeckObservation::TrackUnloaded {
                        deck_id,
                        track_load_id: previous.track_load_id,
                    },
                )?;
            }
            if self.leader_deck_id == Some(deck_id) {
                self.leader_deck_id = None;
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn leader_deck_id(&self) -> Option<DeckId> {
        self.leader_deck_id
    }

    #[must_use]
    pub const fn diagnostics(&self) -> BltMidiDiagnostics {
        BltMidiDiagnostics {
            committed_frame_count: self.committed_frame_count,
            ignored_message_count: self.ignored_message_count,
            duplicate_frame_count: self.duplicate_frame_count,
            last_deck_id: match self.last_frame {
                Some((deck_id, _)) => Some(deck_id),
                None => None,
            },
            last_frame_sequence: match self.last_frame {
                Some((_, frame)) => Some(frame.frame_sequence),
                None => None,
            },
        }
    }

    #[must_use]
    pub fn track_identity(&self, track_load_id: TrackLoadId) -> Option<BltTrackIdentity> {
        self.decks
            .values()
            .find(|deck| deck.track_load_id == track_load_id)
            .map(|deck| deck.media_identity)
    }

    #[must_use]
    pub fn transport(&self, track_load_id: TrackLoadId) -> Option<BltTransportSnapshot> {
        self.decks
            .values()
            .find(|deck| deck.track_load_id == track_load_id)
            .map(|deck| BltTransportSnapshot {
                position_millis: deck.playback_position_millis,
                effective_bpm_milli: deck.effective_bpm_milli,
                playing: deck.playing,
            })
    }

    pub fn clear(&mut self, at: MonotonicTime) -> Result<(), BltMidiError> {
        let loaded = self
            .decks
            .iter()
            .map(|(deck_id, deck)| (*deck_id, deck.track_load_id))
            .collect::<Vec<_>>();
        for (deck_id, track_load_id) in loaded {
            self.emit(
                at,
                DeckObservation::TrackUnloaded {
                    deck_id,
                    track_load_id,
                },
            )?;
        }
        self.decks.clear();
        self.leader_deck_id = None;
        self.last_frame_received_at = [None; 2];
        self.last_frame_sequences = [None; 2];
        self.assemblers = [FrameAssembler::default(); 2];
        Ok(())
    }

    fn apply_frame(
        &mut self,
        deck_id: DeckId,
        frame: DeckFrame,
        at: MonotonicTime,
    ) -> Result<(), BltMidiError> {
        if !self.ready_emitted {
            self.emit(
                at,
                DeckObservation::SourceStatusChanged {
                    status: DeckSourceStatus::Ready,
                },
            )?;
            self.ready_emitted = true;
        }
        if !frame.loaded {
            if let Some(previous) = self.decks.remove(&deck_id) {
                self.emit(
                    at,
                    DeckObservation::TrackUnloaded {
                        deck_id,
                        track_load_id: previous.track_load_id,
                    },
                )?;
            }
            return Ok(());
        }

        let identity = track_identity(frame);
        let needs_load = self
            .decks
            .get(&deck_id)
            .is_none_or(|deck| deck.identity != identity);
        if needs_load {
            if let Some(previous) = self.decks.remove(&deck_id) {
                self.emit(
                    at,
                    DeckObservation::TrackUnloaded {
                        deck_id,
                        track_load_id: previous.track_load_id,
                    },
                )?;
            }
            let track_load_id = TrackLoadId::new(self.next_track_load_id);
            self.next_track_load_id = self
                .next_track_load_id
                .checked_add(1)
                .ok_or(BltMidiError::TrackLoadIdOverflow)?;
            let duration_beats = duration_beats(frame)?;
            let metadata = TrackMetadata::try_new_unanalyzed(
                TrackId::new(identity),
                format!("External track {}", frame.rekordbox_id),
                "Beat Link Trigger".to_owned(),
                frame.track_bpm_milli,
                MusicalKey::new(PitchClass::C, KeyMode::Minor),
                duration_beats,
            )?;
            self.decks.insert(
                deck_id,
                LoadedDeck {
                    identity,
                    track_load_id,
                    beat: frame.beat.min(duration_beats),
                    effective_bpm_milli: frame.effective_bpm_milli,
                    playing: frame.playing,
                    media_identity: BltTrackIdentity {
                        rekordbox_id: frame.rekordbox_id,
                        source_player: frame.source_player,
                        source_slot: frame.source_slot,
                        simulator_signature: frame.simulator_signature,
                    },
                    playback_position_millis: frame.playback_position_millis,
                },
            );
            self.emit(
                at,
                DeckObservation::TrackLoaded {
                    deck_id,
                    metadata,
                    track_load_id,
                },
            )?;
            if frame.effective_bpm_milli != frame.track_bpm_milli {
                self.emit(
                    at,
                    DeckObservation::PlaybackTempoChanged {
                        deck_id,
                        track_load_id,
                        bpm_milli: frame.effective_bpm_milli,
                    },
                )?;
            }
            self.emit(
                at,
                DeckObservation::PlaybackPosition {
                    deck_id,
                    track_load_id,
                    beat: frame.beat.min(duration_beats),
                },
            )?;
            self.emit(
                at,
                DeckObservation::PlaybackStateChanged {
                    deck_id,
                    track_load_id,
                    playing: frame.playing,
                },
            )?;
        } else if let Some(previous) = self.decks.get(&deck_id).copied() {
            let duration = duration_beats(frame)?;
            let beat = frame.beat.min(duration);
            if previous.effective_bpm_milli != frame.effective_bpm_milli {
                self.emit(
                    at,
                    DeckObservation::PlaybackTempoChanged {
                        deck_id,
                        track_load_id: previous.track_load_id,
                        bpm_milli: frame.effective_bpm_milli,
                    },
                )?;
            }
            if previous.beat != beat {
                let observation = if beat < previous.beat {
                    DeckObservation::PlaybackPositionSeeked {
                        deck_id,
                        track_load_id: previous.track_load_id,
                        beat,
                    }
                } else {
                    DeckObservation::PlaybackPosition {
                        deck_id,
                        track_load_id: previous.track_load_id,
                        beat,
                    }
                };
                self.emit(at, observation)?;
            }
            if previous.playing != frame.playing {
                self.emit(
                    at,
                    DeckObservation::PlaybackStateChanged {
                        deck_id,
                        track_load_id: previous.track_load_id,
                        playing: frame.playing,
                    },
                )?;
            }
            if let Some(deck) = self.decks.get_mut(&deck_id) {
                deck.beat = beat;
                deck.effective_bpm_milli = frame.effective_bpm_milli;
                deck.playing = frame.playing;
                deck.playback_position_millis = frame.playback_position_millis;
            }
        }

        if frame.master && self.leader_deck_id != Some(deck_id) {
            let track_load_id = self
                .decks
                .get(&deck_id)
                .ok_or(BltMidiError::LoadedDeckMissing)?
                .track_load_id;
            self.leader_deck_id = Some(deck_id);
            self.emit(
                at,
                DeckObservation::LeaderChanged {
                    deck_id,
                    track_load_id,
                },
            )?;
        }
        Ok(())
    }

    fn emit(
        &mut self,
        at: MonotonicTime,
        observation: DeckObservation,
    ) -> Result<(), BltMidiError> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(BltMidiError::SequenceOverflow)?;
        self.pending_events
            .push(DomainEvent::Observation(ObservationEnvelope {
                source_id: self.source_id,
                sequence: SourceSequence::new(self.sequence),
                observed_at: at,
                observation,
            }));
        Ok(())
    }
}

impl DeckSourceProvider for BltMidiDeckSourceProvider {
    type Error = BltMidiError;

    fn provider_kind(&self) -> &'static str {
        "beatLinkTriggerMidi"
    }

    fn drain_events(&mut self) -> Result<Vec<DomainEvent>, Self::Error> {
        Ok(std::mem::take(&mut self.pending_events))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BltMidiDiagnostics {
    pub committed_frame_count: u64,
    pub ignored_message_count: u64,
    pub duplicate_frame_count: u64,
    pub last_deck_id: Option<DeckId>,
    pub last_frame_sequence: Option<u8>,
}

fn write_field(frame: &mut FrameAssembler, controller: u8, value: u8) -> bool {
    match controller {
        FLAGS_CC => frame.flags = value,
        REKORDBOX_ID_CC..=20 => {
            frame.rekordbox_id[usize::from(controller - REKORDBOX_ID_CC)] = value
        }
        SOURCE_PLAYER_CC => frame.source_player = value,
        SOURCE_SLOT_CC => frame.source_slot = value,
        BPM_MILLI_CC..=25 => {
            frame.track_bpm_milli[usize::from(controller - BPM_MILLI_CC)] = value;
        }
        BEAT_CC..=28 => frame.beat[usize::from(controller - BEAT_CC)] = value,
        DURATION_SECONDS_CC..=31 => {
            frame.duration_seconds[usize::from(controller - DURATION_SECONDS_CC)] = value;
        }
        FRAME_SEQUENCE_CC => frame.frame_sequence = value,
        EFFECTIVE_BPM_MILLI_CC..=35 => {
            frame.effective_bpm_milli[usize::from(controller - EFFECTIVE_BPM_MILLI_CC)] = value;
        }
        SIMULATOR_SIGNATURE_CC..=40 => {
            frame.simulator_signature[usize::from(controller - SIMULATOR_SIGNATURE_CC)] = value;
        }
        PLAYBACK_POSITION_MILLIS_CC..=43 => {
            frame.playback_position_millis[usize::from(controller - PLAYBACK_POSITION_MILLIS_CC)] =
                value;
        }
        _ => return false,
    }
    true
}

fn decode_frame(frame: FrameAssembler) -> Result<DeckFrame, BltMidiError> {
    const REQUIRED_FIELDS: u32 = (1_u32 << 28) - 1;
    if frame.present_fields != REQUIRED_FIELDS {
        return Err(BltMidiError::IncompleteFrame);
    }
    let rekordbox_id = decode_7bit(&frame.rekordbox_id)?;
    let track_bpm_milli = decode_7bit(&frame.track_bpm_milli)?;
    let effective_bpm_milli = decode_7bit(&frame.effective_bpm_milli)?;
    let beat = decode_7bit(&frame.beat)?;
    let duration_seconds = decode_7bit(&frame.duration_seconds)?;
    let simulator_signature = decode_7bit(&frame.simulator_signature)?;
    let playback_position_millis = decode_7bit(&frame.playback_position_millis)?;
    let loaded = frame.flags & FLAG_LOADED != 0;
    if loaded
        && (rekordbox_id == 0
            || !(20_000..=300_000).contains(&track_bpm_milli)
            || !(20_000..=300_000).contains(&effective_bpm_milli)
            || duration_seconds == 0)
    {
        return Err(BltMidiError::IncompleteFrame);
    }
    Ok(DeckFrame {
        loaded,
        playing: frame.flags & FLAG_PLAYING != 0,
        master: frame.flags & FLAG_MASTER != 0,
        on_air: frame.flags & FLAG_ON_AIR != 0,
        rekordbox_id,
        source_player: frame.source_player,
        source_slot: frame.source_slot,
        track_bpm_milli,
        effective_bpm_milli,
        beat,
        duration_seconds,
        frame_sequence: frame.frame_sequence,
        simulator_signature,
        playback_position_millis: (frame.flags & FLAG_POSITION_KNOWN != 0)
            .then_some(playback_position_millis),
    })
}

fn decode_7bit<const N: usize>(chunks: &[u8; N]) -> Result<u32, BltMidiError> {
    let mut value = 0_u32;
    for (index, chunk) in chunks.iter().copied().enumerate() {
        let shift = u32::try_from(index)
            .map_err(|_| BltMidiError::FrameValueOverflow)?
            .checked_mul(7)
            .ok_or(BltMidiError::FrameValueOverflow)?;
        value |= u32::from(chunk) << shift;
    }
    Ok(value)
}

fn track_identity(frame: DeckFrame) -> u64 {
    let provider_track_id = if frame.simulator_signature == 0 {
        frame.rekordbox_id
    } else {
        frame.simulator_signature
    };
    (u64::from(frame.source_player) << 40)
        | (u64::from(frame.source_slot) << 32)
        | u64::from(provider_track_id)
}

fn duration_beats(frame: DeckFrame) -> Result<u32, BltMidiError> {
    let estimated = u64::from(frame.duration_seconds)
        .checked_mul(u64::from(frame.track_bpm_milli))
        .ok_or(BltMidiError::FrameValueOverflow)?
        .checked_add(30_000)
        .ok_or(BltMidiError::FrameValueOverflow)?
        / 60_000;
    let estimated = u32::try_from(estimated).map_err(|_| BltMidiError::FrameValueOverflow)?;
    Ok(estimated.max(frame.beat.saturating_add(1)).max(1))
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BltMidiError {
    #[error("BLT MIDI frame is incomplete or out of range")]
    IncompleteFrame,
    #[error("BLT MIDI frame value overflow")]
    FrameValueOverflow,
    #[error("BLT MIDI source sequence overflow")]
    SequenceOverflow,
    #[error("BLT MIDI track-load identity overflow")]
    TrackLoadIdOverflow,
    #[error("BLT MIDI committed load disappeared before event emission")]
    LoadedDeckMissing,
    #[error("BLT MIDI transient track metadata is invalid: {0}")]
    Track(#[from] lumi_domain::TrackValidationError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cc(channel: u8, controller: u8, value: u8) -> MidiChannelVoiceMessage {
        MidiChannelVoiceMessage {
            status: CONTROL_CHANGE_STATUS,
            channel,
            data_one: controller,
            data_two: value,
        }
    }

    fn encode(channel: u8, frame: DeckFrame) -> Vec<MidiChannelVoiceMessage> {
        let mut messages = vec![cc(channel, FLAGS_CC, flags(frame))];
        extend_chunks(
            &mut messages,
            channel,
            REKORDBOX_ID_CC,
            frame.rekordbox_id,
            4,
        );
        messages.push(cc(channel, SOURCE_PLAYER_CC, frame.source_player));
        messages.push(cc(channel, SOURCE_SLOT_CC, frame.source_slot));
        extend_chunks(
            &mut messages,
            channel,
            BPM_MILLI_CC,
            frame.track_bpm_milli,
            3,
        );
        extend_chunks(&mut messages, channel, BEAT_CC, frame.beat, 3);
        extend_chunks(
            &mut messages,
            channel,
            DURATION_SECONDS_CC,
            frame.duration_seconds,
            3,
        );
        messages.push(cc(channel, FRAME_SEQUENCE_CC, frame.frame_sequence));
        extend_chunks(
            &mut messages,
            channel,
            EFFECTIVE_BPM_MILLI_CC,
            frame.effective_bpm_milli,
            3,
        );
        extend_chunks(
            &mut messages,
            channel,
            SIMULATOR_SIGNATURE_CC,
            frame.simulator_signature,
            5,
        );
        extend_chunks(
            &mut messages,
            channel,
            PLAYBACK_POSITION_MILLIS_CC,
            frame.playback_position_millis.unwrap_or_default(),
            3,
        );
        messages.push(cc(channel, COMMIT_CC, PROTOCOL_VERSION));
        messages
    }

    fn extend_chunks(
        messages: &mut Vec<MidiChannelVoiceMessage>,
        channel: u8,
        first_cc: u8,
        value: u32,
        count: u8,
    ) {
        for index in 0..count {
            messages.push(cc(
                channel,
                first_cc + index,
                u8::try_from((value >> (u32::from(index) * 7)) & 0x7f)
                    .unwrap_or_else(|error| panic!("test chunk must fit: {error}")),
            ));
        }
    }

    fn flags(frame: DeckFrame) -> u8 {
        (if frame.loaded { FLAG_LOADED } else { 0 })
            | (if frame.playing { FLAG_PLAYING } else { 0 })
            | (if frame.master { FLAG_MASTER } else { 0 })
            | (if frame.on_air { FLAG_ON_AIR } else { 0 })
            | (if frame.playback_position_millis.is_some() {
                FLAG_POSITION_KNOWN
            } else {
                0
            })
    }

    fn frame(sequence: u8) -> DeckFrame {
        DeckFrame {
            loaded: true,
            playing: true,
            master: true,
            on_air: true,
            rekordbox_id: 42,
            source_player: 1,
            source_slot: 2,
            track_bpm_milli: 130_000,
            effective_bpm_milli: 130_000,
            beat: 169,
            duration_seconds: 430,
            frame_sequence: sequence,
            simulator_signature: 0,
            playback_position_millis: Some(74_250),
        }
    }

    #[test]
    fn complete_frame_becomes_normalized_deck_events() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        for message in encode(1, frame(1)) {
            provider
                .ingest(message, MonotonicTime::new(10))
                .unwrap_or_else(|error| panic!("frame must ingest: {error}"));
        }
        let events = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("events must drain: {error}"));
        assert_eq!(events.len(), 6);
        assert_eq!(provider.leader_deck_id(), Some(DeckId::new(1)));
        assert_eq!(provider.diagnostics().committed_frame_count, 1);
    }

    #[test]
    fn duplicate_commit_and_foreign_midi_are_ignored() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        for message in encode(2, frame(9)) {
            provider
                .ingest(message, MonotonicTime::new(10))
                .unwrap_or_else(|error| panic!("frame must ingest: {error}"));
        }
        for message in encode(2, frame(9)) {
            provider
                .ingest(message, MonotonicTime::new(11))
                .unwrap_or_else(|error| panic!("duplicate must be harmless: {error}"));
        }
        provider
            .ingest(
                MidiChannelVoiceMessage {
                    status: 0x9,
                    channel: 16,
                    data_one: 60,
                    data_two: 127,
                },
                MonotonicTime::new(12),
            )
            .unwrap_or_else(|error| panic!("foreign MIDI must be harmless: {error}"));
        assert_eq!(provider.diagnostics().duplicate_frame_count, 1);
        assert_eq!(provider.diagnostics().ignored_message_count, 1);
    }

    #[test]
    fn pitch_change_emits_effective_tempo_without_reloading_track_metadata() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        for message in encode(1, frame(1)) {
            provider
                .ingest(message, MonotonicTime::new(10))
                .unwrap_or_else(|error| panic!("initial frame must ingest: {error}"));
        }
        let _ = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("initial events must drain: {error}"));

        let mut changed = frame(2);
        changed.effective_bpm_milli = 131_300;
        for message in encode(1, changed) {
            provider
                .ingest(message, MonotonicTime::new(20))
                .unwrap_or_else(|error| panic!("tempo frame must ingest: {error}"));
        }
        let events = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("tempo events must drain: {error}"));
        assert!(events.iter().any(|event| matches!(
            event,
            DomainEvent::Observation(ObservationEnvelope {
                observation: DeckObservation::PlaybackTempoChanged {
                    bpm_milli: 131_300,
                    ..
                },
                ..
            })
        )));
        assert!(!events.iter().any(|event| matches!(
            event,
            DomainEvent::Observation(ObservationEnvelope {
                observation: DeckObservation::TrackLoaded { .. },
                ..
            })
        )));
    }

    #[test]
    fn simulator_signature_is_preserved_as_media_identity() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        let mut simulated = frame(1);
        simulated.simulator_signature = 3_456_789_012;
        for message in encode(1, simulated) {
            provider
                .ingest(message, MonotonicTime::new(10))
                .unwrap_or_else(|error| panic!("simulation frame must ingest: {error}"));
        }
        let events = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("events must drain: {error}"));
        let track_load_id = events
            .iter()
            .find_map(|event| match event {
                DomainEvent::Observation(ObservationEnvelope {
                    observation: DeckObservation::TrackLoaded { track_load_id, .. },
                    ..
                }) => Some(*track_load_id),
                _ => None,
            })
            .unwrap_or_else(|| panic!("simulation must emit a track load"));
        let identity = provider
            .track_identity(track_load_id)
            .unwrap_or_else(|| panic!("loaded track must keep its media identity"));
        assert_eq!(identity.rekordbox_id, 42);
        assert_eq!(identity.source_player, 1);
        assert_eq!(identity.source_slot, 2);
        assert_eq!(identity.simulator_signature, 3_456_789_012);
        assert_eq!(
            provider.transport(track_load_id),
            Some(BltTransportSnapshot {
                position_millis: Some(74_250),
                effective_bpm_milli: 130_000,
                playing: true,
            })
        );
    }

    #[test]
    fn regressing_beat_is_emitted_as_an_authoritative_seek() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        for message in encode(1, frame(1)) {
            provider
                .ingest(message, MonotonicTime::new(10))
                .unwrap_or_else(|error| panic!("initial frame must ingest: {error}"));
        }
        let _ = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("initial events must drain: {error}"));

        let mut seeked = frame(2);
        seeked.beat = 64;
        seeked.playback_position_millis = Some(29_500);
        for message in encode(1, seeked) {
            provider
                .ingest(message, MonotonicTime::new(20))
                .unwrap_or_else(|error| panic!("seek frame must ingest: {error}"));
        }
        let events = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("seek events must drain: {error}"));
        assert!(events.iter().any(|event| matches!(
            event,
            DomainEvent::Observation(ObservationEnvelope {
                observation: DeckObservation::PlaybackPositionSeeked { beat: 64, .. },
                ..
            })
        )));
        assert_eq!(
            provider
                .transport(TrackLoadId::new(1))
                .and_then(|transport| transport.position_millis),
            Some(29_500)
        );
    }

    #[test]
    fn missing_heartbeat_unloads_only_after_the_stale_timeout() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        for message in encode(1, frame(1)) {
            provider
                .ingest(message, MonotonicTime::new(10))
                .unwrap_or_else(|error| panic!("initial frame must ingest: {error}"));
        }
        let _ = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("initial events must drain: {error}"));
        let received_at = provider.last_frame_received_at[0]
            .unwrap_or_else(|| panic!("committed frame must record its arrival time"));
        let timeout = Duration::from_millis(2_500);

        provider
            .expire_stale(
                received_at + Duration::from_millis(2_500),
                timeout,
                MonotonicTime::new(20),
            )
            .unwrap_or_else(|error| panic!("fresh heartbeat must remain loaded: {error}"));
        assert!(provider.track_identity(TrackLoadId::new(1)).is_some());
        assert!(
            provider
                .drain_events()
                .unwrap_or_else(|error| panic!("fresh events must drain: {error}"))
                .is_empty()
        );

        provider
            .expire_stale(
                received_at + Duration::from_millis(2_501),
                timeout,
                MonotonicTime::new(21),
            )
            .unwrap_or_else(|error| panic!("stale heartbeat must unload cleanly: {error}"));
        assert!(provider.track_identity(TrackLoadId::new(1)).is_none());
        assert_eq!(provider.leader_deck_id(), None);
        assert!(
            provider
                .drain_events()
                .unwrap_or_else(|error| panic!("stale events must drain: {error}"))
                .iter()
                .any(|event| matches!(
                    event,
                    DomainEvent::Observation(ObservationEnvelope {
                        observation: DeckObservation::TrackUnloaded {
                            deck_id,
                            track_load_id,
                        },
                        ..
                    }) if *deck_id == DeckId::new(1) && *track_load_id == TrackLoadId::new(1)
                ))
        );
    }

    #[test]
    fn partial_frame_never_changes_domain_state() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        provider
            .ingest(cc(1, FLAGS_CC, FLAG_LOADED), MonotonicTime::new(1))
            .unwrap_or_else(|error| panic!("partial field must ingest: {error}"));
        assert_eq!(
            provider
                .drain_events()
                .unwrap_or_else(|error| panic!("events must drain: {error}"))
                .len(),
            1
        );
        assert_eq!(provider.diagnostics().committed_frame_count, 0);
    }

    #[test]
    fn malformed_commit_is_counted_and_fails_silent() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        provider
            .ingest(cc(1, FLAGS_CC, FLAG_LOADED), MonotonicTime::new(1))
            .unwrap_or_else(|error| panic!("field must ingest: {error}"));
        provider
            .ingest(cc(1, COMMIT_CC, PROTOCOL_VERSION), MonotonicTime::new(2))
            .unwrap_or_else(|error| panic!("malformed frame must fail silent: {error}"));
        assert_eq!(provider.diagnostics().committed_frame_count, 0);
        assert_eq!(provider.diagnostics().ignored_message_count, 1);
    }

    #[test]
    fn a_new_frame_cannot_reuse_stale_fields_from_the_previous_commit() {
        let mut provider = BltMidiDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        for message in encode(1, frame(1)) {
            provider
                .ingest(message, MonotonicTime::new(1))
                .unwrap_or_else(|error| panic!("first frame must ingest: {error}"));
        }
        provider
            .ingest(cc(1, FLAGS_CC, FLAG_LOADED), MonotonicTime::new(2))
            .unwrap_or_else(|error| panic!("partial field must ingest: {error}"));
        provider
            .ingest(cc(1, COMMIT_CC, PROTOCOL_VERSION), MonotonicTime::new(2))
            .unwrap_or_else(|error| panic!("partial commit must fail silent: {error}"));

        assert_eq!(provider.diagnostics().committed_frame_count, 1);
        assert_eq!(provider.diagnostics().ignored_message_count, 1);
    }
}
