use std::collections::BTreeMap;

use lumi_deck_source::DeckSourceProvider;
use lumi_domain::{
    DeckId, DeckObservation, DeckSourceStatus, DomainEvent, KeyMode, MonotonicTime, MusicalKey,
    ObservationEnvelope, PitchClass, SourceId, SourceSequence, TrackId, TrackLoadId, TrackMetadata,
};
use thiserror::Error;

use crate::{BridgeEvent, BridgeMessage, SourceCondition};

const SOURCE_ID: u64 = 31;
const DEFAULT_TRACK_MINUTES: u32 = 10;

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoadedDeck {
    identity: ProLinkTrackIdentity,
    track_load_id: TrackLoadId,
    beat: u32,
    effective_bpm_milli: u32,
    playing: bool,
}

pub struct ProLinkDeckSourceProvider {
    source_id: SourceId,
    sequence: u64,
    next_track_load_id: u64,
    decks: BTreeMap<DeckId, LoadedDeck>,
    devices: BTreeMap<u8, String>,
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
                self.devices
                    .insert(device.device_number, device.device_name);
            }
            BridgeEvent::DeviceLost(device) => {
                self.devices.remove(&device.device_number);
                self.unload_deck(DeckId::new(device.device_number), at)?;
            }
            BridgeEvent::DeckStatus(status) => self.apply_status(status, at)?,
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
            BridgeEvent::Beat(_) | BridgeEvent::TrackMetadata(_) => {
                // Beat packets are the low-latency timing stream. Deck status
                // remains the atomic source of track and transport identity;
                // metadata hydration is handled by Lumi's USB/library mirror.
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
            })
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
        }
    }

    pub fn clear(&mut self, at: MonotonicTime) -> Result<(), ProLinkProviderError> {
        let deck_ids = self.decks.keys().copied().collect::<Vec<_>>();
        for deck_id in deck_ids {
            self.unload_deck(deck_id, at)?;
        }
        self.leader_deck_id = None;
        Ok(())
    }

    fn apply_status(
        &mut self,
        status: crate::DeckStatus,
        at: MonotonicTime,
    ) -> Result<(), ProLinkProviderError> {
        let deck_id = DeckId::new(status.device_number);
        if status.rekordbox_id == 0 || status.source_player == 0 {
            self.unload_deck(deck_id, at)?;
            return Ok(());
        }
        let track_bpm_milli = bpm_milli(status.track_bpm)?;
        let effective_bpm_milli = bpm_milli(status.effective_bpm)?;
        let beat = status.beat_number.saturating_sub(1);
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
        if needs_load {
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
                    beat,
                    effective_bpm_milli,
                    playing: status.playing,
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
                self.emit(
                    at,
                    DeckObservation::PlaybackTempoChanged {
                        deck_id,
                        track_load_id: previous.track_load_id,
                        bpm_milli: effective_bpm_milli,
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
            if previous.playing != status.playing {
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
                deck.beat = beat;
                deck.effective_bpm_milli = effective_bpm_milli;
                deck.playing = status.playing;
            }
        }
        if status.tempo_master && self.leader_deck_id != Some(deck_id) {
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
    pub discovered_devices: BTreeMap<u8, String>,
    pub last_error: Option<String>,
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

#[derive(Debug, Error)]
pub enum ProLinkProviderError {
    #[error("Pro DJ Link source sequence overflow")]
    SequenceOverflow,
    #[error("Pro DJ Link track-load identity overflow")]
    TrackLoadIdOverflow,
    #[error("Pro DJ Link loaded deck state disappeared")]
    LoadedDeckMissing,
    #[error("invalid Pro DJ Link BPM {0}")]
    InvalidBpm(f64),
    #[error("invalid Pro DJ Link track metadata: {0}")]
    InvalidTrack(#[from] lumi_domain::TrackValidationError),
}
