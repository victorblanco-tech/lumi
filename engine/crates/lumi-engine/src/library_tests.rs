use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use lumi_domain::{PhraseKind, ThemeId, TrackId};
use lumi_library::{
    LibraryRepository as _, LibraryTrackSort, PhraseLoopStrategy, PhraseRoleId, ReconcileStrategy,
    TimelineEditCommand, TimelineRevisionOrigin, TrackPageRequest, VariantId, WaveformPoint,
};
use lumi_library_demo::{DemoLibraryRevision, DemoLibrarySourceProvider};
use lumi_library_source::MusicLibrarySourceProvider as _;
use lumi_library_sqlite::DeviceMatchCandidate;
use lumi_rekordbox_analysis::{
    AnalysisBeat, AnalysisPhrase, ResolvedTrackAnalysis, TrackAnalysisCoverage,
};
use lumi_rekordbox_device::{DeviceLibrarySnapshot, DeviceTrack};
use serde_json::json;

use super::{
    AutoloopCatalogMutation, DeviceInspection, DeviceReviewComparison, LibraryQueryUpdate,
    LibraryWorker, LibraryWorkerError, PhraseRoleCatalogMutation, canonical_beat_grid,
    canonical_phrases, deck_waveform_preview_points, device_audio_uri, device_metadata_matches,
    device_track_matches, first_available_audio_uri, is_kept_active_revision,
    kept_active_track_is_current,
};

#[test]
fn fresh_release_initialization_contains_no_demo_or_personal_data()
-> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("lumi-engine-clean-release-{unique}.sqlite"));
    {
        let worker = LibraryWorker::initialize_with_repository(
            lumi_library_sqlite::SqliteLibraryRepository::open(&path)?,
            Some(path.clone()),
            false,
        )?;
        let snapshot = worker.snapshot_json()?;
        assert_eq!(snapshot["condition"], "empty");
        assert_eq!(snapshot["collectionTotal"], 0);
        assert_eq!(snapshot["dataManagement"]["trackCount"], 0);
        assert_eq!(snapshot["dataManagement"]["playlistCount"], 0);
        assert_eq!(snapshot["rekordboxDevices"], json!([]));
        assert_eq!(snapshot["playlists"], json!([]));
        assert_eq!(snapshot["page"]["tracks"], json!([]));

        let catalog = snapshot["autoloopCatalog"]["themes"]
            .as_array()
            .ok_or("generic output banks are missing")?;
        assert_eq!(catalog.len(), 4);
        let serialized = serde_json::to_string(&snapshot)?;
        for personal_marker in ["90s Bitch", "Favourite Regrets", "DJ VIC", "BLUE PINK"] {
            assert!(!serialized.contains(personal_marker));
        }
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn kept_active_is_exact_revision_scoped_for_analysis_and_hot_cues() {
    assert!(is_kept_active_revision(
        "kept-active",
        "analysis-gray-1",
        "analysis-gray-1"
    ));
    assert!(!is_kept_active_revision(
        "kept-active",
        "analysis-gray-1",
        "analysis-gray-2"
    ));
    assert!(!is_kept_active_revision(
        "held-conflict",
        "analysis-gray-1",
        "analysis-gray-1"
    ));
    assert!(kept_active_track_is_current(
        "kept-active",
        "analysis-gray-1",
        "analysis-gray-1",
        "metadata-gray-1",
        "metadata-gray-1"
    ));
    assert!(!kept_active_track_is_current(
        "kept-active",
        "analysis-gray-1",
        "analysis-gray-1",
        "metadata-gray-1",
        "metadata-color-update"
    ));
}

fn resolved_analysis_with_grid(beat_numbers: &[u16]) -> ResolvedTrackAnalysis {
    ResolvedTrackAnalysis {
        coverage: TrackAnalysisCoverage::default(),
        beat_grid: beat_numbers
            .iter()
            .enumerate()
            .map(|(index, beat_number)| AnalysisBeat {
                beat_number: *beat_number,
                tempo_centi_bpm: 15_500,
                time_millis: u32::try_from(index).unwrap_or_default() * 387,
            })
            .collect(),
        waveform: Vec::new(),
        phrases: Vec::new(),
        hot_cues: Vec::new(),
    }
}

#[test]
fn rekordbox_downbeat_phase_and_exact_times_are_authoritative()
-> Result<(), Box<dyn std::error::Error>> {
    let mut analysis = resolved_analysis_with_grid(&[4, 1, 2, 3, 4, 1, 2, 3, 4, 1]);
    analysis.phrases = vec![
        AnalysisPhrase {
            start_beat: 0,
            end_beat: 5,
            source_label: "Intro".to_owned(),
        },
        AnalysisPhrase {
            start_beat: 5,
            end_beat: 9,
            source_label: "Drop".to_owned(),
        },
    ];

    let canonical = canonical_beat_grid(&analysis)?;
    assert_eq!(canonical.source_beat_offset, 1);
    assert_eq!(canonical.beat_grid.markers().len(), 8);
    assert_eq!(canonical.beat_grid.markers()[0].time_millis(), 387);
    assert_eq!(canonical.beat_grid.markers()[0].bar_index(), 1);
    assert_eq!(canonical.beat_grid.markers()[0].beat_in_bar(), 1);
    assert_eq!(canonical.beat_grid.markers()[4].time_millis(), 1_935);
    assert_eq!(canonical.beat_grid.markers()[4].bar_index(), 2);
    assert_eq!(canonical.beat_grid.markers()[4].beat_in_bar(), 1);

    let phrases = canonical_phrases(&analysis, 8, canonical.source_beat_offset)?;
    assert_eq!(phrases[0].start_beat(), 0);
    assert_eq!(phrases[0].end_beat(), 4);
    assert_eq!(phrases[0].source_label(), "Intro");
    assert_eq!(phrases[1].start_beat(), 4);
    assert_eq!(phrases[1].end_beat(), 8);
    assert_eq!(phrases[1].source_label(), "Drop");
    Ok(())
}

#[test]
fn inconsistent_rekordbox_beat_phase_fails_closed() {
    let analysis = resolved_analysis_with_grid(&[4, 1, 2, 4, 4, 1, 2, 3, 4]);
    assert!(matches!(
        canonical_beat_grid(&analysis),
        Err(LibraryWorkerError::InconsistentRekordboxBeatGrid {
            source_index: 3,
            expected: 3,
            actual: 4,
        })
    ));
}

#[test]
#[ignore = "requires LUMI_REKORDBOX_ANALYSIS_DAT"]
fn mounted_rekordbox_analysis_preserves_every_retained_source_beat()
-> Result<(), Box<dyn std::error::Error>> {
    let dat_path = PathBuf::from(std::env::var("LUMI_REKORDBOX_ANALYSIS_DAT")?);
    let analysis_root = dat_path.parent().ok_or("analysis DAT has no parent")?;
    let temporary = super::RekordboxImportTemporaryRoot::create()?;
    let request = lumi_rekordbox_analysis::ResolvedAnalysisRequest::try_new(
        analysis_root,
        temporary.path().join("exact-grid-evidence"),
        [lumi_rekordbox_analysis::ResolvedAnalysisTrack::try_new(
            "track", &dat_path,
        )?],
    )?;
    let resolved = lumi_rekordbox_analysis::snapshot_resolved_analysis_data(&request)?;
    let source = resolved.tracks.get("track").ok_or("analysis is missing")?;
    let canonical = canonical_beat_grid(source)?;
    let offset = usize::try_from(canonical.source_beat_offset)?;

    let phrases = canonical_phrases(
        source,
        u32::try_from(canonical.beat_grid.markers().len())?,
        canonical.source_beat_offset,
    )?;
    assert_eq!(phrases.len(), source.phrases.len());
    assert_eq!(phrases.first().map(|phrase| phrase.start_beat()), Some(0));
    for (phrase, source_phrase) in phrases.iter().zip(&source.phrases) {
        assert_eq!(
            phrase.start_beat(),
            source_phrase
                .start_beat
                .checked_sub(canonical.source_beat_offset)
                .ok_or("source phrase precedes the canonical beat grid")?
        );
    }

    for (canonical_index, marker) in canonical.beat_grid.markers().iter().enumerate() {
        let source_marker = &source.beat_grid[offset + canonical_index];
        assert_eq!(marker.time_millis(), u64::from(source_marker.time_millis));
        assert_eq!(
            marker.beat_in_bar(),
            u8::try_from(source_marker.beat_number)?
        );
    }
    Ok(())
}

#[test]
fn exact_unique_metadata_can_match_when_usb_container_size_differs() {
    let candidate = DeviceMatchCandidate {
        track_id: lumi_domain::TrackId::new(42),
        source_id: "rekordbox7-local".to_owned(),
        source_kind: "rekordbox7".to_owned(),
        has_user_timeline_edits: false,
        title: "90s Bitch - Extended Mix".to_owned(),
        artist: "Maddix, The Rocketman".to_owned(),
        bpm_milli: 145_000,
        duration_millis: 192_000,
        audio_uri: "file://localhost/nonexistent/local-track.wav".to_owned(),
    };
    let device = DeviceTrack {
        device_track_id: 1_031,
        title: "90s Bitch - Extended Mix".to_owned(),
        artist: "Maddix, The Rocketman".to_owned(),
        musical_key: "4A".to_owned(),
        color_rgb: Some(0x32_80_ff),
        bpm_milli: 145_000,
        duration_millis: 192_000,
        file_size: 123_456,
        audio_path: PathBuf::from("/Volumes/Test/90s Bitch.wav"),
        analysis_dat_path: PathBuf::from("/Volumes/Test/USBANLZ/track.DAT"),
        metadata_revision: "metadata".to_owned(),
        analysis_revision: "analysis".to_owned(),
        analyzed_at: "2026-08-11".to_owned(),
        audio_signature: "signature".to_owned(),
        simulator_signature: 77,
        master_database_id: 1,
        master_content_id: 2,
        analysis_update_count: 3,
        information_update_count: 4,
        cue_update_count: 5,
    };

    assert!(device_metadata_matches(&candidate, &device));
    assert!(!device_track_matches(&candidate, &device));
}

#[test]
#[ignore = "requires LUMI_DEVICE_POC_DATABASE and LUMI_REKORDBOX_DEVICE_ROOT"]
fn mounted_device_sync_hydrates_the_same_canonical_track_by_real_and_simulator_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let database_path = std::env::var("LUMI_DEVICE_POC_DATABASE")?;
    let device_root = std::env::var("LUMI_REKORDBOX_DEVICE_ROOT")?;
    let device = lumi_rekordbox_device::read_device_library(&device_root)?;
    let mut worker = LibraryWorker::demo_at(std::path::Path::new(&database_path))?;

    let selected_playlist_id = std::env::var("LUMI_DEVICE_POC_PLAYLIST_ID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    let playlist_ids = device
        .playlists
        .iter()
        .map(|playlist| playlist.device_playlist_id)
        .filter(|playlist_id| selected_playlist_id.is_none_or(|value| value == *playlist_id))
        .collect::<Vec<_>>();
    let selected_track_count = device
        .playlists
        .iter()
        .filter(|playlist| playlist_ids.contains(&playlist.device_playlist_id))
        .flat_map(|playlist| playlist.track_ids.iter().copied())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let source_id = std::env::var("LUMI_DEVICE_POC_SOURCE_ID")
        .unwrap_or_else(|_| "usb-fs:mounted-device-poc".to_owned());
    let result = worker.sync_rekordbox_device(&device_root, Some(&source_id), &playlist_ids)?;
    assert_eq!(result.tracks, selected_track_count);
    assert!(result.matched > 0);
    assert!(result.refreshed_analyses <= result.matched);

    let (device_track, real) = device
        .tracks
        .values()
        .find_map(|device_track| {
            worker
                .connected_track(device_track.device_track_id, 0)
                .ok()
                .flatten()
                .map(|track| (device_track, track))
        })
        .ok_or("no synchronized device track resolved")?;
    let simulated = worker
        .connected_track(42, device_track.simulator_signature)?
        .ok_or("simulated identity did not resolve")?;
    assert_eq!(
        real.prepared.metadata.id(),
        simulated.prepared.metadata.id()
    );
    Ok(())
}

#[test]
fn deck_waveform_preview_is_bounded_peak_and_hue_preserving() {
    let mut waveform = (0..16_384)
        .map(|_| WaveformPoint::new(8, 16, 24))
        .collect::<Vec<_>>();
    waveform[8_191] = WaveformPoint::new(255, 254, 253);

    let preview = deck_waveform_preview_points(&waveform, 1_024);

    assert_eq!(preview.len(), 1_024);
    assert!(preview.contains(&[255, 254, 253]));

    let distinct_hues = [WaveformPoint::new(255, 0, 0), WaveformPoint::new(0, 255, 0)];
    let hue_preserving = deck_waveform_preview_points(&distinct_hues, 1);
    assert_eq!(hue_preserving.len(), 1);
    assert!(
        hue_preserving[0] == [255, 0, 0] || hue_preserving[0] == [0, 255, 0],
        "downsampling must retain a real source hue instead of inventing yellow"
    );
}

#[test]
fn local_deck_preview_keeps_the_same_eight_bit_rgb_scale_as_detail()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    let snapshot = worker.snapshot_json()?;
    let track = &snapshot["page"]["tracks"][0];
    let track_id = track["id"].as_u64().ok_or("demo track id is missing")?;
    let timeline_revision = track["timelineRevision"]
        .as_u64()
        .ok_or("demo timeline revision is missing")?;
    let (_, context) = worker
        .local_playback_track(track_id, timeline_revision)?
        .into_parts();
    let preview = context.waveform_preview_json();
    let remote = context.remote_waveform_preview_json();
    let maximum_channel = preview["points"]
        .as_array()
        .ok_or("preview points are missing")?
        .iter()
        .flat_map(|point| ["low", "mid", "high"].map(|channel| point[channel].as_u64()))
        .flatten()
        .max()
        .ok_or("preview channels are missing")?;

    assert_eq!(preview["source"], "localLibrary");
    assert_eq!(remote["source"], "localLibraryDetail");
    assert!(
        remote["points"].as_array().map_or(0, Vec::len)
            >= preview["points"].as_array().map_or(0, Vec::len),
        "Remote static projection must retain at least the Mac preview detail"
    );
    assert!(
        maximum_channel > 31,
        "bounded preview must retain 8-bit RGB values"
    );
    Ok(())
}

#[test]
fn collection_total_is_independent_from_the_active_playlist()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    worker.query(LibraryQueryUpdate {
        search: String::new(),
        playlist_id: Some(2),
        workflow_filter: None,
        workflow_step_id: None,
        offset: 0,
        limit: 50,
        sort: LibraryTrackSort::default(),
    });

    let snapshot = worker.snapshot_json()?;

    assert_eq!(snapshot["collectionTotal"], 3);
    assert_eq!(snapshot["page"]["total"], 2);
    Ok(())
}

#[test]
fn library_reset_apply_rejects_phrase_work_created_after_review()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    worker.preview_library_reset(&[])?;
    let token = worker
        .pending_library_reset
        .as_ref()
        .ok_or("reset preview")?
        .token
        .clone();
    worker.open_editor(1)?;
    worker.edit_timeline(
        1,
        1,
        TimelineEditCommand::ChangeRole {
            phrase_index: 0,
            role_id: PhraseRoleId::try_new("synth")?,
        },
    )?;

    let result = worker.apply_library_reset(&token, "/not/used/after/stale-review.sqlite");

    assert!(matches!(
        result,
        Err(LibraryWorkerError::LibraryResetPreviewChanged)
    ));
    assert!(worker.pending_library_reset.is_none());
    assert_eq!(worker.snapshot_json()?["collectionTotal"], 3);
    assert_eq!(
        worker
            .repository
            .timeline_head(TrackId::new(1))?
            .map(|head| head.revision().value()),
        Some(2)
    );
    Ok(())
}

#[test]
fn simulator_track_uses_exact_lumi_revision_and_fails_closed_on_stale_or_unknown_matches()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    worker.open_editor(1)?;
    worker.edit_timeline(
        1,
        1,
        TimelineEditCommand::ChangeRole {
            phrase_index: 0,
            role_id: PhraseRoleId::try_new("synth")?,
        },
    )?;

    let (metadata, context) = worker.simulator_track(1, 2)?.into_parts();
    let identity = metadata
        .identity_facts()
        .ok_or("library simulator track must include identity facts")?;
    assert_eq!(identity.provider_kind(), "demo");
    assert_eq!(identity.lumi_timeline_revision(), 2);
    assert_eq!(metadata.phrases()[0].kind(), PhraseKind::Build);
    assert!(
        context.phrase_role_json(0)["colorRgb"].as_u64().is_some(),
        "library-backed phrases must project the configured Phrase color"
    );
    let resolved = context.resolve(ThemeId::new(1))?;
    assert_eq!(resolved[0].role_id, "synth");
    assert_eq!(resolved[0].strategy, "auto");
    assert_eq!(resolved[0].entry_id, "theme-1--mapping-5");

    assert!(matches!(
        worker.simulator_track(1, 1),
        Err(LibraryWorkerError::TimelineRevisionConflict { .. })
    ));
    assert!(matches!(
        worker.simulator_track(999_999, 1),
        Err(LibraryWorkerError::UnknownTrack(999_999))
    ));
    Ok(())
}

#[test]
fn default_phrase_roles_are_seeded_once_and_user_changes_survive_restart()
-> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("lumi-engine-roles-{unique}.sqlite"));
    {
        let mut worker = LibraryWorker::demo_at(&path)?;
        let initial = worker.snapshot_json()?;
        let roles = initial["phraseRoleSettings"]["roles"]
            .as_array()
            .ok_or("phrase-role settings are missing")?;
        assert_eq!(initial["phraseRoleSettings"]["revision"], 1);
        assert_eq!(roles.len(), 11);
        assert_eq!(roles[0]["id"], "intro-outro");
        assert_eq!(roles[5]["id"], "synth");

        worker.mutate_phrase_role_catalog(
            1,
            PhraseRoleCatalogMutation::Rename {
                role_id: PhraseRoleId::try_new("synth")?,
                display_name: "Lead Synth".to_owned(),
            },
        )?;
        worker.mutate_phrase_role_catalog(
            2,
            PhraseRoleCatalogMutation::SetArchived {
                role_id: PhraseRoleId::try_new("synth")?,
                archived: true,
            },
        )?;
    }

    let worker = LibraryWorker::demo_at(&path)?;
    let restarted = worker.snapshot_json()?;
    let roles = restarted["phraseRoleSettings"]["roles"]
        .as_array()
        .ok_or("phrase-role settings are missing after restart")?;
    assert_eq!(restarted["phraseRoleSettings"]["revision"], 3);
    assert_eq!(roles.len(), 11);
    let synth = roles
        .iter()
        .find(|role| role["id"] == "synth")
        .ok_or("stable Synth role is missing")?;
    assert_eq!(synth["name"], "Lead Synth");
    assert_eq!(synth["archived"], true);
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn mapping_changes_only_initialize_future_timelines_and_keep_raw_provenance()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    let original_track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
        .as_u64()
        .ok_or("demo track ID is missing")?;
    worker.open_editor(original_track_id)?;
    let before = worker.snapshot_json()?;
    assert_eq!(before["editor"]["phrases"][0]["roleId"], "intro-outro");
    assert_eq!(before["editor"]["sourcePhrases"][0]["rawLabel"], "Intro");

    worker.mutate_phrase_role_catalog(
        1,
        PhraseRoleCatalogMutation::SetSourceMapping {
            provider_kind: "demo".to_owned(),
            raw_label: "Demo".to_owned(),
            role_id: PhraseRoleId::try_new("synth")?,
        },
    )?;
    let unchanged = worker.snapshot_json()?;
    assert_eq!(unchanged["editor"]["timeline"]["revision"], 1);
    assert_eq!(unchanged["editor"]["phrases"][0]["roleId"], "intro-outro");

    let new_baseline = DemoLibrarySourceProvider::scaled(1)?.load_baseline()?;
    worker.repository.import_baseline(&new_baseline)?;
    let new_track = worker
        .repository
        .page_tracks(TrackPageRequest::try_new(0, 200)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id().as_str() == "scale-00000")
        .ok_or("new scale track was not imported")?
        .id();
    worker.ensure_timeline(new_track)?;
    let timeline = worker
        .repository
        .timeline_head(new_track)?
        .ok_or("new track timeline was not initialized")?;
    assert_eq!(timeline.phrases()[0].role_id().as_str(), "synth");
    Ok(())
}

#[test]
fn archived_roles_cannot_receive_mappings_or_initialize_future_timelines()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    worker.mutate_phrase_role_catalog(
        1,
        PhraseRoleCatalogMutation::SetSourceMapping {
            provider_kind: "demo".to_owned(),
            raw_label: "Demo".to_owned(),
            role_id: PhraseRoleId::try_new("synth")?,
        },
    )?;
    worker.mutate_phrase_role_catalog(
        2,
        PhraseRoleCatalogMutation::SetArchived {
            role_id: PhraseRoleId::try_new("synth")?,
            archived: true,
        },
    )?;

    let rejected_mapping = worker.mutate_phrase_role_catalog(
        3,
        PhraseRoleCatalogMutation::SetSourceMapping {
            provider_kind: "demo".to_owned(),
            raw_label: "Intro".to_owned(),
            role_id: PhraseRoleId::try_new("synth")?,
        },
    );
    assert!(matches!(
        rejected_mapping,
        Err(super::LibraryWorkerError::ArchivedPhraseRole)
    ));

    let new_baseline = DemoLibrarySourceProvider::scaled(1)?.load_baseline()?;
    worker.repository.import_baseline(&new_baseline)?;
    let new_track = worker
        .repository
        .page_tracks(TrackPageRequest::try_new(0, 200)?)?
        .tracks()
        .iter()
        .find(|track| track.source_track_id().as_str() == "scale-00000")
        .ok_or("new scale track was not imported")?
        .id();
    let initialization = worker.ensure_timeline(new_track);
    assert!(matches!(
        initialization,
        Err(super::LibraryWorkerError::ArchivedSourcePhraseMapping { .. })
    ));
    Ok(())
}

#[test]
fn phrase_role_usage_and_synth_assignment_are_exact_and_stale_safe()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    let track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
        .as_u64()
        .ok_or("demo track ID is missing")?;
    worker.open_editor(track_id)?;
    worker.edit_timeline(
        track_id,
        1,
        TimelineEditCommand::ChangeRole {
            phrase_index: 0,
            role_id: PhraseRoleId::try_new("synth")?,
        },
    )?;
    let snapshot = worker.snapshot_json()?;
    assert_eq!(snapshot["editor"]["phrases"][0]["roleId"], "synth");
    let synth = snapshot["phraseRoleSettings"]["roles"]
        .as_array()
        .and_then(|roles| roles.iter().find(|role| role["id"] == "synth"))
        .ok_or("Synth usage is missing")?;
    assert_eq!(synth["usage"]["trackCount"], 1);
    assert_eq!(synth["usage"]["phraseCount"], 1);
    assert_eq!(synth["usage"]["catalogRowCount"], 4);

    let stale = worker.mutate_phrase_role_catalog(
        2,
        PhraseRoleCatalogMutation::Rename {
            role_id: PhraseRoleId::try_new("synth")?,
            display_name: "Lead Synth".to_owned(),
        },
    );
    assert!(matches!(
        stale,
        Err(
            super::LibraryWorkerError::PhraseRoleCatalogRevisionConflict {
                expected: 2,
                actual: 1,
            }
        )
    ));
    Ok(())
}

#[test]
fn four_theme_autoloop_defaults_and_mutations_survive_restart()
-> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("lumi-engine-autoloops-{unique}.sqlite"));
    {
        let mut worker = LibraryWorker::demo_at(&path)?;
        let initial = worker.snapshot_json()?;
        assert_eq!(initial["autoloopCatalog"]["revision"], 1);
        assert_eq!(
            initial["autoloopCatalog"]["themes"]
                .as_array()
                .map(Vec::len),
            Some(4)
        );
        let synth = initial["autoloopCatalog"]["roles"]
            .as_array()
            .and_then(|roles| roles.iter().find(|role| role["id"] == "synth"))
            .ok_or("Synth matrix row is missing")?;
        assert_eq!(synth["variants"].as_array().map(Vec::len), Some(4));
        assert_eq!(initial["autoloopCatalog"]["preflight"]["status"], "ready");

        worker.mutate_autoloop_catalog(
            1,
            AutoloopCatalogMutation::RenameTheme {
                theme_id: ThemeId::new(1),
                display_name: "Electric Garden".to_owned(),
            },
        )?;
        worker.mutate_autoloop_catalog(
            2,
            AutoloopCatalogMutation::AddVariant {
                role_id: PhraseRoleId::try_new("synth")?,
                display_name: "Variant 3".to_owned(),
            },
        )?;
        let incomplete = worker.snapshot_json()?;
        assert_eq!(
            incomplete["autoloopCatalog"]["preflight"]["missingCellCount"],
            4
        );
        let stale = worker.mutate_autoloop_catalog(
            1,
            AutoloopCatalogMutation::RenameTheme {
                theme_id: ThemeId::new(2),
                display_name: "Ocean Garden".to_owned(),
            },
        );
        assert!(matches!(
            stale,
            Err(LibraryWorkerError::AutoloopCatalogRevisionConflict {
                expected: 1,
                actual: 3,
            })
        ));
    }

    let worker = LibraryWorker::demo_at(&path)?;
    let restarted = worker.snapshot_json()?;
    assert_eq!(restarted["autoloopCatalog"]["revision"], 3);
    assert_eq!(
        restarted["autoloopCatalog"]["themes"][0]["name"],
        "Electric Garden"
    );
    assert_eq!(
        restarted["autoloopCatalog"]["preflight"]["missingCellCount"],
        4
    );
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn new_phrase_roles_block_preflight_until_they_have_a_variant()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    worker.mutate_phrase_role_catalog(
        1,
        PhraseRoleCatalogMutation::Add {
            display_name: "Vocal".to_owned(),
        },
    )?;

    let uncovered = worker.snapshot_json()?;
    assert_eq!(
        uncovered["autoloopCatalog"]["preflight"]["missingRoleCount"],
        1
    );
    assert_eq!(
        uncovered["autoloopCatalog"]["preflight"]["missingRoleIds"][0],
        "custom-1"
    );
    assert_eq!(
        uncovered["autoloopCatalog"]["preflight"]["status"],
        "incomplete"
    );

    worker.mutate_autoloop_catalog(
        1,
        AutoloopCatalogMutation::AddVariant {
            role_id: PhraseRoleId::try_new("custom-1")?,
            display_name: "Variant 1".to_owned(),
        },
    )?;
    let covered = worker.snapshot_json()?;
    assert_eq!(
        covered["autoloopCatalog"]["preflight"]["missingRoleCount"],
        0
    );
    assert_eq!(
        covered["autoloopCatalog"]["preflight"]["missingCellCount"],
        4
    );
    Ok(())
}

#[test]
fn phrase_loop_strategy_is_role_safe_revisioned_and_restart_persistent()
-> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("lumi-engine-loop-strategy-{unique}.sqlite"));
    let track_id;
    {
        let mut worker = LibraryWorker::demo_at(&path)?;
        track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("demo track ID is missing")?;
        worker.open_editor(track_id)?;
        worker.set_phrase_loop_strategy(
            track_id,
            1,
            1,
            0,
            PhraseLoopStrategy::FixedVariant(VariantId::try_new("mapping-1")?),
        )?;
        let fixed = worker.snapshot_json()?;
        assert_eq!(fixed["editor"]["timeline"]["revision"], 2);
        assert_eq!(fixed["editor"]["timeline"]["reason"], "changeLoopStrategy");
        assert_eq!(
            fixed["editor"]["phrases"][0]["loopStrategy"]["kind"],
            "fixedVariant"
        );
        assert_eq!(
            fixed["editor"]["phrases"][0]["loopStrategy"]["fixedVariantId"],
            "mapping-1"
        );
        assert_eq!(
            fixed["editor"]["phrases"][0]["loopStrategy"]["locked"],
            true
        );
    }
    {
        let mut worker = LibraryWorker::demo_at(&path)?;
        worker.open_editor(track_id)?;
        let restarted = worker.snapshot_json()?;
        assert_eq!(
            restarted["editor"]["phrases"][0]["loopStrategy"]["kind"],
            "fixedVariant"
        );
        worker.mutate_autoloop_catalog(
            1,
            AutoloopCatalogMutation::RenameTheme {
                theme_id: ThemeId::new(1),
                display_name: "Electric Garden".to_owned(),
            },
        )?;
        let stale = worker.set_phrase_loop_strategy(track_id, 2, 1, 0, PhraseLoopStrategy::Auto);
        assert!(matches!(
            stale,
            Err(LibraryWorkerError::AutoloopCatalogRevisionConflict {
                expected: 1,
                actual: 2,
            })
        ));
        worker.set_phrase_loop_strategy(track_id, 2, 2, 0, PhraseLoopStrategy::Auto)?;
    }
    let mut worker = LibraryWorker::demo_at(&path)?;
    worker.open_editor(track_id)?;
    let automatic = worker.snapshot_json()?;
    assert_eq!(automatic["editor"]["timeline"]["revision"], 3);
    assert_eq!(
        automatic["editor"]["phrases"][0]["loopStrategy"]["kind"],
        "auto"
    );
    assert_eq!(
        automatic["editor"]["phrases"][0]["loopStrategy"]["locked"],
        false
    );
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn editor_snapshot_exposes_read_only_analysis_and_closes_cleanly()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    let collection = worker.snapshot_json()?;
    let track_id = collection["page"]["tracks"][0]["id"]
        .as_u64()
        .ok_or("demo track ID is missing")?;

    worker.open_editor(track_id)?;
    let opened = worker.snapshot_json()?;
    assert_eq!(opened["editor"]["track"]["id"], track_id);
    assert_eq!(opened["editor"]["beatGrid"]["beatsPerBar"], 4);
    assert!(
        opened["editor"]["beatGrid"]["markers"]
            .as_array()
            .is_some_and(|markers| !markers.is_empty())
    );
    assert!(
        opened["editor"]["waveform"]
            .as_array()
            .is_some_and(|points| !points.is_empty())
    );
    assert!(
        opened["editor"]["phrases"]
            .as_array()
            .is_some_and(|phrases| !phrases.is_empty())
    );

    worker.close_editor();
    assert!(worker.snapshot_json()?["editor"].is_null());
    Ok(())
}

#[test]
fn unknown_editor_track_is_rejected_without_changing_selection()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    assert!(worker.open_editor(u64::MAX).is_err());
    assert!(worker.snapshot_json()?["editor"].is_null());
    Ok(())
}

#[test]
fn source_mapping_becomes_the_authoritative_lumi_timeline() -> Result<(), Box<dyn std::error::Error>>
{
    let mut worker = LibraryWorker::demo()?;
    let imported = worker.snapshot_json()?;
    assert_eq!(imported["page"]["tracks"][0]["timelineRevision"], 1);
    let track_id = imported["page"]["tracks"][0]["id"]
        .as_u64()
        .ok_or("track id")?;

    worker.open_editor(track_id)?;
    let snapshot = worker.snapshot_json()?;
    assert_eq!(snapshot["editor"]["timeline"]["revision"], 1);
    assert_eq!(
        snapshot["editor"]["timeline"]["reason"],
        "initialSourceMapping"
    );
    assert_eq!(snapshot["editor"]["timeline"]["canUndo"], false);
    assert_eq!(snapshot["editor"]["phrases"][0]["roleId"], "intro-outro");
    assert!(
        snapshot["editor"]["phrases"]
            .as_array()
            .is_some_and(|phrases| phrases.iter().all(|phrase| {
                phrase["startBeat"]
                    .as_u64()
                    .is_some_and(|value| value % 4 == 0)
                    && phrase["endBeat"]
                        .as_u64()
                        .is_some_and(|value| value % 4 == 0)
            }))
    );
    Ok(())
}

#[test]
fn edit_undo_redo_restore_and_stale_rejection_are_deterministic()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    let track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
        .as_u64()
        .ok_or("track id")?;
    worker.open_editor(track_id)?;

    worker.edit_timeline(
        track_id,
        1,
        TimelineEditCommand::Split {
            phrase_index: 0,
            at_beat: 4,
        },
    )?;
    let edited = worker.snapshot_json()?;
    assert_eq!(edited["editor"]["timeline"]["revision"], 2);
    assert_eq!(edited["editor"]["timeline"]["reason"], "splitPhrase");
    assert_eq!(edited["editor"]["timeline"]["canUndo"], true);
    assert_eq!(
        edited["editor"]["phrases"].as_array().map(Vec::len),
        Some(5)
    );

    let stale = worker.edit_timeline(
        track_id,
        1,
        TimelineEditCommand::ChangeRole {
            phrase_index: 0,
            role_id: PhraseRoleId::try_new("synth")?,
        },
    );
    assert!(matches!(
        stale,
        Err(super::LibraryWorkerError::TimelineRevisionConflict { .. })
    ));

    worker.undo_timeline(track_id, 2)?;
    let undone = worker.snapshot_json()?;
    assert_eq!(undone["editor"]["timeline"]["revision"], 3);
    assert_eq!(undone["editor"]["timeline"]["reason"], "undo");
    assert_eq!(undone["editor"]["timeline"]["canRedo"], true);
    assert_eq!(
        undone["editor"]["phrases"].as_array().map(Vec::len),
        Some(4)
    );

    worker.redo_timeline(track_id, 3)?;
    let redone = worker.snapshot_json()?;
    assert_eq!(redone["editor"]["timeline"]["revision"], 4);
    assert_eq!(redone["editor"]["timeline"]["reason"], "redo");
    assert_eq!(
        redone["editor"]["phrases"].as_array().map(Vec::len),
        Some(5)
    );

    worker.restore_timeline_revision(track_id, 4, 1)?;
    let restored = worker.snapshot_json()?;
    assert_eq!(restored["editor"]["timeline"]["revision"], 5);
    assert_eq!(restored["editor"]["timeline"]["reason"], "restoreRevision");
    assert_eq!(
        restored["editor"]["phrases"].as_array().map(Vec::len),
        Some(4)
    );
    Ok(())
}

#[test]
fn phrase_protection_is_persisted_and_enforced_below_the_ui()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    let track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
        .as_u64()
        .ok_or("track id")?;
    worker.open_editor(track_id)?;
    worker.set_track_phrase_protection(track_id, 0, true)?;
    let locked = worker.snapshot_json()?;
    assert_eq!(
        locked["editor"]["track"]["phraseProtection"]["locked"],
        true
    );
    assert_eq!(locked["editor"]["track"]["phraseProtection"]["revision"], 1);

    let rejected = worker.edit_timeline(
        track_id,
        1,
        TimelineEditCommand::Split {
            phrase_index: 0,
            at_beat: 4,
        },
    );
    assert!(matches!(
        rejected,
        Err(super::LibraryWorkerError::TrackPhrasesProtected)
    ));
    assert_eq!(worker.snapshot_json()?["editor"]["timeline"]["revision"], 1);

    worker.set_track_phrase_protection(track_id, 1, false)?;
    worker.edit_timeline(
        track_id,
        1,
        TimelineEditCommand::Split {
            phrase_index: 0,
            at_beat: 4,
        },
    )?;
    assert_eq!(worker.snapshot_json()?["editor"]["timeline"]["revision"], 2);
    Ok(())
}

#[test]
fn prepared_live_phrase_edit_rechecks_protection_and_durable_head()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    worker.open_editor(1)?;
    let (first, _) = worker.prepare_live_phrase_role(1, 1, 1, PhraseRoleId::try_new("drop")?)?;
    let (stale, _) =
        worker.prepare_live_phrase_role(1, 1, 1, PhraseRoleId::try_new("intro-outro")?)?;
    // Preparation does not append a revision or alter the open editor.
    assert!(worker.local_playback_track(1, 1).is_ok());
    worker.commit_live_phrase_role(first)?;
    assert!(matches!(
        worker.commit_live_phrase_role(stale),
        Err(LibraryWorkerError::Persistence(
            lumi_library_sqlite::SqliteLibraryError::RevisionConflict { .. }
        ))
    ));
    let (_, committed) = worker.local_playback_track(1, 2)?.into_parts();
    assert_eq!(committed.phrase_role_json(1)["roleId"], "drop");
    let (protected, _) =
        worker.prepare_live_phrase_role(1, 2, 1, PhraseRoleId::try_new("intro-outro")?)?;
    worker.set_track_phrase_protection(1, 0, true)?;
    assert!(matches!(
        worker.commit_live_phrase_role(protected),
        Err(LibraryWorkerError::TrackPhrasesProtected)
    ));
    let (_, unchanged) = worker.local_playback_track(1, 2)?.into_parts();
    assert_eq!(unchanged.phrase_role_json(1), committed.phrase_role_json(1));
    Ok(())
}

#[test]
fn phrase_protection_keeps_the_active_workflow_query_and_page_atomic()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    let track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
        .as_u64()
        .ok_or("track id")?;
    worker.open_editor(track_id)?;
    worker.query(LibraryQueryUpdate {
        search: String::new(),
        playlist_id: None,
        workflow_filter: Some(lumi_library::TrackWorkflowFilter::ChangedAfterUsbSync),
        workflow_step_id: None,
        offset: 0,
        limit: 50,
        sort: LibraryTrackSort::default(),
    });

    let before = worker.snapshot_json()?;
    assert_eq!(before["query"]["workflowFilter"], "changedAfterUsbSync");
    assert_eq!(before["page"]["total"], 0);

    worker.set_track_phrase_protection(track_id, 0, true)?;
    let after = worker.snapshot_json()?;
    assert_eq!(after["query"]["workflowFilter"], "changedAfterUsbSync");
    assert_eq!(after["page"]["total"], 0);
    assert_eq!(after["editor"]["track"]["phraseProtection"]["locked"], true);
    Ok(())
}

#[test]
fn timeline_and_undo_redo_cursor_survive_worker_restart() -> Result<(), Box<dyn std::error::Error>>
{
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("lumi-engine-history-{unique}.sqlite"));
    let track_id;
    {
        let mut worker = LibraryWorker::demo_at(&path)?;
        track_id = worker.snapshot_json()?["page"]["tracks"][0]["id"]
            .as_u64()
            .ok_or("track id")?;
        worker.open_editor(track_id)?;
        worker.edit_timeline(
            track_id,
            1,
            TimelineEditCommand::Split {
                phrase_index: 0,
                at_beat: 4,
            },
        )?;
        worker.undo_timeline(track_id, 2)?;
    }

    {
        let mut worker = LibraryWorker::demo_at(&path)?;
        worker.open_editor(track_id)?;
        let restored = worker.snapshot_json()?;
        assert_eq!(restored["editor"]["timeline"]["revision"], 3);
        assert_eq!(restored["editor"]["timeline"]["canRedo"], true);
        worker.redo_timeline(track_id, 3)?;
        assert_eq!(worker.snapshot_json()?["editor"]["timeline"]["revision"], 4);
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn source_refresh_is_previewed_then_explicitly_reconciled() -> Result<(), Box<dyn std::error::Error>>
{
    let mut worker = LibraryWorker::demo()?;
    let horizon_id = worker.snapshot_json()?["page"]["tracks"]
        .as_array()
        .ok_or("tracks")?
        .iter()
        .find(|track| track["sourceTrackId"] == "horizon-lines")
        .and_then(|track| track["id"].as_u64())
        .ok_or("horizon id")?;
    worker.open_editor(horizon_id)?;
    worker.preview_demo_source_refresh()?;
    let preview = worker.snapshot_json()?;
    assert_eq!(preview["source"]["status"], "changesAvailable");
    assert_eq!(preview["sourceRefresh"]["changeCount"], 3);
    assert_eq!(
        preview["editor"]["sourceReconciliation"]["toRevision"],
        "horizon-lines-v2"
    );
    assert!(
        preview["editor"]["sourceReconciliation"]["conflicts"]
            .as_array()
            .is_some_and(|conflicts| !conflicts.is_empty())
    );
    let golden: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/source-reconciliation/horizon-lines-preview.json"
    ))?;
    assert_eq!(preview["editor"]["sourceReconciliation"], golden);

    worker.reconcile_source_refresh(horizon_id, 1, ReconcileStrategy::ReplaceWithSource)?;
    let reconciled = worker.snapshot_json()?;
    assert_eq!(reconciled["editor"]["timeline"]["revision"], 2);
    assert_eq!(
        reconciled["editor"]["timeline"]["reason"],
        "sourceReconcile"
    );
    assert!(reconciled["editor"]["sourceReconciliation"].is_null());
    assert_eq!(reconciled["sourceRefresh"]["changeCount"], 2);

    worker.close_editor();
    let afterglow_id = reconciled["page"]["tracks"]
        .as_array()
        .ok_or("tracks")?
        .iter()
        .find(|track| track["sourceTrackId"] == "afterglow-drive")
        .and_then(|track| track["id"].as_u64())
        .ok_or("afterglow id")?;
    worker.open_editor(afterglow_id)?;
    worker.reconcile_source_refresh(afterglow_id, 1, ReconcileStrategy::KeepLumi)?;
    let metadata_refresh = worker.snapshot_json()?;
    assert_eq!(
        metadata_refresh["editor"]["track"]["title"],
        "Afterglow Drive (Extended)"
    );
    assert_eq!(metadata_refresh["editor"]["timeline"]["revision"], 1);
    assert_eq!(metadata_refresh["sourceRefresh"]["changeCount"], 1);
    Ok(())
}

#[test]
fn epic_two_a_golden_survives_restart_refresh_and_four_theme_resolution()
-> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("lumi-epic-2a-golden-{unique}.sqlite"));
    let horizon_id;

    {
        let mut worker = LibraryWorker::demo_at(&path)?;
        let initial = worker.snapshot_json()?;
        let tracks = initial["page"]["tracks"]
            .as_array()
            .ok_or("demo tracks are missing")?;
        let track_id = |source_track_id: &str| {
            tracks
                .iter()
                .find(|track| track["sourceTrackId"] == source_track_id)
                .and_then(|track| track["id"].as_u64())
                .ok_or("demo track ID is missing")
        };
        horizon_id = track_id("horizon-lines")?;
        let afterglow_id = track_id("afterglow-drive")?;
        let northern_id = track_id("northern-pulse")?;

        worker.query(LibraryQueryUpdate {
            search: "Horizon Lines".to_owned(),
            playlist_id: None,
            workflow_filter: None,
            workflow_step_id: None,
            offset: 0,
            limit: 50,
            sort: LibraryTrackSort::default(),
        });
        let browsed = worker.snapshot_json()?;
        assert_eq!(browsed["page"]["total"], 1);
        assert_eq!(browsed["page"]["tracks"][0]["id"], horizon_id);

        worker.open_editor(horizon_id)?;
        worker.edit_timeline(
            horizon_id,
            1,
            TimelineEditCommand::ChangeRole {
                phrase_index: 0,
                role_id: PhraseRoleId::try_new("synth")?,
            },
        )?;
        worker.set_phrase_loop_strategy(
            horizon_id,
            2,
            1,
            0,
            PhraseLoopStrategy::FixedVariant(VariantId::try_new("mapping-5")?),
        )?;
        worker.mutate_phrase_role_catalog(
            1,
            PhraseRoleCatalogMutation::Rename {
                role_id: PhraseRoleId::try_new("synth")?,
                display_name: "Lead Synth".to_owned(),
            },
        )?;

        worker.preview_demo_source_refresh()?;
        assert_eq!(worker.snapshot_json()?["sourceRefresh"]["changeCount"], 3);
        worker.reconcile_source_refresh(horizon_id, 3, ReconcileStrategy::KeepLumi)?;

        worker.close_editor();
        worker.open_editor(afterglow_id)?;
        worker.reconcile_source_refresh(afterglow_id, 1, ReconcileStrategy::KeepLumi)?;

        worker.close_editor();
        worker.open_editor(northern_id)?;
        worker.reconcile_source_refresh(northern_id, 1, ReconcileStrategy::KeepLumi)?;
        let refreshed = worker.snapshot_json()?;
        assert!(refreshed["sourceRefresh"].is_null());
        assert_eq!(refreshed["source"]["revision"], "demo-library-v2");
    }

    let mut restarted = LibraryWorker::demo_at(&path)?;
    restarted.open_editor(horizon_id)?;
    let snapshot = restarted.snapshot_json()?;
    assert_eq!(snapshot["source"]["revision"], "demo-library-v2");
    assert_eq!(snapshot["editor"]["timeline"]["revision"], 4);
    assert_eq!(snapshot["editor"]["phrases"][0]["roleId"], "synth");
    assert_eq!(
        snapshot["editor"]["phrases"][0]["loopStrategy"]["fixedVariantId"],
        "mapping-5"
    );

    let (_, context) = restarted.simulator_track(horizon_id, 4)?.into_parts();
    let theme_resolution = (1..=4)
        .map(|theme_id| {
            let cue = context
                .resolve(ThemeId::new(theme_id))?
                .into_iter()
                .next()
                .ok_or("resolved Theme has no first cue")?;
            Ok(json!({
                "themeId": theme_id,
                "roleId": cue.role_id,
                "strategy": cue.strategy,
                "variantId": cue.variant_id,
                "entryId": cue.entry_id,
            }))
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let synth_role = snapshot["phraseRoleSettings"]["roles"]
        .as_array()
        .and_then(|roles| roles.iter().find(|role| role["id"] == "synth"))
        .ok_or("persisted Synth role is missing")?;
    let peak_time_playlist = snapshot["playlists"]
        .as_array()
        .and_then(|playlists| {
            playlists
                .iter()
                .find(|playlist| playlist["sourcePlaylistId"] == "peak-time")
        })
        .ok_or("persisted Peak Time playlist is missing")?;
    assert_eq!(peak_time_playlist["name"], "Peak Time 2026");
    let phrase_roles = snapshot["editor"]["phrases"]
        .as_array()
        .ok_or("persisted phrases are missing")?
        .iter()
        .map(|phrase| phrase["roleId"].clone())
        .collect::<Vec<_>>();
    let evidence = json!({
        "scenarioVersion": 1,
        "offline": true,
        "source": {
            "id": snapshot["source"]["id"],
            "providerKind": snapshot["providerKind"],
            "revision": snapshot["source"]["revision"],
            "status": snapshot["source"]["status"],
            "peakTimePlaylist": peak_time_playlist["name"],
        },
        "browse": {
            "query": "Horizon Lines",
            "resultCount": 1,
            "trackId": horizon_id,
            "sourceTrackId": snapshot["editor"]["track"]["sourceTrackId"],
        },
        "editor": {
            "analysisRevision": snapshot["editor"]["track"]["analysisRevision"],
            "timelineRevision": snapshot["editor"]["timeline"]["revision"],
            "baselineRevision": snapshot["editor"]["timeline"]["baselineRevision"],
            "reason": snapshot["editor"]["timeline"]["reason"],
            "phraseRoles": phrase_roles,
            "firstPhraseStartBeat": snapshot["editor"]["phrases"][0]["startBeat"],
            "firstPhraseEndBeat": snapshot["editor"]["phrases"][0]["endBeat"],
        },
        "phraseRoleSettings": {
            "revision": snapshot["phraseRoleSettings"]["revision"],
            "stableId": synth_role["id"],
            "displayName": synth_role["name"],
            "archived": synth_role["archived"],
        },
        "loopStrategy": {
            "kind": snapshot["editor"]["phrases"][0]["loopStrategy"]["kind"],
            "variantId": snapshot["editor"]["phrases"][0]["loopStrategy"]["fixedVariantId"],
            "catalogRevision": snapshot["editor"]["phrases"][0]["loopStrategy"]["validatedCatalogRevision"],
        },
        "themeResolution": theme_resolution,
        "persistence": {
            "workerRestarted": true,
            "sourceRefreshPending": !snapshot["sourceRefresh"].is_null(),
        },
    });
    let mut encoded = serde_json::to_vec_pretty(&evidence)?;
    encoded.push(b'\n');
    assert_eq!(
        String::from_utf8_lossy(&encoded),
        String::from_utf8_lossy(include_bytes!(
            "../../../../fixtures/epic-2a-v1/library-editor-e2e.json"
        ))
    );
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn interrupted_source_refresh_reopens_on_last_committed_source_and_resumes()
-> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("lumi-refresh-recovery-{unique}.sqlite"));
    let horizon_id;

    {
        let mut worker = LibraryWorker::demo_at(&path)?;
        horizon_id = worker.snapshot_json()?["page"]["tracks"]
            .as_array()
            .and_then(|tracks| {
                tracks
                    .iter()
                    .find(|track| track["sourceTrackId"] == "horizon-lines")
            })
            .and_then(|track| track["id"].as_u64())
            .ok_or("Horizon Lines is missing")?;
        worker.open_editor(horizon_id)?;
        worker.preview_demo_source_refresh()?;
        worker.reconcile_source_refresh(horizon_id, 1, ReconcileStrategy::ReplaceWithSource)?;
        let partial = worker.snapshot_json()?;
        assert_eq!(partial["source"]["revision"], "demo-library-v1");
        assert_eq!(partial["sourceRefresh"]["changeCount"], 2);
        assert_eq!(
            partial["editor"]["track"]["analysisRevision"],
            "horizon-lines-v2"
        );
        let latest =
            DemoLibrarySourceProvider::curated_revision(DemoLibraryRevision::V2).load_baseline()?;
        worker.repository.restore_source_checkpoint(&latest)?;
    }

    let mut restarted = LibraryWorker::demo_at(&path)?;
    let recovered = restarted.snapshot_json()?;
    assert_eq!(recovered["source"]["revision"], "demo-library-v1");
    assert!(recovered["sourceRefresh"].is_null());
    let peak_time = recovered["playlists"]
        .as_array()
        .and_then(|playlists| {
            playlists
                .iter()
                .find(|playlist| playlist["sourcePlaylistId"] == "peak-time")
        })
        .ok_or("recovered Peak Time playlist is missing")?;
    assert_eq!(peak_time["name"], "Peak Time");
    restarted.preview_demo_source_refresh()?;
    let resumed = restarted.snapshot_json()?;
    assert_eq!(resumed["sourceRefresh"]["changeCount"], 2);
    assert_eq!(resumed["source"]["status"], "changesAvailable");

    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn playback_prefers_an_existing_device_audio_location_when_canonical_is_unmounted()
-> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let mounted = std::env::temp_dir().join(format!("lumi-mounted-audio-{unique}.mp3"));
    std::fs::write(&mounted, b"test audio location")?;
    let mounted_uri = device_audio_uri(&mounted);
    let selected = first_available_audio_uri(
        "file://localhost/Volumes/Disconnected/Track.mp3",
        &[
            "file://localhost/Volumes/Also%20Disconnected/Track.mp3".to_owned(),
            mounted_uri.clone(),
        ],
    );
    assert_eq!(selected, mounted_uri);
    std::fs::remove_file(mounted)?;
    Ok(())
}

#[test]
fn review_comparisons_remain_available_after_inspecting_another_usb()
-> Result<(), Box<dyn std::error::Error>> {
    fn inspection(source_id: &str, device_track_id: u32) -> DeviceInspection {
        DeviceInspection {
            snapshot: DeviceLibrarySnapshot {
                source_id: source_id.to_owned(),
                display_name: source_id.to_owned(),
                database_path: std::path::PathBuf::from("/tmp/exportLibrary.db"),
                database_revision: "revision".to_owned(),
                database_version: "1".to_owned(),
                exported_at: "2026-08-23".to_owned(),
                tracks: BTreeMap::new(),
                playlists: Vec::new(),
            },
            selected_playlist_ids: Vec::new(),
            tracks: BTreeMap::new(),
            review_comparisons: BTreeMap::from([(
                device_track_id,
                DeviceReviewComparison {
                    beat_grid_changed: false,
                    hot_cues_changed: false,
                    file_data_changed: true,
                    raw_phrases_changed: false,
                    waveform_changed: true,
                    beat_grid_detail: "unchanged grid".to_owned(),
                    hot_cues_detail: "unchanged cues".to_owned(),
                    raw_phrases_detail: "unchanged phrases".to_owned(),
                    waveform_detail: "changed waveform".to_owned(),
                    file_detail: "changed file data".to_owned(),
                },
            )]),
        }
    }

    let mut worker = LibraryWorker::demo()?;
    worker.remember_device_inspection(inspection("usb-chrm", 1031));
    worker.remember_device_inspection(inspection("usb-gray", 1256));

    assert!(worker.device_review_comparisons_by_source["usb-chrm"].contains_key(&1031));
    assert!(worker.device_review_comparisons_by_source["usb-gray"].contains_key(&1256));
    assert_eq!(
        worker
            .pending_device_inspection
            .as_ref()
            .map(|current| current.snapshot.source_id.as_str()),
        Some("usb-gray")
    );
    Ok(())
}

#[test]
#[ignore = "requires a mounted OneLibrary USB selected by LUMI_TEST_USB_ROOT"]
fn mounted_usb_inspection_fits_the_local_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::var("LUMI_TEST_USB_ROOT")?;
    let mut worker = LibraryWorker::demo()?;
    let trusted_source_id = std::env::var("LUMI_TEST_USB_SOURCE_ID").ok();
    worker.inspect_rekordbox_device(root, trusted_source_id.as_deref())?;
    let second_source_id = std::env::var("LUMI_TEST_USB_SECOND_SOURCE_ID").ok();
    if let Ok(second_root) = std::env::var("LUMI_TEST_USB_SECOND_ROOT") {
        worker.inspect_rekordbox_device(second_root, second_source_id.as_deref())?;
    }
    let encoded = serde_json::to_vec(&worker.snapshot_json()?)?;
    eprintln!("USB inspection snapshot: {} bytes", encoded.len());
    if let Ok(output_path) = std::env::var("LUMI_TEST_USB_SNAPSHOT_OUTPUT") {
        std::fs::write(output_path, &encoded)?;
    }
    assert!(encoded.len() <= lumi_protocol::MAX_MESSAGE_BYTES);
    if let (Some(first_source_id), Some(second_source_id)) =
        (trusted_source_id.as_deref(), second_source_id.as_deref())
    {
        let snapshot: serde_json::Value = serde_json::from_slice(&encoded)?;
        for source_id in [first_source_id, second_source_id] {
            let source = snapshot["rekordboxDevices"]
                .as_array()
                .and_then(|sources| {
                    sources
                        .iter()
                        .find(|source| source["sourceId"] == source_id)
                })
                .ok_or("inspected USB source missing from snapshot")?;
            assert!(source["reviewTracks"].as_array().is_some_and(|tracks| {
                tracks.iter().all(|track| !track["components"].is_null())
            }));
        }
    }
    Ok(())
}

#[test]
fn creative_timeline_reuse_is_revisioned_and_exact_beat_safe()
-> Result<(), Box<dyn std::error::Error>> {
    let mut worker = LibraryWorker::demo()?;
    let equal_fixture = DemoLibrarySourceProvider::scaled(2)?.load_baseline()?;
    worker.repository.import_baseline(&equal_fixture)?;
    worker.ensure_imported_timelines()?;
    let tracks = worker
        .repository
        .page_tracks(TrackPageRequest::try_new(0, 200)?)?
        .tracks()
        .to_vec();
    let pair = tracks.iter().enumerate().find_map(|(index, source)| {
        let source_beats = worker
            .repository
            .timeline_head(source.id())
            .ok()??
            .total_beats();
        tracks.iter().skip(index + 1).find_map(|target| {
            let target_beats = worker
                .repository
                .timeline_head(target.id())
                .ok()??
                .total_beats();
            (source_beats == target_beats).then_some((source.clone(), target.clone()))
        })
    });
    let (source, target) = pair.ok_or("demo fixture needs an equal-beat track pair")?;
    let source_head = worker
        .repository
        .timeline_head(source.id())?
        .ok_or("source")?;
    let edited = source_head.edit(TimelineEditCommand::ChangeRole {
        phrase_index: 0,
        role_id: PhraseRoleId::try_new("drop")?,
    })?;
    worker
        .repository
        .append_timeline_revision(&edited, Some(source_head.revision()))?;

    let target_before = worker
        .repository
        .timeline_head(target.id())?
        .ok_or("target")?;
    worker.open_editor(target.id().value())?;
    worker.reuse_creative_timeline(
        source.id().value(),
        target.id().value(),
        target_before.revision().value(),
    )?;

    let target_after = worker
        .repository
        .timeline_head(target.id())?
        .ok_or("target")?;
    assert_eq!(
        target_after.revision().value(),
        target_before.revision().value() + 1
    );
    assert_eq!(
        target_after.origin(),
        TimelineRevisionOrigin::RevisionRestore
    );
    assert_eq!(target_after.phrases(), edited.phrases());
    assert_eq!(
        worker
            .repository
            .timeline_head(source.id())?
            .ok_or("source")?,
        edited
    );
    Ok(())
}
