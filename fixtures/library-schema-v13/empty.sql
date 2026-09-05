-- Historical schema 13 fixture, frozen before the E10 restore correction.
BEGIN IMMEDIATE;
CREATE TABLE library_sources (
    source_id TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL,
    display_name TEXT NOT NULL,
    source_revision TEXT NOT NULL
);
CREATE TABLE import_baselines (
    source_id TEXT NOT NULL,
    source_revision TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    display_name TEXT NOT NULL,
    track_count INTEGER NOT NULL,
    playlist_count INTEGER NOT NULL,
    PRIMARY KEY(source_id, source_revision)
);
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id TEXT NOT NULL REFERENCES library_sources(source_id),
    source_track_id TEXT NOT NULL,
    analysis_revision TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    bpm_milli INTEGER NOT NULL,
    key_pitch TEXT NOT NULL,
    key_mode TEXT NOT NULL,
    duration_millis INTEGER NOT NULL,
    color_rgb INTEGER,
    audio_uri TEXT NOT NULL,
    UNIQUE(source_id, source_track_id)
);
CREATE INDEX tracks_title_artist ON tracks(title, artist, id);
CREATE TABLE playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id TEXT NOT NULL REFERENCES library_sources(source_id),
    source_playlist_id TEXT NOT NULL,
    name TEXT NOT NULL,
    UNIQUE(source_id, source_playlist_id)
);
CREATE INDEX playlists_name ON playlists(name, id);
CREATE TABLE playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY(playlist_id, position),
    UNIQUE(playlist_id, track_id)
);
CREATE TABLE beat_grids (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    beats_per_bar INTEGER NOT NULL
);
CREATE TABLE beat_markers (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    beat_index INTEGER NOT NULL,
    time_millis INTEGER NOT NULL,
    bar_index INTEGER NOT NULL,
    beat_in_bar INTEGER NOT NULL,
    PRIMARY KEY(track_id, beat_index)
);
CREATE TABLE waveform_points (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    point_index INTEGER NOT NULL,
    low INTEGER NOT NULL,
    mid INTEGER NOT NULL,
    high INTEGER NOT NULL,
    PRIMARY KEY(track_id, point_index)
);
CREATE TABLE raw_phrases (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    phrase_index INTEGER NOT NULL,
    start_beat INTEGER NOT NULL,
    end_beat INTEGER NOT NULL,
    source_label TEXT NOT NULL,
    PRIMARY KEY(track_id, phrase_index)
);
CREATE TABLE hot_cues (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    cue_index INTEGER NOT NULL,
    time_millis INTEGER NOT NULL,
    loop_end_millis INTEGER,
    name TEXT NOT NULL,
    color_rgb INTEGER NOT NULL,
    PRIMARY KEY(track_id, cue_index)
);
CREATE TABLE phrase_roles (
    role_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    archived INTEGER NOT NULL
);
CREATE TABLE library_settings (
    key TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);
INSERT INTO library_settings(key, value)
    VALUES ('phrase-role-defaults-version', 0),
           ('phrase-role-catalog-revision', 0),
           ('autoloop-catalog-defaults-version', 0),
           ('autoloop-catalog-revision', 0),
           ('suppress-demo-seed', 0);
CREATE TABLE source_phrase_mappings (
    provider_kind TEXT NOT NULL,
    normalized_label TEXT NOT NULL,
    raw_label TEXT NOT NULL,
    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
    PRIMARY KEY(provider_kind, normalized_label)
);
CREATE TABLE autoloop_themes (
    theme_id INTEGER PRIMARY KEY,
    display_name TEXT NOT NULL,
    sort_order INTEGER NOT NULL UNIQUE
);
CREATE TABLE autoloop_variants (
    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
    variant_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    archived INTEGER NOT NULL,
    PRIMARY KEY(role_id, variant_id),
    UNIQUE(role_id, sort_order)
);
CREATE TABLE autoloop_matrix_cells (
    theme_id INTEGER NOT NULL REFERENCES autoloop_themes(theme_id),
    role_id TEXT NOT NULL,
    variant_id TEXT NOT NULL,
    entry_id TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    PRIMARY KEY(theme_id, role_id, variant_id),
    FOREIGN KEY(role_id, variant_id)
        REFERENCES autoloop_variants(role_id, variant_id)
);
CREATE TABLE timeline_revisions (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    baseline_revision TEXT NOT NULL,
    total_beats INTEGER NOT NULL,
    origin TEXT NOT NULL,
    reason TEXT NOT NULL,
    parent_revision INTEGER,
    restored_from_revision INTEGER,
    PRIMARY KEY(track_id, revision)
);
CREATE TABLE phrase_points (
    track_id INTEGER NOT NULL,
    revision INTEGER NOT NULL,
    phrase_index INTEGER NOT NULL,
    beat INTEGER NOT NULL,
    role_id TEXT NOT NULL,
    loop_strategy TEXT NOT NULL,
    PRIMARY KEY(track_id, revision, phrase_index),
    FOREIGN KEY(track_id, revision)
        REFERENCES timeline_revisions(track_id, revision) ON DELETE CASCADE
);
CREATE TABLE timeline_heads (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    FOREIGN KEY(track_id, revision)
        REFERENCES timeline_revisions(track_id, revision)
);
CREATE TABLE source_mirrors (
    source_id TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL,
    display_name TEXT NOT NULL,
    source_revision TEXT NOT NULL
);
CREATE TABLE source_mirror_tracks (
    source_id TEXT NOT NULL REFERENCES source_mirrors(source_id),
    source_track_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT,
    average_bpm TEXT,
    musical_key TEXT,
    duration_millis INTEGER,
    color TEXT,
    audio_uri TEXT NOT NULL,
    archived INTEGER NOT NULL,
    last_seen_revision TEXT NOT NULL,
    PRIMARY KEY(source_id, source_track_id)
);
CREATE INDEX source_mirror_tracks_active
    ON source_mirror_tracks(source_id, archived, source_track_id);
CREATE TABLE source_mirror_playlists (
    source_id TEXT NOT NULL REFERENCES source_mirrors(source_id),
    source_playlist_id TEXT NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY(source_id, source_playlist_id)
);
CREATE TABLE source_mirror_playlist_tracks (
    source_id TEXT NOT NULL,
    source_playlist_id TEXT NOT NULL,
    source_track_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY(source_id, source_playlist_id, position),
    UNIQUE(source_id, source_playlist_id, source_track_id),
    FOREIGN KEY(source_id, source_playlist_id)
        REFERENCES source_mirror_playlists(source_id, source_playlist_id)
        ON DELETE CASCADE,
    FOREIGN KEY(source_id, source_track_id)
        REFERENCES source_mirror_tracks(source_id, source_track_id)
);
CREATE TABLE source_mirror_revisions (
    source_id TEXT NOT NULL,
    source_revision TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    active_track_count INTEGER NOT NULL,
    archived_track_count INTEGER NOT NULL,
    playlist_count INTEGER NOT NULL,
    PRIMARY KEY(source_id, source_revision)
);
CREATE TABLE device_library_sources (
    source_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    database_revision TEXT NOT NULL,
    synced_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE device_library_track_aliases (
    source_id TEXT NOT NULL REFERENCES device_library_sources(source_id),
    device_track_id INTEGER NOT NULL,
    simulator_signature INTEGER NOT NULL,
    canonical_track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
    match_kind TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    bpm_milli INTEGER NOT NULL,
    duration_millis INTEGER NOT NULL,
    file_size INTEGER NOT NULL,
    metadata_revision TEXT NOT NULL,
    analysis_revision TEXT NOT NULL,
    audio_signature TEXT NOT NULL DEFAULT '',
    analyzed_at TEXT NOT NULL,
    sync_disposition TEXT NOT NULL,
    archived INTEGER NOT NULL,
    PRIMARY KEY(source_id, device_track_id)
);
CREATE INDEX device_alias_by_simulator_signature
    ON device_library_track_aliases(simulator_signature, archived);
CREATE INDEX device_alias_by_canonical_track
    ON device_library_track_aliases(canonical_track_id, archived);
CREATE TABLE track_analysis_provenance (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL,
    device_track_id INTEGER NOT NULL,
    analysis_revision TEXT NOT NULL,
    analyzed_at TEXT NOT NULL,
    hot_cues_loaded INTEGER NOT NULL DEFAULT 0,
    promoted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE track_hot_cue_provenance (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL,
    device_track_id INTEGER NOT NULL,
    analysis_revision TEXT NOT NULL,
    analyzed_at TEXT NOT NULL,
    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE creative_track_archives (
    archive_id INTEGER PRIMARY KEY AUTOINCREMENT,
    identity_key TEXT NOT NULL UNIQUE,
    original_track_id INTEGER,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    bpm_milli INTEGER NOT NULL,
    duration_millis INTEGER NOT NULL,
    audio_signature TEXT NOT NULL DEFAULT '',
    total_beats INTEGER NOT NULL,
    source_timeline_revision INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    restored_track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX creative_archives_state
    ON creative_track_archives(state, archive_id);
CREATE TABLE creative_phrase_points (
    archive_id INTEGER NOT NULL REFERENCES creative_track_archives(archive_id)
        ON DELETE CASCADE,
    phrase_index INTEGER NOT NULL,
    beat INTEGER NOT NULL,
    role_id TEXT NOT NULL REFERENCES phrase_roles(role_id),
    loop_strategy TEXT NOT NULL,
    PRIMARY KEY(archive_id, phrase_index)
);
CREATE TABLE light_planning_policy (
    singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
    revision INTEGER NOT NULL,
    policy_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
PRAGMA user_version = 12;
COMMIT;

BEGIN IMMEDIATE;
CREATE TABLE track_metadata_provenance (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL,
    device_track_id INTEGER NOT NULL,
    metadata_revision TEXT NOT NULL,
    analyzed_at TEXT NOT NULL,
    master_database_id INTEGER NOT NULL,
    master_content_id INTEGER NOT NULL,
    information_update_count INTEGER NOT NULL,
    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
PRAGMA user_version = 13;
COMMIT;
