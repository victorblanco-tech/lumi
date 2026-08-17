use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{
    DeckId, DeckObservation, DeckSourceStatus, DomainEvent, KeyMode, MonotonicTime, MusicalKey,
    ObservationEnvelope, PitchClass, SourceId, SourceSequence, TrackId, TrackLoadId, TrackMetadata,
};
use thiserror::Error;

use crate::{BridgeEvent, BridgeMessage, SourceCondition};

const SOURCE_ID: u64 = 31;
const DEFAULT_TRACK_MINUTES: u32 = 10;
const POSITION_CONTINUITY_TOLERANCE_BEATS: f64 = 2.25;
const POSITION_AUTHORITY_DIAGNOSTIC_MAX_AGE: Duration = Duration::from_millis(500);
const PRECISE_POSITION_SEEK_TOLERANCE_MILLIS: u64 = 750;
const PRECISE_POSITION_DISCONTINUITY_CONFIRMATIONS: u8 = 3;
const PRECISE_POSITION_HOT_CUE_CONFIRMATIONS: u8 = 2;
const PRECISE_POSITION_STATUS_TOLERANCE_BEATS: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProLinkTrackIdentity {
    pub rekordbox_id: u32,
    pub source_player: u8,
    pub source_slot: String,
    pub signature: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProLinkTransportSnapshot {
    pub beat: u32,
    pub effective_bpm_milli: u32,
    pub playing: bool,
    pub discontinuity_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProLinkTimingObservation {
    pub deck_id: DeckId,
    pub observed_at_nanos: u64,
    pub absolute_beat: u32,
    pub effective_bpm_milli: u32,
    pub beat_within_bar: u8,
    pub playing: bool,
    pub generation: u64,
    pub discontinuity: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProLinkPrecisePositionObservation {
    pub deck_id: DeckId,
    pub track_load_id: TrackLoadId,
    pub observed_at_nanos: u64,
    pub playback_position_millis: u64,
    pub effective_bpm_milli: u32,
    pub beat_within_bar: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProLinkAuthoritativePosition {
    pub deck_id: DeckId,
    pub track_load_id: TrackLoadId,
    pub absolute_beat: u32,
    pub effective_bpm_milli: u32,
    pub playing: bool,
    pub generation: u64,
    pub discontinuity: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoadedDeck {
    identity: ProLinkTrackIdentity,
    track_load_id: TrackLoadId,
    metadata: TrackMetadata,
    beat: u32,
    last_status_beat: u32,
    phrase_index: Option<u16>,
    precise_position_seen: bool,
    last_precise_position_millis: Option<u64>,
    last_precise_position_observed_at_nanos: Option<u64>,
    pending_precise_discontinuity: Option<PendingPreciseDiscontinuity>,
    last_position_observed_at_nanos: u64,
    effective_bpm_milli: u32,
    playing: bool,
    discontinuity_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingPreciseDiscontinuity {
    playback_position_millis: u64,
    absolute_beat: u32,
    observed_at_nanos: u64,
    confirmation_count: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrecisePositionCandidate {
    playback_position_millis: u64,
    absolute_beat: u32,
    observed_at_nanos: u64,
    last_status_beat: u32,
    known_hot_cue_target: bool,
    track_bpm_milli: u32,
    effective_bpm_milli: u32,
    playing: bool,
}

pub struct ProLinkDeckSourceProvider {
    source_id: SourceId,
    sequence: u64,
    next_track_load_id: u64,
    decks: BTreeMap<DeckId, LoadedDeck>,
    devices: BTreeMap<u8, ProLinkDiscoveredDevice>,
    signatures: BTreeMap<u8, Option<String>>,
    leader_deck_id: Option<DeckId>,
    pending_events: Vec<DomainEvent>,
    source_status: DeckSourceStatus,
    received_message_count: u64,
    ignored_message_count: u64,
    last_bridge_sequence: Option<u64>,
    bridge_version: Option<String>,
    beat_link_version: Option<String>,
    last_error: Option<String>,
    ingress_queue_capacity: usize,
    ingress_queue_depth: usize,
    ingress_queue_high_water: usize,
    ingress_coalesced_message_count: u64,
    ingress_critical_saturation_count: u64,
    precise_position_message_count: u64,
    authoritative_position_count: u64,
    position_discontinuity_count: u64,
    last_precise_position_received_at: Option<Instant>,
    timing_generation: u64,
    timing_observations: Vec<ProLinkTimingObservation>,
    precise_position_observations: Vec<ProLinkPrecisePositionObservation>,
}

impl ProLinkDeckSourceProvider {
    pub fn new(at: MonotonicTime) -> Result<Self, ProLinkProviderError> {
        let mut provider = Self {
            source_id: SourceId::new(SOURCE_ID),
            sequence: 0,
            next_track_load_id: 1,
            decks: BTreeMap::new(),
            devices: BTreeMap::new(),
            signatures: BTreeMap::new(),
            leader_deck_id: None,
            pending_events: Vec::new(),
            source_status: DeckSourceStatus::Starting,
            received_message_count: 0,
            ignored_message_count: 0,
            last_bridge_sequence: None,
            bridge_version: None,
            beat_link_version: None,
            last_error: None,
            ingress_queue_capacity: 0,
            ingress_queue_depth: 0,
            ingress_queue_high_water: 0,
            ingress_coalesced_message_count: 0,
            ingress_critical_saturation_count: 0,
            precise_position_message_count: 0,
            authoritative_position_count: 0,
            position_discontinuity_count: 0,
            last_precise_position_received_at: None,
            timing_generation: 0,
            timing_observations: Vec::new(),
            precise_position_observations: Vec::new(),
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
        message: BridgeMessage,
        at: MonotonicTime,
    ) -> Result<(), ProLinkProviderError> {
        self.received_message_count = self.received_message_count.saturating_add(1);
        self.last_bridge_sequence = Some(message.sequence);
        let observed_at_nanos = message.observed_at_nanos;
        match message.event {
            BridgeEvent::Hello(hello) => {
                self.bridge_version = Some(hello.bridge_version);
                self.beat_link_version = Some(hello.beat_link_version);
            }
            BridgeEvent::SourceStatus(status) => {
                let status = source_status(status.status);
                self.update_source_status(status, at)?;
            }
            BridgeEvent::DeviceFound(device) => {
                self.devices.insert(
                    device.device_number,
                    ProLinkDiscoveredDevice {
                        name: device.device_name,
                        address: device.address,
                    },
                );
            }
            BridgeEvent::DeviceLost(device) => {
                self.devices.remove(&device.device_number);
                self.unload_deck(DeckId::new(device.device_number), at)?;
            }
            BridgeEvent::DeckStatus(status) => {
                self.apply_status(status, observed_at_nanos, at)?;
            }
            BridgeEvent::TrackSignature(signature) => {
                self.signatures
                    .insert(signature.deck_number, signature.signature.clone());
                if let Some(deck) = self.decks.get_mut(&DeckId::new(signature.deck_number)) {
                    deck.identity.signature = signature.signature;
                }
            }
            BridgeEvent::Error(failure) => {
                self.last_error = Some(format!("{}: {}", failure.operation, failure.message));
                self.update_source_status(DeckSourceStatus::Degraded, at)?;
            }
            BridgeEvent::Beat(beat) => self.apply_beat(beat, observed_at_nanos, at)?,
            BridgeEvent::PrecisePosition(position) => {
                self.queue_precise_position(position, observed_at_nanos)?;
            }
            BridgeEvent::TrackMetadata(_) => {
                // Metadata hydration is handled by Lumi's USB/library mirror.
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn leader_deck_id(&self) -> Option<DeckId> {
        self.leader_deck_id
    }

    #[must_use]
    pub fn track_identity(&self, track_load_id: TrackLoadId) -> Option<&ProLinkTrackIdentity> {
        self.decks
            .values()
            .find(|deck| deck.track_load_id == track_load_id)
            .map(|deck| &deck.identity)
    }

    #[must_use]
    pub fn transport(&self, track_load_id: TrackLoadId) -> Option<ProLinkTransportSnapshot> {
        self.decks
            .values()
            .find(|deck| deck.track_load_id == track_load_id)
            .map(|deck| ProLinkTransportSnapshot {
                beat: deck.beat,
                effective_bpm_milli: deck.effective_bpm_milli,
                playing: deck.playing,
                discontinuity_revision: deck.discontinuity_revision,
            })
    }

    pub fn drain_timing_observations(&mut self) -> Vec<ProLinkTimingObservation> {
        let generation = self.timing_generation;
        std::mem::take(&mut self.timing_observations)
            .into_iter()
            .filter(|observation| observation.generation == generation)
            .collect()
    }

    pub fn drain_precise_position_observations(
        &mut self,
    ) -> Vec<ProLinkPrecisePositionObservation> {
        std::mem::take(&mut self.precise_position_observations)
    }

    /// Applies a position that the engine mapped through the exact local
    /// Rekordbox beat grid. Modern players publish this independently from
    /// their beat packet, so a hotcue can never be mistaken for sequential
    /// playback through an old phrase.
    pub fn apply_authoritative_position(
        &mut self,
        observation: ProLinkPrecisePositionObservation,
        absolute_beat: u32,
        known_hot_cue_target: bool,
        at: MonotonicTime,
    ) -> Result<Option<ProLinkAuthoritativePosition>, ProLinkProviderError> {
        let Some(previous) = self.decks.get(&observation.deck_id).cloned() else {
            return Ok(None);
        };
        if previous.track_load_id != observation.track_load_id {
            return Ok(None);
        }
        let first_authoritative_position = !previous.precise_position_seen;
        let tempo_changed = previous.effective_bpm_milli != observation.effective_bpm_milli;
        let discontinuity_candidate = if first_authoritative_position {
            previous.beat.abs_diff(absolute_beat) > 2
        } else {
            previous.beat.abs_diff(absolute_beat) > 2
                && position_millis_is_discontinuous(
                    previous.last_precise_position_millis,
                    observation.playback_position_millis,
                    previous.last_precise_position_observed_at_nanos,
                    observation.observed_at_nanos,
                    previous.metadata.bpm_milli(),
                    observation.effective_bpm_milli,
                    previous.playing,
                )
        };
        let seeked = if discontinuity_candidate && !first_authoritative_position {
            let pending = confirmed_precise_discontinuity(
                previous.pending_precise_discontinuity,
                PrecisePositionCandidate {
                    playback_position_millis: observation.playback_position_millis,
                    absolute_beat,
                    observed_at_nanos: observation.observed_at_nanos,
                    last_status_beat: previous.last_status_beat,
                    known_hot_cue_target,
                    track_bpm_milli: previous.metadata.bpm_milli(),
                    effective_bpm_milli: observation.effective_bpm_milli,
                    playing: previous.playing,
                },
            );
            let corroborated_by_status = pending.absolute_beat.abs_diff(previous.last_status_beat)
                <= PRECISE_POSITION_STATUS_TOLERANCE_BEATS;
            let required_confirmations = if known_hot_cue_target {
                PRECISE_POSITION_HOT_CUE_CONFIRMATIONS
            } else {
                PRECISE_POSITION_DISCONTINUITY_CONFIRMATIONS
            };
            if pending.confirmation_count < required_confirmations
                || (!known_hot_cue_target && !corroborated_by_status)
            {
                if let Some(deck) = self.decks.get_mut(&observation.deck_id) {
                    deck.pending_precise_discontinuity = Some(pending);
                }
                // CDJ-3000/1500X precise-position packets are intentionally
                // high frequency, but Beat Link documents that they can be
                // too jittery to steer an Ableton Link clock directly. Never
                // turn one noisy sample into a transport generation. A real
                // hotcue/seek persists on the new timeline. Imported hot-cue
                // targets need two exact packets; arbitrary seeks and loop
                // wraps additionally need the independent CdjStatus beat to
                // corroborate their landing point. This prevents a coherent
                // cluster of precise-position noise from scrubbing the show.
                return Ok(None);
            }
            true
        } else {
            false
        };
        if seeked {
            self.advance_timing_generation()?;
            self.position_discontinuity_count = self.position_discontinuity_count.saturating_add(1);
        }
        self.authoritative_position_count = self.authoritative_position_count.saturating_add(1);
        if previous.beat != absolute_beat {
            self.emit(
                at,
                if seeked {
                    DeckObservation::PlaybackPositionSeeked {
                        deck_id: observation.deck_id,
                        track_load_id: observation.track_load_id,
                        beat: absolute_beat,
                    }
                } else {
                    DeckObservation::PlaybackPosition {
                        deck_id: observation.deck_id,
                        track_load_id: observation.track_load_id,
                        beat: absolute_beat,
                    }
                },
            )?;
        }
        let phrase_index = phrase_index_at(&previous.metadata, absolute_beat)
            .filter(|phrase_index| Some(*phrase_index) != previous.phrase_index);
        if let Some(deck) = self.decks.get_mut(&observation.deck_id) {
            deck.beat = absolute_beat;
            deck.phrase_index = phrase_index.or(previous.phrase_index);
            deck.precise_position_seen = true;
            deck.last_precise_position_millis = Some(observation.playback_position_millis);
            deck.last_precise_position_observed_at_nanos = Some(observation.observed_at_nanos);
            deck.pending_precise_discontinuity = None;
            deck.last_position_observed_at_nanos = observation.observed_at_nanos;
            deck.effective_bpm_milli = observation.effective_bpm_milli;
            if seeked {
                deck.discontinuity_revision = self.timing_generation;
            }
        }
        if let Some(phrase_index) = phrase_index {
            self.emit(
                at,
                DeckObservation::PhraseChanged {
                    deck_id: observation.deck_id,
                    track_load_id: observation.track_load_id,
                    phrase_index,
                },
            )?;
        }
        if first_authoritative_position || seeked || tempo_changed {
            self.timing_observations.push(ProLinkTimingObservation {
                deck_id: observation.deck_id,
                observed_at_nanos: observation.observed_at_nanos,
                absolute_beat,
                effective_bpm_milli: observation.effective_bpm_milli,
                beat_within_bar: observation.beat_within_bar.max(1),
                playing: previous.playing,
                generation: self.timing_generation,
                discontinuity: seeked,
            });
        }
        Ok(Some(ProLinkAuthoritativePosition {
            deck_id: observation.deck_id,
            track_load_id: observation.track_load_id,
            absolute_beat,
            effective_bpm_milli: observation.effective_bpm_milli,
            playing: previous.playing,
            generation: self.timing_generation,
            discontinuity: seeked,
        }))
    }

    /// Replaces provisional network metadata with the exact Lumi Library
    /// track. The next precise beat activates its phrase timeline; hydration
    /// itself never fabricates a musical boundary.
    pub fn hydrate_track_metadata(
        &mut self,
        track_load_id: TrackLoadId,
        metadata: TrackMetadata,
    ) -> bool {
        let Some(deck) = self
            .decks
            .values_mut()
            .find(|deck| deck.track_load_id == track_load_id)
        else {
            return false;
        };
        deck.metadata = metadata;
        deck.phrase_index = None;
        true
    }

    #[must_use]
    pub fn diagnostics(&self) -> ProLinkDeckSourceDiagnostics {
        ProLinkDeckSourceDiagnostics {
            source_status: self.source_status,
            received_message_count: self.received_message_count,
            ignored_message_count: self.ignored_message_count,
            last_bridge_sequence: self.last_bridge_sequence,
            bridge_version: self.bridge_version.clone(),
            beat_link_version: self.beat_link_version.clone(),
            discovered_devices: self.devices.clone(),
            last_error: self.last_error.clone(),
            ingress_queue_capacity: self.ingress_queue_capacity,
            ingress_queue_depth: self.ingress_queue_depth,
            ingress_queue_high_water: self.ingress_queue_high_water,
            ingress_coalesced_message_count: self.ingress_coalesced_message_count,
            ingress_critical_saturation_count: self.ingress_critical_saturation_count,
            precise_position_message_count: self.precise_position_message_count,
            authoritative_position_count: self.authoritative_position_count,
            position_discontinuity_count: self.position_discontinuity_count,
            position_authority_ready: self
                .leader_deck_id
                .and_then(|deck_id| self.decks.get(&deck_id))
                .is_some_and(|deck| deck.precise_position_seen)
                && self
                    .last_precise_position_received_at
                    .is_some_and(|received| {
                        received.elapsed() <= POSITION_AUTHORITY_DIAGNOSTIC_MAX_AGE
                    }),
        }
    }

    pub fn record_ingress_metrics(
        &mut self,
        capacity: usize,
        depth: usize,
        high_water: usize,
        coalesced_message_count: u64,
        critical_saturation_count: u64,
    ) {
        self.ingress_queue_capacity = capacity;
        self.ingress_queue_depth = depth;
        self.ingress_queue_high_water = high_water;
        self.ingress_coalesced_message_count = coalesced_message_count;
        self.ingress_critical_saturation_count = critical_saturation_count;
    }

    pub fn clear(&mut self, at: MonotonicTime) -> Result<(), ProLinkProviderError> {
        let deck_ids = self.decks.keys().copied().collect::<Vec<_>>();
        for deck_id in deck_ids {
            self.unload_deck(deck_id, at)?;
        }
        self.leader_deck_id = None;
        Ok(())
    }

    pub fn mark_degraded(
        &mut self,
        message: impl Into<String>,
        at: MonotonicTime,
    ) -> Result<(), ProLinkProviderError> {
        self.last_error = Some(message.into());
        self.update_source_status(DeckSourceStatus::Degraded, at)
    }

    fn apply_status(
        &mut self,
        status: crate::DeckStatus,
        observed_at_nanos: u64,
        at: MonotonicTime,
    ) -> Result<(), ProLinkProviderError> {
        let deck_id = DeckId::new(status.device_number);
        if status.rekordbox_id == 0 || status.source_player == 0 {
            self.unload_deck(deck_id, at)?;
            return Ok(());
        }
        let track_bpm_milli = bpm_milli(status.track_bpm)?;
        let effective_bpm_milli = bpm_milli(status.effective_bpm)?;
        // Protocol validation guarantees a non-negative beat for loaded
        // tracks. Beat Link's unloaded `-1` sentinel returns above.
        let beat = u32::try_from(status.beat_number)
            .map_err(|_| ProLinkProviderError::InvalidBeatNumber(status.beat_number))?
            .saturating_sub(1);
        let identity = ProLinkTrackIdentity {
            rekordbox_id: status.rekordbox_id,
            source_player: status.source_player,
            source_slot: status.source_slot,
            signature: self
                .signatures
                .get(&status.device_number)
                .cloned()
                .flatten(),
        };
        let needs_load = self
            .decks
            .get(&deck_id)
            .is_none_or(|deck| deck.identity != identity);
        let mut discontinuity = false;
        let mut stopped_timing_changed = false;
        let mut playing_tempo_changed = false;
        if needs_load {
            self.advance_timing_generation()?;
            discontinuity = true;
            stopped_timing_changed = true;
            self.unload_deck(deck_id, at)?;
            let track_load_id = TrackLoadId::new(self.next_track_load_id);
            self.next_track_load_id = self
                .next_track_load_id
                .checked_add(1)
                .ok_or(ProLinkProviderError::TrackLoadIdOverflow)?;
            let duration_beats = placeholder_duration_beats(track_bpm_milli, beat);
            let metadata = TrackMetadata::try_new_unanalyzed(
                TrackId::new(track_identity_key(&identity)),
                format!("External track {}", identity.rekordbox_id),
                status.device_name,
                track_bpm_milli,
                MusicalKey::new(PitchClass::C, KeyMode::Minor),
                duration_beats,
            )?;
            self.decks.insert(
                deck_id,
                LoadedDeck {
                    identity,
                    track_load_id,
                    metadata: metadata.clone(),
                    beat,
                    last_status_beat: beat,
                    phrase_index: None,
                    precise_position_seen: false,
                    last_precise_position_millis: None,
                    last_precise_position_observed_at_nanos: None,
                    pending_precise_discontinuity: None,
                    last_position_observed_at_nanos: observed_at_nanos,
                    effective_bpm_milli,
                    playing: status.playing,
                    discontinuity_revision: self.timing_generation,
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
            self.emit(
                at,
                DeckObservation::PlaybackPosition {
                    deck_id,
                    track_load_id,
                    beat,
                },
            )?;
            if effective_bpm_milli != track_bpm_milli {
                self.emit(
                    at,
                    DeckObservation::PlaybackTempoChanged {
                        deck_id,
                        track_load_id,
                        bpm_milli: effective_bpm_milli,
                    },
                )?;
            }
            self.emit(
                at,
                DeckObservation::PlaybackStateChanged {
                    deck_id,
                    track_load_id,
                    playing: status.playing,
                },
            )?;
        } else if let Some(previous) = self.decks.get(&deck_id).cloned() {
            if previous.effective_bpm_milli != effective_bpm_milli {
                stopped_timing_changed = true;
                playing_tempo_changed = previous.playing && status.playing;
                self.emit(
                    at,
                    DeckObservation::PlaybackTempoChanged {
                        deck_id,
                        track_load_id: previous.track_load_id,
                        bpm_milli: effective_bpm_milli,
                    },
                )?;
            }
            let seeked = !previous.precise_position_seen
                && previous.beat != beat
                && position_is_discontinuous(
                    previous.beat,
                    beat,
                    previous.last_position_observed_at_nanos,
                    observed_at_nanos,
                    previous.effective_bpm_milli,
                    previous.playing,
                );
            if previous.beat != beat {
                if seeked {
                    self.advance_timing_generation()?;
                    discontinuity = true;
                    stopped_timing_changed = true;
                    self.emit(
                        at,
                        DeckObservation::PlaybackPositionSeeked {
                            deck_id,
                            track_load_id: previous.track_load_id,
                            beat,
                        },
                    )?;
                } else if !previous.playing {
                    self.emit(
                        at,
                        DeckObservation::PlaybackPosition {
                            deck_id,
                            track_load_id: previous.track_load_id,
                            beat,
                        },
                    )?;
                }
            }
            if !previous.playing && status.playing {
                self.advance_timing_generation()?;
                discontinuity = true;
            }
            if previous.playing != status.playing {
                stopped_timing_changed = true;
                self.emit(
                    at,
                    DeckObservation::PlaybackStateChanged {
                        deck_id,
                        track_load_id: previous.track_load_id,
                        playing: status.playing,
                    },
                )?;
            }
            if let Some(deck) = self.decks.get_mut(&deck_id) {
                // Playing status frames are not beat-boundary facts and may
                // arrive after a newer precise Beat packet. Never let such a
                // frame rewind the canonical timeline. It may advance the
                // internal baseline after missed Beat packets, while the Live
                // UI and lighting output remain driven by precise beats.
                if !previous.precise_position_seen
                    && (seeked || !previous.playing || beat >= previous.beat)
                {
                    deck.beat = beat;
                }
                deck.last_position_observed_at_nanos = observed_at_nanos;
                deck.last_status_beat = beat;
                deck.effective_bpm_milli = effective_bpm_milli;
                deck.playing = status.playing;
                if discontinuity {
                    deck.discontinuity_revision = self.timing_generation;
                }
            }
        }
        if status.tempo_master && self.leader_deck_id != Some(deck_id) {
            self.advance_timing_generation()?;
            discontinuity = true;
            stopped_timing_changed = true;
            let track_load_id = self
                .decks
                .get(&deck_id)
                .ok_or(ProLinkProviderError::LoadedDeckMissing)?
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
        // Deck status frames describe the latest known beat but are not sent
        // on the beat boundary. Only publish them while transport is stopped,
        // where immediate BPM/hold state matters and phase is not advancing.
        // Playing transport is anchored exclusively by precise Beat packets.
        if status.tempo_master && !status.playing && stopped_timing_changed {
            self.timing_observations.push(ProLinkTimingObservation {
                deck_id,
                observed_at_nanos,
                absolute_beat: beat,
                effective_bpm_milli,
                beat_within_bar: status.beat_within_bar.max(1),
                playing: status.playing,
                generation: self.timing_generation,
                discontinuity,
            });
        } else if status.tempo_master && status.playing && playing_tempo_changed && !discontinuity {
            // CdjStatus arrives more often than a musical beat on slow tracks.
            // A continuous tempo-only anchor lets Ableton Link follow the
            // pitch slider promptly. Carabiner preserves the established Link
            // phase for a continuous generation, so this asynchronous status
            // frame cannot scrub the beat timeline.
            let canonical_beat = self.decks.get(&deck_id).map_or(beat, |deck| deck.beat);
            self.timing_observations.push(ProLinkTimingObservation {
                deck_id,
                observed_at_nanos,
                absolute_beat: canonical_beat,
                effective_bpm_milli,
                beat_within_bar: status.beat_within_bar.max(1),
                playing: true,
                generation: self.timing_generation,
                discontinuity: false,
            });
        }
        Ok(())
    }

    fn queue_precise_position(
        &mut self,
        position: crate::PrecisePosition,
        observed_at_nanos: u64,
    ) -> Result<(), ProLinkProviderError> {
        if !position.tempo_master {
            return Ok(());
        }
        let deck_id = DeckId::new(position.device_number);
        let Some(track_load_id) = self.decks.get(&deck_id).map(|deck| deck.track_load_id) else {
            return Ok(());
        };
        self.precise_position_message_count = self.precise_position_message_count.saturating_add(1);
        self.last_precise_position_received_at = Some(Instant::now());
        self.precise_position_observations
            .push(ProLinkPrecisePositionObservation {
                deck_id,
                track_load_id,
                observed_at_nanos,
                playback_position_millis: position.playback_position_millis,
                effective_bpm_milli: bpm_milli(position.effective_bpm)?,
                beat_within_bar: position.beat_within_bar,
            });
        Ok(())
    }

    fn apply_beat(
        &mut self,
        beat: crate::Beat,
        observed_at_nanos: u64,
        _at: MonotonicTime,
    ) -> Result<(), ProLinkProviderError> {
        if !beat.tempo_master {
            return Ok(());
        }
        let deck_id = DeckId::new(beat.device_number);
        let Some(previous) = self.decks.get(&deck_id).cloned() else {
            return Ok(());
        };
        if let Some(deck) = self.decks.get_mut(&deck_id) {
            deck.effective_bpm_milli = bpm_milli(beat.effective_bpm)?;
        }
        // A Beat packet is exact in time but contains only its position within
        // the bar. It cannot reveal a hotcue or beat-jump absolute position,
        // so it is timing-only and may never move Lumi's track/phrase state.
        self.timing_observations.push(ProLinkTimingObservation {
            deck_id,
            observed_at_nanos,
            absolute_beat: previous.beat,
            effective_bpm_milli: bpm_milli(beat.effective_bpm)?,
            beat_within_bar: beat.beat_within_bar,
            playing: previous.playing,
            generation: self.timing_generation,
            discontinuity: false,
        });
        Ok(())
    }

    fn advance_timing_generation(&mut self) -> Result<(), ProLinkProviderError> {
        self.timing_generation = self
            .timing_generation
            .checked_add(1)
            .ok_or(ProLinkProviderError::TimingGenerationOverflow)?;
        Ok(())
    }

    fn unload_deck(
        &mut self,
        deck_id: DeckId,
        at: MonotonicTime,
    ) -> Result<(), ProLinkProviderError> {
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
        Ok(())
    }

    fn update_source_status(
        &mut self,
        status: DeckSourceStatus,
        at: MonotonicTime,
    ) -> Result<(), ProLinkProviderError> {
        if self.source_status != status {
            self.source_status = status;
            if status == DeckSourceStatus::Ready {
                self.last_error = None;
            }
            self.emit(at, DeckObservation::SourceStatusChanged { status })?;
        }
        Ok(())
    }

    fn emit(
        &mut self,
        at: MonotonicTime,
        observation: DeckObservation,
    ) -> Result<(), ProLinkProviderError> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(ProLinkProviderError::SequenceOverflow)?;
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

impl DeckSourceProvider for ProLinkDeckSourceProvider {
    type Error = ProLinkProviderError;

    fn provider_kind(&self) -> &'static str {
        "directProDjLink"
    }

    fn drain_events(&mut self) -> Result<Vec<DomainEvent>, Self::Error> {
        Ok(std::mem::take(&mut self.pending_events))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProLinkDeckSourceDiagnostics {
    pub source_status: DeckSourceStatus,
    pub received_message_count: u64,
    pub ignored_message_count: u64,
    pub last_bridge_sequence: Option<u64>,
    pub bridge_version: Option<String>,
    pub beat_link_version: Option<String>,
    pub discovered_devices: BTreeMap<u8, ProLinkDiscoveredDevice>,
    pub last_error: Option<String>,
    pub ingress_queue_capacity: usize,
    pub ingress_queue_depth: usize,
    pub ingress_queue_high_water: usize,
    pub ingress_coalesced_message_count: u64,
    pub ingress_critical_saturation_count: u64,
    pub precise_position_message_count: u64,
    pub authoritative_position_count: u64,
    pub position_discontinuity_count: u64,
    pub position_authority_ready: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProLinkDiscoveredDevice {
    pub name: String,
    pub address: String,
}

fn source_status(condition: SourceCondition) -> DeckSourceStatus {
    match condition {
        SourceCondition::Starting | SourceCondition::Discovering => DeckSourceStatus::Starting,
        SourceCondition::Ready => DeckSourceStatus::Ready,
        SourceCondition::Degraded => DeckSourceStatus::Degraded,
        SourceCondition::Stopped => DeckSourceStatus::Disconnected,
    }
}

fn bpm_milli(bpm: f64) -> Result<u32, ProLinkProviderError> {
    let milli = (bpm * 1_000.0).round();
    if !(20_000.0..=300_000.0).contains(&milli) {
        return Err(ProLinkProviderError::InvalidBpm(bpm));
    }
    Ok(milli as u32)
}

fn placeholder_duration_beats(bpm_milli: u32, current_beat: u32) -> u32 {
    let ten_minutes = bpm_milli
        .saturating_mul(DEFAULT_TRACK_MINUTES)
        .saturating_div(1_000);
    ten_minutes.max(current_beat.saturating_add(1))
}

fn track_identity_key(identity: &ProLinkTrackIdentity) -> u64 {
    u64::from(identity.source_player) << 56 | u64::from(identity.rekordbox_id)
}

fn phrase_index_at(metadata: &TrackMetadata, beat: u32) -> Option<u16> {
    metadata
        .phrases()
        .iter()
        .find(|phrase| beat >= phrase.start_beat() && beat < phrase.end_beat())
        .or_else(|| {
            metadata
                .phrases()
                .last()
                .filter(|phrase| beat >= phrase.start_beat())
        })
        .map(|phrase| phrase.index())
}

#[cfg(test)]
fn precise_absolute_beat(
    status_beat: u32,
    previous_precise_beat: Option<u32>,
    beat_within_bar: u8,
) -> u32 {
    let remainder = u32::from(beat_within_bar.saturating_sub(1).min(3));
    let reference =
        previous_precise_beat.map_or(status_beat, |beat| status_beat.max(beat.saturating_add(1)));
    let base = reference
        .saturating_sub(reference % 4)
        .saturating_add(remainder);
    [base.saturating_sub(4), base, base.saturating_add(4)]
        .into_iter()
        .min_by_key(|candidate| candidate.abs_diff(reference))
        .unwrap_or(status_beat)
}

fn position_is_discontinuous(
    previous_beat: u32,
    candidate_beat: u32,
    previous_observed_at_nanos: u64,
    observed_at_nanos: u64,
    effective_bpm_milli: u32,
    playing: bool,
) -> bool {
    if !playing {
        return previous_beat != candidate_beat;
    }
    let elapsed_nanos = observed_at_nanos.saturating_sub(previous_observed_at_nanos);
    let expected_progress =
        elapsed_nanos as f64 * f64::from(effective_bpm_milli) / 1_000.0 / 60_000_000_000.0;
    let expected_beat = f64::from(previous_beat) + expected_progress;
    (f64::from(candidate_beat) - expected_beat).abs() > POSITION_CONTINUITY_TOLERANCE_BEATS
}

fn position_millis_is_discontinuous(
    previous_position_millis: Option<u64>,
    candidate_position_millis: u64,
    previous_observed_at_nanos: Option<u64>,
    observed_at_nanos: u64,
    track_bpm_milli: u32,
    effective_bpm_milli: u32,
    playing: bool,
) -> bool {
    let (Some(previous_position_millis), Some(previous_observed_at_nanos)) =
        (previous_position_millis, previous_observed_at_nanos)
    else {
        return false;
    };
    if !playing {
        return previous_position_millis != candidate_position_millis;
    }
    let elapsed_millis = observed_at_nanos
        .saturating_sub(previous_observed_at_nanos)
        .saturating_div(1_000_000);
    // A delayed UDP/callback update can still move forward while trailing the
    // wall-clock prediction. That is ordinary network jitter, not a seek. An
    // absolute-difference test turned those late-but-monotonic samples into
    // transport generations and needlessly re-anchored Link. Only an actual
    // backward move, or a forward move materially ahead of elapsed time, can
    // be an explicit hotcue/beat-jump/loop discontinuity.
    if candidate_position_millis.saturating_add(PRECISE_POSITION_SEEK_TOLERANCE_MILLIS)
        < previous_position_millis
    {
        return true;
    }
    // Playback position is expressed on the track's original timeline. At a
    // non-zero CDJ pitch it advances faster or slower than wall time. Failing
    // to apply that multiplier makes ordinary playback look like a forward
    // seek after packet coalescing or temporary scheduling pressure.
    let expected_progress = u64::try_from(
        u128::from(elapsed_millis)
            .saturating_mul(u128::from(effective_bpm_milli))
            .checked_div(u128::from(track_bpm_milli.max(1)))
            .unwrap_or_default(),
    )
    .unwrap_or(u64::MAX);
    let expected = previous_position_millis.saturating_add(expected_progress);
    candidate_position_millis > expected.saturating_add(PRECISE_POSITION_SEEK_TOLERANCE_MILLIS)
}

fn confirmed_precise_discontinuity(
    pending: Option<PendingPreciseDiscontinuity>,
    candidate: PrecisePositionCandidate,
) -> PendingPreciseDiscontinuity {
    let confirmation_count = pending
        .filter(|pending| {
            pending.absolute_beat.abs_diff(candidate.absolute_beat) <= 1
                && (candidate.known_hot_cue_target
                    || pending.absolute_beat.abs_diff(candidate.last_status_beat)
                        <= PRECISE_POSITION_STATUS_TOLERANCE_BEATS
                    || candidate.absolute_beat.abs_diff(candidate.last_status_beat)
                        <= PRECISE_POSITION_STATUS_TOLERANCE_BEATS)
                && !position_millis_is_discontinuous(
                    Some(pending.playback_position_millis),
                    candidate.playback_position_millis,
                    Some(pending.observed_at_nanos),
                    candidate.observed_at_nanos,
                    candidate.track_bpm_milli,
                    candidate.effective_bpm_milli,
                    candidate.playing,
                )
        })
        .map_or(1, |pending| pending.confirmation_count.saturating_add(1));
    PendingPreciseDiscontinuity {
        playback_position_millis: candidate.playback_position_millis,
        absolute_beat: candidate.absolute_beat,
        observed_at_nanos: candidate.observed_at_nanos,
        confirmation_count,
    }
}

#[derive(Debug, Error)]
pub enum ProLinkProviderError {
    #[error("Pro DJ Link source sequence overflow")]
    SequenceOverflow,
    #[error("Pro DJ Link track-load identity overflow")]
    TrackLoadIdOverflow,
    #[error("Pro DJ Link timing generation overflow")]
    TimingGenerationOverflow,
    #[error("Pro DJ Link loaded deck state disappeared")]
    LoadedDeckMissing,
    #[error("invalid Pro DJ Link BPM {0}")]
    InvalidBpm(f64),
    #[error("invalid Pro DJ Link beat number {0}")]
    InvalidBeatNumber(i64),
    #[error("invalid Pro DJ Link track metadata: {0}")]
    InvalidTrack(#[from] lumi_domain::TrackValidationError),
}

#[cfg(test)]
mod timing_tests {
    use super::{
        PRECISE_POSITION_DISCONTINUITY_CONFIRMATIONS, PrecisePositionCandidate,
        confirmed_precise_discontinuity, position_is_discontinuous,
        position_millis_is_discontinuous, precise_absolute_beat,
    };

    fn candidate(
        playback_position_millis: u64,
        absolute_beat: u32,
        observed_at_nanos: u64,
        last_status_beat: u32,
    ) -> PrecisePositionCandidate {
        PrecisePositionCandidate {
            playback_position_millis,
            absolute_beat,
            observed_at_nanos,
            last_status_beat,
            known_hot_cue_target: false,
            track_bpm_milli: 155_000,
            effective_bpm_milli: 155_000,
            playing: true,
        }
    }

    #[test]
    fn delayed_status_progress_is_not_misclassified_as_a_seek() {
        assert!(!position_is_discontinuous(
            17,
            23,
            1_000_000_000,
            3_300_000_000,
            155_000,
            true,
        ));
    }

    #[test]
    fn late_status_frame_cannot_rewind_a_playing_timeline() {
        assert!(!position_is_discontinuous(
            23,
            22,
            1_000_000_000,
            1_050_000_000,
            155_000,
            true,
        ));
    }

    #[test]
    fn hotcue_and_loop_wrap_remain_explicit_discontinuities() {
        assert!(position_is_discontinuous(
            23,
            27,
            1_000_000_000,
            1_010_000_000,
            155_000,
            true,
        ));
        assert!(position_is_discontinuous(
            180,
            32,
            1_000_000_000,
            1_010_000_000,
            155_000,
            true,
        ));
        assert!(position_is_discontinuous(
            23,
            24,
            1_000_000_000,
            1_010_000_000,
            155_000,
            false,
        ));
    }

    #[test]
    fn precise_position_discontinuity_needs_a_stable_new_timeline() {
        let first = confirmed_precise_discontinuity(None, candidate(24_000, 64, 1_000_000_000, 64));
        assert_eq!(first.confirmation_count, 1);

        // A packet from the old timeline arriving between jump candidates
        // breaks consensus instead of producing a transport generation.
        let reordered =
            confirmed_precise_discontinuity(Some(first), candidate(40_030, 104, 1_030_000_000, 64));
        assert_eq!(reordered.confirmation_count, 1);

        let second =
            confirmed_precise_discontinuity(Some(first), candidate(24_030, 64, 1_030_000_000, 64));
        let third =
            confirmed_precise_discontinuity(Some(second), candidate(24_060, 64, 1_060_000_000, 64));
        assert_eq!(second.confirmation_count, 2);
        assert_eq!(
            third.confirmation_count,
            PRECISE_POSITION_DISCONTINUITY_CONFIRMATIONS
        );
    }

    #[test]
    fn arbitrary_position_cluster_waits_for_independent_status_corroboration() {
        let first =
            confirmed_precise_discontinuity(None, candidate(24_000, 64, 1_000_000_000, 120));
        let still_unconfirmed =
            confirmed_precise_discontinuity(Some(first), candidate(24_030, 64, 1_030_000_000, 120));
        assert_eq!(still_unconfirmed.confirmation_count, 1);

        let status_confirmed = confirmed_precise_discontinuity(
            Some(still_unconfirmed),
            candidate(24_060, 64, 1_060_000_000, 64),
        );
        let ready = confirmed_precise_discontinuity(
            Some(status_confirmed),
            candidate(24_090, 64, 1_090_000_000, 64),
        );
        assert_eq!(status_confirmed.confirmation_count, 2);
        assert_eq!(
            ready.confirmation_count,
            PRECISE_POSITION_DISCONTINUITY_CONFIRMATIONS
        );
    }

    #[test]
    fn status_baseline_recovers_after_missed_precise_beats() {
        assert_eq!(precise_absolute_beat(22, Some(17), 4), 23);
    }

    #[test]
    fn delayed_but_forward_precise_position_is_network_jitter_not_a_seek() {
        assert!(!position_millis_is_discontinuous(
            Some(10_000),
            10_400,
            Some(1_000_000_000),
            1_900_000_000,
            155_000,
            155_000,
            true,
        ));
    }

    #[test]
    fn small_reordering_noise_is_not_a_seek() {
        assert!(!position_millis_is_discontinuous(
            Some(10_000),
            9_500,
            Some(1_000_000_000),
            1_030_000_000,
            155_000,
            155_000,
            true,
        ));
    }

    #[test]
    fn hotcue_loop_and_forward_beatjump_are_precise_discontinuities() {
        assert!(position_millis_is_discontinuous(
            Some(40_000),
            24_800,
            Some(1_000_000_000),
            1_030_000_000,
            155_000,
            155_000,
            true,
        ));
        assert!(position_millis_is_discontinuous(
            Some(40_000),
            44_000,
            Some(1_000_000_000),
            1_030_000_000,
            155_000,
            155_000,
            true,
        ));
    }

    #[test]
    fn pitched_playback_progress_is_not_a_forward_seek() {
        assert!(!position_millis_is_discontinuous(
            Some(10_000),
            20_690,
            Some(1_000_000_000),
            11_000_000_000,
            145_000,
            155_000,
            true,
        ));
    }
}
