use super::*;
use lumi_library_demo::DemoLibrarySourceProvider;
use lumi_library_source::MusicLibrarySourceProvider;

fn fixture() -> Result<(SqliteLibraryRepository, Vec<TrackId>), Box<dyn std::error::Error>> {
    let mut repository = SqliteLibraryRepository::in_memory()?;
    repository.import_baseline(&DemoLibrarySourceProvider::curated().load_baseline()?)?;
    let ids = repository
        .page_tracks(TrackPageRequest::try_new(0, 25)?)?
        .tracks()
        .iter()
        .map(|track| track.id())
        .collect();
    Ok((repository, ids))
}

fn alias(track: TrackId, signature: &str) -> DeviceAliasUpsert {
    DeviceAliasUpsert {
        device_track_id: 42,
        simulator_signature: 0,
        canonical_track_id: Some(track),
        audio_signature: signature.into(),
        match_kind: "verified-audio".into(),
        title: "Track".into(),
        artist: "Artist".into(),
        bpm_milli: 140_000,
        duration_millis: 100_000,
        file_size: 123,
        audio_uri: "file://localhost/Volumes/Test/Track.mp3".into(),
        metadata_revision: "metadata-1".into(),
        color_rgb: None,
        master_database_id: 1,
        master_content_id: 42,
        information_update_count: 1,
        analysis_revision: "analysis-1".into(),
        analyzed_at: "2026-10-05".into(),
        sync_disposition: "current".into(),
    }
}

fn sync(
    repository: &mut SqliteLibraryRepository,
    source: &str,
    alias: DeviceAliasUpsert,
) -> Result<(), SqliteLibraryError> {
    repository.sync_device_aliases(
        source,
        source,
        "database-1",
        &mut [alias],
        &[],
        &[],
        &[],
        &[],
    )?;
    Ok(())
}

#[test]
fn exact_usb_identity_disambiguates_colliding_track_ids() -> Result<(), Box<dyn std::error::Error>>
{
    let (mut repository, ids) = fixture()?;
    sync(
        &mut repository,
        "usb:gray",
        alias(ids[0], "audio-full-v1:gray"),
    )?;
    sync(
        &mut repository,
        "usb:chrm",
        alias(ids[1], "audio-full-v1:chrm"),
    )?;
    assert!(repository.resolve_device_alias(42, 0)?.is_none());
    assert_eq!(
        repository
            .resolve_device_alias_for_source("usb:gray", 42)?
            .ok_or("missing GRAY")?
            .canonical_track_id,
        ids[0]
    );
    assert_eq!(
        repository
            .resolve_device_alias_for_source("usb:chrm", 42)?
            .ok_or("missing CHRM")?
            .canonical_track_id,
        ids[1]
    );
    assert!(
        repository
            .resolve_device_alias_for_source("usb:unknown", 42)?
            .is_none()
    );
    assert!(
        repository
            .resolve_device_alias_for_source("usb:gray", 43)?
            .is_none()
    );
    Ok(())
}

#[test]
fn alias_replacement_and_archival_retain_original_audio_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut repository, ids) = fixture()?;
    sync(
        &mut repository,
        "usb:gray",
        alias(ids[0], "audio-full-v1:original"),
    )?;
    sync(
        &mut repository,
        "usb:gray",
        alias(ids[1], "audio-full-v1:new-edit"),
    )?;
    let signatures = repository.device_audio_signatures()?;
    assert!(signatures[&ids[0]].contains("audio-full-v1:original"));
    assert!(signatures[&ids[1]].contains("audio-full-v1:new-edit"));
    assert!(repository.device_audio_uris(ids[0])?.is_empty());
    assert_eq!(repository.device_audio_uris(ids[1])?.len(), 1);
    repository.sync_device_aliases("usb:gray", "GRAY", "empty", &mut [], &[], &[], &[], &[])?;
    assert!(repository.device_audio_signatures()?[&ids[0]].contains("audio-full-v1:original"));
    assert!(
        repository
            .resolve_device_alias_for_source("usb:gray", 42)?
            .is_none()
    );
    Ok(())
}

#[test]
fn schema19_migration_backfills_complete_fingerprints_without_changing_tracks()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut repository, ids) = fixture()?;
    sync(
        &mut repository,
        "usb:gray",
        alias(ids[0], "audio-full-v1:original"),
    )?;
    let before = repository.page_tracks(TrackPageRequest::try_new(0, 25)?)?;
    repository
        .connection
        .execute_batch("DROP TABLE track_audio_fingerprints; PRAGMA user_version = 19;")?;
    repository.migrate()?;
    assert_eq!(repository.schema_version()?, 21);
    assert_eq!(
        repository.page_tracks(TrackPageRequest::try_new(0, 25)?)?,
        before
    );
    assert!(repository.device_audio_signatures()?[&ids[0]].contains("audio-full-v1:original"));
    repository.migrate()?;
    assert_eq!(repository.device_audio_signatures()?[&ids[0]].len(), 1);
    Ok(())
}
