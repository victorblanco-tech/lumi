use lumi_domain::{KeyMode, MusicalKey, PitchClass, TrackId};
use lumi_library::{
    LumiPhraseTimeline, PhraseInstance, PhraseRoleId, SourceRevision, TimelineRevision,
    TimelineRevisionOrigin, TimelineValidationError, TrackPageRequest,
};

#[test]
fn page_requests_are_bounded() {
    assert!(TrackPageRequest::try_new(0, 1).is_ok());
    assert!(TrackPageRequest::try_new(0, 200).is_ok());
    assert!(TrackPageRequest::try_new(0, 0).is_err());
    assert!(TrackPageRequest::try_new(0, 201).is_err());
}

#[test]
fn timeline_rejects_gaps_and_preserves_complete_bar_coverage()
-> Result<(), Box<dyn std::error::Error>> {
    let role = PhraseRoleId::try_new("breakdown-1")?;
    let valid = LumiPhraseTimeline::try_new(
        TrackId::new(1),
        TimelineRevision::initial(),
        SourceRevision::try_new("baseline-1")?,
        8,
        TimelineRevisionOrigin::UserEdit,
        vec![
            PhraseInstance::new(0, 0, 4, role.clone()),
            PhraseInstance::new(1, 4, 8, role.clone()),
        ],
    );
    assert!(valid.is_ok());

    let invalid = LumiPhraseTimeline::try_new(
        TrackId::new(1),
        TimelineRevision::initial(),
        SourceRevision::try_new("baseline-1")?,
        8,
        TimelineRevisionOrigin::UserEdit,
        vec![
            PhraseInstance::new(0, 0, 3, role.clone()),
            PhraseInstance::new(1, 4, 8, role),
        ],
    );
    assert_eq!(invalid, Err(TimelineValidationError::InvalidPhraseCoverage));
    Ok(())
}

#[test]
fn domain_key_remains_provider_neutral() {
    let key = MusicalKey::new(PitchClass::A, KeyMode::Minor);
    assert_eq!(key.pitch_class(), PitchClass::A);
    assert_eq!(key.mode(), KeyMode::Minor);
}
