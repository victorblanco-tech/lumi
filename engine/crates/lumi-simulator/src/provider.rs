use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{
    DeckId, DeckObservation, DeckSourceStatus, DomainEvent, MonotonicTime, ObservationEnvelope,
    SourceId, SourceSequence, TrackLoadId, TrackMetadata,
};
use serde::Serialize;

use crate::clock::MonotonicClock;
use crate::fixture::parse;

const DEMO_FIXTURE: &str = include_str!("../../../../fixtures/demo-session-v1/session.json");
const BEAT_DENOMINATOR: u128 = 60_000_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SimulationSpeed {
    #[default]
    One,
    Four,
    Sixteen,
    SixtyFour,
}

impl SimulationSpeed {
    #[must_use]
    pub const fn multiplier(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Four => 4,
            Self::Sixteen => 16,
            Self::SixtyFour => 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationControl {
    SetSpeed(SimulationSpeed),
    Pause,
    Resume,
    Reset,
    AdvanceLeader,
}

#[derive(Clone, Debug)]
struct SimulatedDeck {
    track_load_id: TrackLoadId,
    metadata: TrackMetadata,
    beat: u32,
    phrase_index: u16,
    playing: bool,
}

pub struct SimulatorDeckSourceProvider<C: MonotonicClock> {
    clock: C,
    source_id: SourceId,
    initial_leader_deck_id: DeckId,
    leader_deck_id: DeckId,
    decks: BTreeMap<DeckId, SimulatedDeck>,
    sequence: u64,
    last_clock: MonotonicTime,
    beat_remainder: u128,
    speed: SimulationSpeed,
    paused: bool,
    pending_events: Vec<DomainEvent>,
}

impl<C: MonotonicClock> SimulatorDeckSourceProvider<C> {
    pub fn demo(clock: C) -> Result<Self, SimulatorError> {
        Self::from_fixture_json(DEMO_FIXTURE, clock)
    }

    pub fn from_fixture_json(input: &str, clock: C) -> Result<Self, SimulatorError> {
        let fixture = parse(input)?;
        let now = clock.now();
        let initial_leader_deck_id = fixture.initial_leader_deck_id;
        let mut decks: BTreeMap<DeckId, SimulatedDeck> = fixture
            .decks
            .into_iter()
            .map(|deck| {
                (
                    deck.deck_id,
                    SimulatedDeck {
                        track_load_id: deck.track_load_id,
                        metadata: deck.metadata,
                        beat: 0,
                        phrase_index: 0,
                        playing: false,
                    },
                )
            })
            .collect();
        if let Some(leader) = decks.get_mut(&initial_leader_deck_id) {
            leader.playing = true;
        }
        let mut provider = Self {
            clock,
            source_id: fixture.source_id,
            initial_leader_deck_id,
            leader_deck_id: initial_leader_deck_id,
            decks,
            sequence: 0,
            last_clock: now,
            beat_remainder: 0,
            speed: SimulationSpeed::One,
            paused: false,
            pending_events: Vec::new(),
        };
        provider.queue_initial_events()?;
        Ok(provider)
    }

    #[must_use]
    pub const fn speed(&self) -> SimulationSpeed {
        self.speed
    }

    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    #[must_use]
    pub const fn leader_deck_id(&self) -> DeckId {
        self.leader_deck_id
    }

    pub fn apply_control(&mut self, control: SimulationControl) -> Result<(), SimulatorError> {
        match control {
            SimulationControl::SetSpeed(speed) => {
                self.update_to_clock()?;
                self.speed = speed;
            }
            SimulationControl::Pause => {
                self.update_to_clock()?;
                self.paused = true;
                self.set_leader_playing(false)?;
            }
            SimulationControl::Resume => {
                self.last_clock = self.clock.now();
                self.paused = false;
                self.set_leader_playing(true)?;
            }
            SimulationControl::Reset => self.reset()?,
            SimulationControl::AdvanceLeader => {
                self.update_to_clock()?;
                self.advance_leader()?;
            }
        }
        Ok(())
    }

    /// Replaces the track on one simulated deck and emits the same normalized
    /// `TrackLoaded` observation a real deck-source adapter would publish.
    pub fn load_track(
        &mut self,
        deck_id: DeckId,
        metadata: TrackMetadata,
    ) -> Result<TrackLoadId, SimulatorError> {
        self.update_to_clock()?;
        if !self.decks.contains_key(&deck_id) {
            return Err(SimulatorError::UnknownDeck(deck_id));
        }
        let next_load = self
            .decks
            .values()
            .map(|deck| deck.track_load_id.value())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(SimulatorError::TrackLoadIdOverflow)?;
        let track_load_id = TrackLoadId::new(next_load);
        self.decks.insert(
            deck_id,
            SimulatedDeck {
                track_load_id,
                metadata: metadata.clone(),
                beat: 0,
                phrase_index: 0,
                playing: false,
            },
        );
        if deck_id == self.leader_deck_id {
            self.beat_remainder = 0;
        }
        let now = self.clock.now();
        self.emit(
            now,
            DeckObservation::TrackLoaded {
                deck_id,
                metadata,
                track_load_id,
            },
        )?;
        if deck_id == self.leader_deck_id {
            self.emit(
                now,
                DeckObservation::PlaybackPosition {
                    deck_id,
                    track_load_id,
                    beat: 0,
                },
            )?;
            self.emit(
                now,
                DeckObservation::PhraseChanged {
                    deck_id,
                    track_load_id,
                    phrase_index: 0,
                },
            )?;
        }
        Ok(track_load_id)
    }

    pub fn update_to_clock(&mut self) -> Result<(), SimulatorError> {
        let now = self.clock.now();
        if now < self.last_clock {
            return Err(SimulatorError::ClockRegressed {
                previous: self.last_clock,
                current: now,
            });
        }
        let elapsed = now.ticks() - self.last_clock.ticks();
        self.last_clock = now;
        if self.paused || elapsed == 0 {
            return Ok(());
        }

        let Some(leader) = self.decks.get(&self.leader_deck_id) else {
            return Err(SimulatorError::LeaderDeckMissing);
        };
        let increment = u128::from(elapsed)
            .checked_mul(u128::from(leader.metadata.bpm_milli()))
            .and_then(|value| value.checked_mul(u128::from(self.speed.multiplier())))
            .ok_or(SimulatorError::BeatAccumulatorOverflow)?;
        let accumulator = self
            .beat_remainder
            .checked_add(increment)
            .ok_or(SimulatorError::BeatAccumulatorOverflow)?;
        let whole_beats = accumulator / BEAT_DENOMINATOR;
        self.beat_remainder = accumulator % BEAT_DENOMINATOR;

        let beats_to_emit = match u32::try_from(whole_beats) {
            Ok(beats) => beats,
            Err(_) => return Err(SimulatorError::BeatAccumulatorOverflow),
        };
        for _ in 0..beats_to_emit {
            if !self.advance_one_beat(now)? {
                self.paused = true;
                self.set_leader_playing(false)?;
                break;
            }
        }
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), SimulatorError> {
        self.pending_events.clear();
        self.leader_deck_id = self.initial_leader_deck_id;
        self.speed = SimulationSpeed::One;
        self.beat_remainder = 0;
        self.last_clock = self.clock.now();
        for deck in self.decks.values_mut() {
            deck.beat = 0;
            deck.phrase_index = 0;
            deck.playing = false;
        }
        if let Some(leader) = self.decks.get_mut(&self.initial_leader_deck_id) {
            leader.playing = true;
        }
        self.paused = false;
        self.queue_initial_events()
    }

    pub fn canonical_snapshot(&self) -> SimulatorCanonicalSnapshot {
        SimulatorCanonicalSnapshot {
            provider_kind: "simulator",
            speed: self.speed.multiplier(),
            paused: self.paused,
            leader_deck_id: self.leader_deck_id.value(),
            decks: self
                .decks
                .iter()
                .map(|(deck_id, deck)| SimulatorDeckSnapshot {
                    deck_id: deck_id.value(),
                    track_load_id: deck.track_load_id.value(),
                    track_id: deck.metadata.id().value(),
                    beat: deck.beat,
                    phrase_index: deck.phrase_index,
                })
                .collect(),
        }
    }

    fn queue_initial_events(&mut self) -> Result<(), SimulatorError> {
        let now = self.clock.now();
        self.emit(
            now,
            DeckObservation::SourceStatusChanged {
                status: DeckSourceStatus::Ready,
            },
        )?;
        let decks: Vec<(DeckId, TrackLoadId, TrackMetadata)> = self
            .decks
            .iter()
            .map(|(deck_id, deck)| (*deck_id, deck.track_load_id, deck.metadata.clone()))
            .collect();
        for (deck_id, track_load_id, metadata) in decks {
            self.emit(
                now,
                DeckObservation::TrackLoaded {
                    deck_id,
                    metadata,
                    track_load_id,
                },
            )?;
        }
        let leader = self.leader_snapshot()?;
        self.emit(
            now,
            DeckObservation::LeaderChanged {
                deck_id: self.leader_deck_id,
                track_load_id: leader.track_load_id,
            },
        )?;
        self.emit(
            now,
            DeckObservation::PlaybackPosition {
                deck_id: self.leader_deck_id,
                track_load_id: leader.track_load_id,
                beat: leader.beat,
            },
        )?;
        self.emit(
            now,
            DeckObservation::PhraseChanged {
                deck_id: self.leader_deck_id,
                track_load_id: leader.track_load_id,
                phrase_index: leader.phrase_index,
            },
        )?;
        self.emit(
            now,
            DeckObservation::PlaybackStateChanged {
                deck_id: self.leader_deck_id,
                track_load_id: leader.track_load_id,
                playing: true,
            },
        )
    }

    fn advance_leader(&mut self) -> Result<(), SimulatorError> {
        self.set_leader_playing(false)?;
        let Some(next_deck_id) = self
            .decks
            .keys()
            .copied()
            .find(|deck_id| *deck_id != self.leader_deck_id)
        else {
            return Err(SimulatorError::NextDeckMissing);
        };
        self.leader_deck_id = next_deck_id;
        self.beat_remainder = 0;
        let now = self.clock.now();
        let leader = self.leader_snapshot()?;
        if !self.paused {
            self.set_leader_playing(true)?;
        }
        self.emit(
            now,
            DeckObservation::LeaderChanged {
                deck_id: next_deck_id,
                track_load_id: leader.track_load_id,
            },
        )?;
        self.emit(
            now,
            DeckObservation::PlaybackPosition {
                deck_id: next_deck_id,
                track_load_id: leader.track_load_id,
                beat: leader.beat,
            },
        )?;
        self.emit(
            now,
            DeckObservation::PhraseChanged {
                deck_id: next_deck_id,
                track_load_id: leader.track_load_id,
                phrase_index: leader.phrase_index,
            },
        )
    }

    fn advance_one_beat(&mut self, now: MonotonicTime) -> Result<bool, SimulatorError> {
        let (track_load_id, beat, phrase_change) = {
            let Some(deck) = self.decks.get_mut(&self.leader_deck_id) else {
                return Err(SimulatorError::LeaderDeckMissing);
            };
            if deck.beat >= deck.metadata.duration_beats() {
                return Ok(false);
            }
            deck.beat += 1;
            let phrase_change = deck
                .metadata
                .phrases()
                .iter()
                .find(|phrase| phrase.start_beat() == deck.beat)
                .map(|phrase| phrase.index());
            if let Some(index) = phrase_change {
                deck.phrase_index = index;
            }
            (deck.track_load_id, deck.beat, phrase_change)
        };

        self.emit(
            now,
            DeckObservation::PlaybackPosition {
                deck_id: self.leader_deck_id,
                track_load_id,
                beat,
            },
        )?;
        if let Some(phrase_index) = phrase_change {
            self.emit(
                now,
                DeckObservation::PhraseChanged {
                    deck_id: self.leader_deck_id,
                    track_load_id,
                    phrase_index,
                },
            )?;
        }
        Ok(true)
    }

    fn set_leader_playing(&mut self, playing: bool) -> Result<(), SimulatorError> {
        let now = self.clock.now();
        let Some(deck) = self.decks.get_mut(&self.leader_deck_id) else {
            return Err(SimulatorError::LeaderDeckMissing);
        };
        if deck.playing == playing {
            return Ok(());
        }
        deck.playing = playing;
        let track_load_id = deck.track_load_id;
        self.emit(
            now,
            DeckObservation::PlaybackStateChanged {
                deck_id: self.leader_deck_id,
                track_load_id,
                playing,
            },
        )
    }

    fn leader_snapshot(&self) -> Result<LeaderSnapshot, SimulatorError> {
        self.decks
            .get(&self.leader_deck_id)
            .map(|deck| LeaderSnapshot {
                track_load_id: deck.track_load_id,
                beat: deck.beat,
                phrase_index: deck.phrase_index,
            })
            .ok_or(SimulatorError::LeaderDeckMissing)
    }

    fn emit(
        &mut self,
        observed_at: MonotonicTime,
        observation: DeckObservation,
    ) -> Result<(), SimulatorError> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(SimulatorError::SequenceOverflow)?;
        self.pending_events
            .push(DomainEvent::Observation(ObservationEnvelope {
                source_id: self.source_id,
                sequence: SourceSequence::new(self.sequence),
                observed_at,
                observation,
            }));
        Ok(())
    }
}

impl<C: MonotonicClock> DeckSourceProvider for SimulatorDeckSourceProvider<C> {
    type Error = SimulatorError;

    fn provider_kind(&self) -> &'static str {
        "simulator"
    }

    fn drain_events(&mut self) -> Result<Vec<DomainEvent>, Self::Error> {
        Ok(std::mem::take(&mut self.pending_events))
    }
}

#[derive(Clone, Copy)]
struct LeaderSnapshot {
    track_load_id: TrackLoadId,
    beat: u32,
    phrase_index: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorCanonicalSnapshot {
    provider_kind: &'static str,
    speed: u32,
    paused: bool,
    leader_deck_id: u8,
    decks: Vec<SimulatorDeckSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SimulatorDeckSnapshot {
    deck_id: u8,
    track_load_id: u64,
    track_id: u64,
    beat: u32,
    phrase_index: u16,
}

#[derive(Debug)]
pub enum SimulatorError {
    InvalidFixture(String),
    UnsupportedFixtureVersion(u16),
    ClockRegressed {
        previous: MonotonicTime,
        current: MonotonicTime,
    },
    BeatAccumulatorOverflow,
    SequenceOverflow,
    LeaderDeckMissing,
    NextDeckMissing,
    UnknownDeck(DeckId),
    TrackLoadIdOverflow,
    TranscriptEncoding(serde_json::Error),
}

impl fmt::Display for SimulatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFixture(reason) => write!(formatter, "invalid demo fixture: {reason}"),
            Self::UnsupportedFixtureVersion(version) => {
                write!(formatter, "unsupported demo fixture version {version}")
            }
            Self::ClockRegressed { previous, current } => write!(
                formatter,
                "simulator clock regressed from {} to {}",
                previous.ticks(),
                current.ticks()
            ),
            Self::BeatAccumulatorOverflow => formatter.write_str("beat accumulator overflow"),
            Self::SequenceOverflow => formatter.write_str("source event sequence overflow"),
            Self::LeaderDeckMissing => formatter.write_str("leader deck is missing"),
            Self::NextDeckMissing => formatter.write_str("next deck is missing"),
            Self::UnknownDeck(deck_id) => {
                write!(
                    formatter,
                    "simulator deck {} does not exist",
                    deck_id.value()
                )
            }
            Self::TrackLoadIdOverflow => formatter.write_str("track-load identifier overflow"),
            Self::TranscriptEncoding(error) => {
                write!(formatter, "transcript encoding failed: {error}")
            }
        }
    }
}

impl Error for SimulatorError {}

impl From<serde_json::Error> for SimulatorError {
    fn from(error: serde_json::Error) -> Self {
        Self::TranscriptEncoding(error)
    }
}
