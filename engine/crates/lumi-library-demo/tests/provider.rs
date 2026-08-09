use lumi_library_demo::{DemoLibraryRevision, DemoLibrarySourceProvider};
use lumi_library_source::MusicLibrarySourceProvider;

#[test]
fn curated_demo_baseline_is_deterministic_and_complete() -> Result<(), Box<dyn std::error::Error>> {
    let provider = DemoLibrarySourceProvider::curated();
    let first = provider.load_baseline()?;
    let second = provider.load_baseline()?;
    assert_eq!(provider.provider_kind(), "demo");
    assert_eq!(first, second);
    assert_eq!(first.tracks().len(), 3);
    assert_eq!(first.playlists().len(), 2);
    assert_eq!(first.playlists()[0].track_ids().len(), 3);
    assert!(first.tracks().iter().all(|track| {
        track.audio_uri().starts_with("lumi-demo://audio/")
            && !track.waveform().is_empty()
            && !track.beat_grid().markers().is_empty()
    }));
    Ok(())
}

#[test]
fn two_curated_revisions_are_deterministic_and_keep_stable_track_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let first =
        DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V1).load_baseline()?;
    let second =
        DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V2).load_baseline()?;
    assert_eq!(first.source_revision().as_str(), "demo-library-v1");
    assert_eq!(second.source_revision().as_str(), "demo-library-v2");
    assert_eq!(
        first
            .tracks()
            .iter()
            .map(|track| track.source_track_id())
            .collect::<Vec<_>>(),
        second
            .tracks()
            .iter()
            .map(|track| track.source_track_id())
            .collect::<Vec<_>>()
    );
    assert_ne!(first.tracks(), second.tracks());
    Ok(())
}

#[test]
fn scale_fixture_reaches_ten_thousand_tracks() -> Result<(), Box<dyn std::error::Error>> {
    let provider = DemoLibrarySourceProvider::scaled(10_000)?;
    let baseline = provider.load_baseline()?;
    assert_eq!(baseline.tracks().len(), 10_000);
    assert_eq!(baseline.playlists()[0].track_ids().len(), 10_000);
    assert_eq!(baseline.tracks()[9_999].title(), "Demo Track 10000");
    Ok(())
}

#[test]
fn demo_audio_is_generated_offline_and_deterministically() -> Result<(), Box<dyn std::error::Error>>
{
    let provider = DemoLibrarySourceProvider::curated();
    let first = provider.render_audio_segment("lumi-demo://audio/horizon-lines", 0, 100)?;
    let second = provider.render_audio_segment("lumi-demo://audio/horizon-lines", 0, 100)?;
    assert_eq!(first, second);
    assert_eq!(first.sample_rate_hz(), 44_100);
    assert_eq!(first.channel_count(), 1);
    assert_eq!(first.samples().len(), 4_410);
    assert!(first.samples().iter().any(|sample| *sample != 0));
    Ok(())
}
