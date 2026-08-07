//! Application-owned local audio deck source.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{
    DeckId, DeckObservation, DeckSourceStatus, DomainEvent, MonotonicTime, ObservationEnvelope,
    SourceId, SourceSequence, TrackLoadId, TrackMetadata,
};
use thiserror::Error;

const LOCAL_SOURCE_ID: u64 = 20;

#[derive(Clone, Debug)]
struct LocalDeck {
    track_load_id: TrackLoadId,
    metadata: TrackMetadata,
    beat: u32,
    phrase_index: u16,
    playing: bool,
}

/// Normalizes native local audio transport into the same events as a connected
/// deck adapter. Audio remains owned by the macOS client; this provider owns the
/// authoritative deck/load/execution identity in the engine.
pub struct LocalPlaybackDeckSourceProvider {
    source_id: SourceId,
    decks: BTreeMap<DeckId, LocalDeck>,
    leader_deck_id: Option<DeckId>,
    next_track_load_id: u64,
    sequence: u64,
    pending_events: Vec<DomainEvent>,
}

impl LocalPlaybackDeckSourceProvider {
    pub fn new(at: MonotonicTime) -> Result<Self, LocalPlaybackError> {
        let mut provider = Self {
            source_id: SourceId::new(LOCAL_SOURCE_ID),
            decks: BTreeMap::new(),
            leader_deck_id: None,
            next_track_load_id: 1,
            sequence: 0,
            pending_events: Vec::new(),
        };
        provider.emit(
            at,
            DeckObservation::SourceStatusChanged {
                status: DeckSourceStatus::Ready,
            },
        )?;
        Ok(provider)
    }

    #[must_use]
    pub const fn leader_deck_id(&self) -> Option<DeckId> {
        self.leader_deck_id
    }

    pub fn load_track(
        &mut self,
        deck_id: DeckId,
        metadata: TrackMetadata,
        at: MonotonicTime,
    ) -> Result<TrackLoadId, LocalPlaybackError> {
        validate_deck(deck_id)?;
        if let Some(previous) = self.decks.get(&deck_id) {
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
            .ok_or(LocalPlaybackError::TrackLoadIdOverflow)?;
        self.decks.insert(
            deck_id,
            LocalDeck {
                track_load_id,
                metadata: metadata.clone(),
                beat: 0,
                phrase_index: 0,
                playing: false,
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
        self.emit_position_state(deck_id, at)?;
        if self.leader_deck_id.is_none() {
            self.set_leader(deck_id, at)?;
        }
        Ok(track_load_id)
    }

    pub fn set_leader(
        &mut self,
        deck_id: DeckId,
        at: MonotonicTime,
    ) -> Result<(), LocalPlaybackError> {
        let deck = self
            .decks
            .get(&deck_id)
            .ok_or(LocalPlaybackError::DeckNotLoaded(deck_id))?;
        self.leader_deck_id = Some(deck_id);
        self.emit(
            at,
            DeckObservation::LeaderChanged {
                deck_id,
                track_load_id: deck.track_load_id,
            },
        )
    }

    pub fn update_transport(
        &mut self,
        deck_id: DeckId,
        track_load_id: TrackLoadId,
        beat: u32,
        playing: bool,
        at: MonotonicTime,
    ) -> Result<(), LocalPlaybackError> {
        let (duration, previous_beat, previous_phrase, previous_playing) = {
            let deck = self
                .decks
                .get(&deck_id)
                .ok_or(LocalPlaybackError::DeckNotLoaded(deck_id))?;
            if deck.track_load_id != track_load_id {
                return Err(LocalPlaybackError::StaleTrackLoad {
                    expected: deck.track_load_id,
                    actual: track_load_id,
                });
            }
            (
                deck.metadata.duration_beats(),
                deck.beat,
                deck.phrase_index,
                deck.playing,
            )
        };
        let normalized_beat = beat.min(duration);
        let phrase_index = self
            .decks
            .get(&deck_id)
            .and_then(|deck| {
                deck.metadata
                    .phrases()
                    .iter()
                    .find(|phrase| {
                        normalized_beat >= phrase.start_beat()
                            && normalized_beat < phrase.end_beat()
                    })
                    .or_else(|| deck.metadata.phrases().last())
            })
            .map_or(0, |phrase| phrase.index());
        let normalized_playing = playing && normalized_beat < duration;
        if let Some(deck) = self.decks.get_mut(&deck_id) {
            deck.beat = normalized_beat;
            deck.phrase_index = phrase_index;
            deck.playing = normalized_playing;
        }
        if previous_beat != normalized_beat {
            let observation = if normalized_beat < previous_beat {
                DeckObservation::PlaybackPositionSeeked {
                    deck_id,
                    track_load_id,
                    beat: normalized_beat,
                }
            } else {
                DeckObservation::PlaybackPosition {
                    deck_id,
                    track_load_id,
                    beat: normalized_beat,
                }
            };
            self.emit(at, observation)?;
        }
        if previous_phrase != phrase_index {
            self.emit(
                at,
                DeckObservation::PhraseChanged {
                    deck_id,
                    track_load_id,
                    phrase_index,
                },
            )?;
        }
        if previous_playing != normalized_playing {
            self.emit(
                at,
                DeckObservation::PlaybackStateChanged {
                    deck_id,
                    track_load_id,
                    playing: normalized_playing,
                },
            )?;
        }
        Ok(())
    }

    pub fn clear(&mut self, at: MonotonicTime) -> Result<(), LocalPlaybackError> {
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
        Ok(())
    }

    fn emit_position_state(
        &mut self,
        deck_id: DeckId,
        at: MonotonicTime,
    ) -> Result<(), LocalPlaybackError> {
        let deck = self
            .decks
            .get(&deck_id)
            .ok_or(LocalPlaybackError::DeckNotLoaded(deck_id))?;
        let track_load_id = deck.track_load_id;
        self.emit(
            at,
            DeckObservation::PlaybackPosition {
                deck_id,
                track_load_id,
                beat: 0,
            },
        )?;
        self.emit(
            at,
            DeckObservation::PhraseChanged {
                deck_id,
                track_load_id,
                phrase_index: 0,
            },
        )?;
        self.emit(
            at,
            DeckObservation::PlaybackStateChanged {
                deck_id,
                track_load_id,
                playing: false,
            },
        )
    }

    fn emit(
        &mut self,
        at: MonotonicTime,
        observation: DeckObservation,
    ) -> Result<(), LocalPlaybackError> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(LocalPlaybackError::SequenceOverflow)?;
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

impl DeckSourceProvider for LocalPlaybackDeckSourceProvider {
    type Error = LocalPlaybackError;

    fn provider_kind(&self) -> &'static str {
        "localPlayback"
    }

    fn drain_events(&mut self) -> Result<Vec<DomainEvent>, Self::Error> {
        Ok(std::mem::take(&mut self.pending_events))
    }
}

fn validate_deck(deck_id: DeckId) -> Result<(), LocalPlaybackError> {
    if matches!(deck_id.value(), 1 | 2) {
        Ok(())
    } else {
        Err(LocalPlaybackError::UnknownDeck(deck_id))
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LocalPlaybackError {
    #[error("local playback only exposes Deck 1 and Deck 2")]
    UnknownDeck(DeckId),
    #[error("the selected local deck has no loaded track")]
    DeckNotLoaded(DeckId),
    #[error("the local transport update targets a stale track load")]
    StaleTrackLoad {
        expected: TrackLoadId,
        actual: TrackLoadId,
    },
    #[error("local playback track-load identity overflow")]
    TrackLoadIdOverflow,
    #[error("local playback source sequence overflow")]
    SequenceOverflow,
}

#[cfg(test)]
mod tests {
    use lumi_deck_source::DeckSourceProvider as _;
    use lumi_domain::{
        DeckId, KeyMode, MusicalKey, PhraseKind, PitchClass, TrackId, TrackMetadata, TrackPhrase,
    };

    use super::*;

    #[test]
    fn local_transport_is_empty_until_a_real_library_track_is_loaded() {
        let mut provider = LocalPlaybackDeckSourceProvider::new(MonotonicTime::new(0))
            .unwrap_or_else(|error| panic!("provider must initialize: {error}"));
        let initial = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("status must drain: {error}"));
        assert_eq!(initial.len(), 1);
        let load = provider
            .load_track(DeckId::new(1), track(), MonotonicTime::new(1))
            .unwrap_or_else(|error| panic!("track must load: {error}"));
        provider
            .update_transport(DeckId::new(1), load, 40, true, MonotonicTime::new(2))
            .unwrap_or_else(|error| panic!("transport must update: {error}"));
        let events = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("events must drain: {error}"));
        assert!(events.len() >= 7);

        provider
            .update_transport(DeckId::new(1), load, 8, true, MonotonicTime::new(3))
            .unwrap_or_else(|error| panic!("backward seek must update: {error}"));
        let seek_events = provider
            .drain_events()
            .unwrap_or_else(|error| panic!("seek events must drain: {error}"));
        assert!(seek_events.iter().any(|event| matches!(
            event,
            DomainEvent::Observation(ObservationEnvelope {
                observation: DeckObservation::PlaybackPositionSeeked { beat: 8, .. },
                ..
            })
        )));
    }

    fn track() -> TrackMetadata {
        TrackMetadata::try_new(
            TrackId::new(1),
            "Local Track".to_owned(),
            "Lumi Library".to_owned(),
            120_000,
            MusicalKey::new(PitchClass::C, KeyMode::Minor),
            64,
            vec![
                TrackPhrase::new(0, 0, 32, PhraseKind::Intro),
                TrackPhrase::new(1, 32, 64, PhraseKind::Drop),
            ],
        )
        .unwrap_or_else(|error| panic!("fixture track must be valid: {error}"))
    }
}
