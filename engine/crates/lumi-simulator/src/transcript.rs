use lumi_domain::{
    DeckObservation, DeckSourceStatus, DomainEvent, KeyMode, PhraseKind, PitchClass, TrackColor,
    TrackMetadata,
};
use serde::Serialize;

use crate::provider::SimulatorError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordedEvent {
    sequence: u64,
    monotonic_ticks: u64,
    source_id: u64,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deck_id: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    track_load_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    track: Option<RecordedTrack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    beat: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    playing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phrase_index: Option<u16>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordedTrack {
    id: u64,
    title: String,
    artist: String,
    bpm_milli: u32,
    pitch_class: &'static str,
    key_mode: &'static str,
    color_rgb: Option<u32>,
    duration_beats: u32,
    phrases: Vec<RecordedPhrase>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordedPhrase {
    index: u16,
    start_beat: u32,
    end_beat: u32,
    kind: &'static str,
}

pub fn canonical_transcript(events: &[DomainEvent]) -> Result<Vec<u8>, SimulatorError> {
    let mut lines = Vec::with_capacity(events.len());
    for event in events {
        let DomainEvent::Observation(envelope) = event else {
            continue;
        };
        let mut recorded = RecordedEvent {
            sequence: envelope.sequence.value(),
            monotonic_ticks: envelope.observed_at.ticks(),
            source_id: envelope.source_id.value(),
            kind: observation_kind(&envelope.observation),
            status: None,
            deck_id: None,
            track_load_id: None,
            track: None,
            beat: None,
            playing: None,
            phrase_index: None,
        };
        match &envelope.observation {
            DeckObservation::SourceStatusChanged { status } => {
                recorded.status = Some(source_status_name(*status));
            }
            DeckObservation::TrackLoaded {
                deck_id,
                metadata,
                track_load_id,
            } => {
                recorded.deck_id = Some(deck_id.value());
                recorded.track_load_id = Some(track_load_id.value());
                recorded.track = Some(record_track(metadata));
            }
            DeckObservation::PlaybackPosition {
                deck_id,
                track_load_id,
                beat,
            } => {
                recorded.deck_id = Some(deck_id.value());
                recorded.track_load_id = Some(track_load_id.value());
                recorded.beat = Some(*beat);
            }
            DeckObservation::PlaybackStateChanged {
                deck_id,
                track_load_id,
                playing,
            } => {
                recorded.deck_id = Some(deck_id.value());
                recorded.track_load_id = Some(track_load_id.value());
                recorded.playing = Some(*playing);
            }
            DeckObservation::TrackUnloaded {
                deck_id,
                track_load_id,
            }
            | DeckObservation::LeaderChanged {
                deck_id,
                track_load_id,
            } => {
                recorded.deck_id = Some(deck_id.value());
                recorded.track_load_id = Some(track_load_id.value());
            }
            DeckObservation::PhraseChanged {
                deck_id,
                track_load_id,
                phrase_index,
            } => {
                recorded.deck_id = Some(deck_id.value());
                recorded.track_load_id = Some(track_load_id.value());
                recorded.phrase_index = Some(*phrase_index);
            }
        }
        lines.push(serde_json::to_string(&recorded)?);
    }
    let mut bytes = lines.join("\n").into_bytes();
    bytes.push(b'\n');
    Ok(bytes)
}

const fn observation_kind(observation: &DeckObservation) -> &'static str {
    match observation {
        DeckObservation::SourceStatusChanged { .. } => "sourceStatusChanged",
        DeckObservation::TrackLoaded { .. } => "trackLoaded",
        DeckObservation::PlaybackPosition { .. } => "playbackPosition",
        DeckObservation::PlaybackStateChanged { .. } => "playbackStateChanged",
        DeckObservation::TrackUnloaded { .. } => "trackUnloaded",
        DeckObservation::PhraseChanged { .. } => "phraseChanged",
        DeckObservation::LeaderChanged { .. } => "leaderChanged",
    }
}

fn record_track(metadata: &TrackMetadata) -> RecordedTrack {
    RecordedTrack {
        id: metadata.id().value(),
        title: metadata.title().to_owned(),
        artist: metadata.artist().to_owned(),
        bpm_milli: metadata.bpm_milli(),
        pitch_class: pitch_class_name(metadata.musical_key().pitch_class()),
        key_mode: key_mode_name(metadata.musical_key().mode()),
        color_rgb: metadata.color().map(TrackColor::rgb_u32),
        duration_beats: metadata.duration_beats(),
        phrases: metadata
            .phrases()
            .iter()
            .map(|phrase| RecordedPhrase {
                index: phrase.index(),
                start_beat: phrase.start_beat(),
                end_beat: phrase.end_beat(),
                kind: phrase_kind_name(phrase.kind()),
            })
            .collect(),
    }
}

const fn source_status_name(status: DeckSourceStatus) -> &'static str {
    match status {
        DeckSourceStatus::Starting => "starting",
        DeckSourceStatus::Ready => "ready",
        DeckSourceStatus::Degraded => "degraded",
        DeckSourceStatus::Disconnected => "disconnected",
    }
}

pub(crate) const fn pitch_class_name(pitch_class: PitchClass) -> &'static str {
    match pitch_class {
        PitchClass::C => "c",
        PitchClass::CSharp => "cSharp",
        PitchClass::D => "d",
        PitchClass::DSharp => "dSharp",
        PitchClass::E => "e",
        PitchClass::F => "f",
        PitchClass::FSharp => "fSharp",
        PitchClass::G => "g",
        PitchClass::GSharp => "gSharp",
        PitchClass::A => "a",
        PitchClass::ASharp => "aSharp",
        PitchClass::B => "b",
    }
}

pub(crate) const fn key_mode_name(mode: KeyMode) -> &'static str {
    match mode {
        KeyMode::Major => "major",
        KeyMode::Minor => "minor",
    }
}

const fn phrase_kind_name(kind: PhraseKind) -> &'static str {
    match kind {
        PhraseKind::Intro => "intro",
        PhraseKind::Verse => "verse",
        PhraseKind::Build => "build",
        PhraseKind::Drop => "drop",
        PhraseKind::Breakdown => "breakdown",
        PhraseKind::Outro => "outro",
    }
}
