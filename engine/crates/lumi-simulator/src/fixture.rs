use lumi_domain::{
    DeckId, KeyMode, MusicalKey, PhraseKind, PitchClass, SourceId, TrackColor, TrackId,
    TrackLoadId, TrackMetadata, TrackPhrase,
};
use serde::Deserialize;

use crate::provider::SimulatorError;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoFixture {
    schema_version: u16,
    source_id: u64,
    initial_leader_deck_id: u8,
    decks: Vec<DeckFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeckFixture {
    deck_id: u8,
    track_load_id: u64,
    track: TrackFixture,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackFixture {
    id: u64,
    title: String,
    artist: String,
    bpm_milli: u32,
    key: KeyFixture,
    color_rgb: u32,
    duration_beats: u32,
    phrases: Vec<PhraseFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyFixture {
    pitch_class: String,
    mode: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhraseFixture {
    index: u16,
    start_beat: u32,
    end_beat: u32,
    kind: String,
}

pub(crate) struct ParsedFixture {
    pub source_id: SourceId,
    pub initial_leader_deck_id: DeckId,
    pub decks: Vec<ParsedDeck>,
}

#[derive(Clone)]
pub(crate) struct ParsedDeck {
    pub deck_id: DeckId,
    pub track_load_id: TrackLoadId,
    pub metadata: TrackMetadata,
}

pub(crate) fn parse(input: &str) -> Result<ParsedFixture, SimulatorError> {
    let fixture: DemoFixture = serde_json::from_str(input)
        .map_err(|error| SimulatorError::InvalidFixture(error.to_string()))?;
    if fixture.schema_version != 1 {
        return Err(SimulatorError::UnsupportedFixtureVersion(
            fixture.schema_version,
        ));
    }
    if fixture.decks.len() != 2 {
        return Err(SimulatorError::InvalidFixture(
            "the Epic 1 fixture must contain exactly two decks".to_owned(),
        ));
    }

    let mut decks = Vec::with_capacity(fixture.decks.len());
    for deck in fixture.decks {
        let phrases = deck
            .track
            .phrases
            .into_iter()
            .map(|phrase| {
                Ok(TrackPhrase::new(
                    phrase.index,
                    phrase.start_beat,
                    phrase.end_beat,
                    phrase_kind(&phrase.kind)?,
                ))
            })
            .collect::<Result<Vec<_>, SimulatorError>>()?;
        if deck.track.color_rgb > 0x00ff_ffff {
            return Err(SimulatorError::InvalidFixture(
                "track color must be a normalized 24-bit sRGB value".to_owned(),
            ));
        }
        let metadata = TrackMetadata::try_new_with_color(
            TrackId::new(deck.track.id),
            deck.track.title,
            deck.track.artist,
            deck.track.bpm_milli,
            MusicalKey::new(
                pitch_class(&deck.track.key.pitch_class)?,
                key_mode(&deck.track.key.mode)?,
            ),
            Some(TrackColor::from_rgb_u32(deck.track.color_rgb)),
            deck.track.duration_beats,
            phrases,
        )
        .map_err(|error| SimulatorError::InvalidFixture(error.to_string()))?;
        decks.push(ParsedDeck {
            deck_id: DeckId::new(deck.deck_id),
            track_load_id: TrackLoadId::new(deck.track_load_id),
            metadata,
        });
    }

    let initial_leader_deck_id = DeckId::new(fixture.initial_leader_deck_id);
    if !decks
        .iter()
        .any(|deck| deck.deck_id == initial_leader_deck_id)
    {
        return Err(SimulatorError::InvalidFixture(
            "the initial leader deck is not present".to_owned(),
        ));
    }

    Ok(ParsedFixture {
        source_id: SourceId::new(fixture.source_id),
        initial_leader_deck_id,
        decks,
    })
}

fn pitch_class(value: &str) -> Result<PitchClass, SimulatorError> {
    match value {
        "c" => Ok(PitchClass::C),
        "cSharp" => Ok(PitchClass::CSharp),
        "d" => Ok(PitchClass::D),
        "dSharp" => Ok(PitchClass::DSharp),
        "e" => Ok(PitchClass::E),
        "f" => Ok(PitchClass::F),
        "fSharp" => Ok(PitchClass::FSharp),
        "g" => Ok(PitchClass::G),
        "gSharp" => Ok(PitchClass::GSharp),
        "a" => Ok(PitchClass::A),
        "aSharp" => Ok(PitchClass::ASharp),
        "b" => Ok(PitchClass::B),
        _ => Err(SimulatorError::InvalidFixture(format!(
            "unknown pitch class {value}"
        ))),
    }
}

fn key_mode(value: &str) -> Result<KeyMode, SimulatorError> {
    match value {
        "major" => Ok(KeyMode::Major),
        "minor" => Ok(KeyMode::Minor),
        _ => Err(SimulatorError::InvalidFixture(format!(
            "unknown key mode {value}"
        ))),
    }
}

fn phrase_kind(value: &str) -> Result<PhraseKind, SimulatorError> {
    match value {
        "intro" => Ok(PhraseKind::Intro),
        "verse" => Ok(PhraseKind::Verse),
        "build" => Ok(PhraseKind::Build),
        "drop" => Ok(PhraseKind::Drop),
        "breakdown" => Ok(PhraseKind::Breakdown),
        "outro" => Ok(PhraseKind::Outro),
        _ => Err(SimulatorError::InvalidFixture(format!(
            "unknown phrase kind {value}"
        ))),
    }
}
