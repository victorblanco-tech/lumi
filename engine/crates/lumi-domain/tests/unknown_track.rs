use lumi_domain::{KeyMode, MusicalKey, PitchClass, TrackId, TrackMetadata};

#[test]
fn an_unmatched_live_track_is_valid_without_fabricated_phrases() {
    let track = TrackMetadata::try_new_unanalyzed(
        TrackId::new(9001),
        "Unmatched live track".to_owned(),
        "External deck".to_owned(),
        128_000,
        MusicalKey::new(PitchClass::A, KeyMode::Minor),
        256,
    )
    .expect("provider facts should be valid without phrase analysis");

    assert!(track.phrases().is_empty());
    assert!(track.identity_facts().is_none());
    assert_eq!(track.duration_beats(), 256);
}
