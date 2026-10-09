//! Local authorization for remotely read identity markers. A network reply
//! never creates a trusted source or rewrites track/phrase provenance.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedUsbMedia {
    pub source_id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UsbMediaTrust {
    Trusted(TrustedUsbMedia),
    Unknown,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceInvalidAnalysisTrack {
    pub device_track_id: u32,
    pub title: String,
    pub reason: String,
}

impl SqliteLibraryRepository {
    /// Durable, source-specific exclusions. They remain visible after restart
    /// and disappear only when a subsequent valid sync replaces the alias.
    pub fn device_invalid_analysis_tracks(
        &self,
    ) -> Result<BTreeMap<String, Vec<DeviceInvalidAnalysisTrack>>, SqliteLibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT source_id,device_track_id,title,sync_disposition
             FROM device_library_track_aliases
             WHERE archived=0 AND sync_disposition LIKE 'held-invalid:%'
             ORDER BY source_id,device_track_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut result = BTreeMap::<String, Vec<DeviceInvalidAnalysisTrack>>::new();
        for row in rows {
            let (source, id, title, disposition) = row?;
            result
                .entry(source)
                .or_default()
                .push(DeviceInvalidAnalysisTrack {
                    device_track_id: id,
                    title,
                    reason: disposition.trim_start_matches("held-invalid:").to_owned(),
                });
        }
        Ok(result)
    }

    pub fn with_consistent_read<T, E: From<SqliteLibraryError>>(
        &self,
        read: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(SqliteLibraryError::from)?;
        let result = read();
        // rusqlite also rolls back on Drop if a parser panics; another Player
        // can then prepare rather than inheriting a stranded transaction.
        transaction.rollback().map_err(SqliteLibraryError::from)?;
        result
    }
    /// Connection-local counter for commits made by other Library workers.
    pub fn data_version(&self) -> Result<u64, SqliteLibraryError> {
        from_nonnegative_i64(
            self.connection
                .query_row("PRAGMA data_version", [], |row| row.get(0))?,
            "data version",
        )
    }

    /// Separate bounded reader; no migrations, seeding, journal changes or writes.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, SqliteLibraryError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(Duration::from_millis(100))?;
        connection.execute_batch("PRAGMA query_only = ON;")?;
        Self::register_read_functions(&connection)?;
        Ok(Self { connection })
    }

    /// Called only after local USB validation, against an already authorized
    /// source. Retains the canonical source key and all its aliases/phrases.
    /// A copied marker on a different physical medium becomes a sticky conflict.
    pub fn trust_local_usb_media(
        &mut self,
        media_id: &str,
        marker_source_id: &str,
        source_id: &str,
        physical_source_id: &str,
    ) -> Result<bool, SqliteLibraryError> {
        if !valid_media_id(media_id)
            || !valid_usb_source(marker_source_id)
            || !valid_usb_source(source_id)
            || !valid_usb_source(physical_source_id)
        {
            return Err(SqliteLibraryError::CorruptData(
                "invalid local USB binding".to_owned(),
            ));
        }
        let changed = self.connection.execute(
            "INSERT INTO usb_media_bindings(media_id,marker_source_id,source_id,physical_source_id)
             SELECT ?1,?2,?3,?4 WHERE EXISTS(SELECT 1 FROM device_library_sources WHERE source_id=?3)
             ON CONFLICT(media_id) DO UPDATE SET
               conflicted = CASE WHEN usb_media_bindings.source_id <> excluded.source_id
                                   OR usb_media_bindings.marker_source_id <> excluded.marker_source_id
                                   OR usb_media_bindings.physical_source_id <> excluded.physical_source_id
                                 THEN 1 ELSE usb_media_bindings.conflicted END,
               verified_at = CURRENT_TIMESTAMP",
            params![media_id, marker_source_id, source_id, physical_source_id],
        )?;
        Ok(changed != 0)
    }

    pub fn trusted_usb_media(
        &self,
        media_id: &str,
        marker_source_id: &str,
    ) -> Result<UsbMediaTrust, SqliteLibraryError> {
        // Older installations stay read-only until their normal writer migrates.
        if !self.table_exists("usb_media_bindings")? {
            return Ok(UsbMediaTrust::Unknown);
        }
        let row: Option<(String, String, String, bool)> = self
            .connection
            .query_row(
                "SELECT b.marker_source_id,b.source_id,s.display_name,b.conflicted
             FROM usb_media_bindings b JOIN device_library_sources s ON s.source_id=b.source_id
             WHERE b.media_id=?1",
                [media_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        Ok(match row {
            Some((_, _, _, true)) => UsbMediaTrust::Conflict,
            Some((marker, source_id, display_name, false)) if marker == marker_source_id => {
                UsbMediaTrust::Trusted(TrustedUsbMedia {
                    source_id,
                    display_name,
                })
            }
            Some(_) => UsbMediaTrust::Conflict,
            None => UsbMediaTrust::Unknown,
        })
    }

    /// Requires the resolver's verified canonical source. Numeric RB IDs are
    /// never searched across sticks on this path, including over-Link loads.
    pub fn resolve_device_alias_for_source(
        &self,
        source_id: &str,
        device_track_id: u32,
    ) -> Result<Option<DeviceAliasResolution>, SqliteLibraryError> {
        self.connection.query_row(
            "SELECT a.source_id,s.display_name,a.device_track_id,a.canonical_track_id,a.analysis_revision,a.match_kind
             FROM device_library_track_aliases a JOIN device_library_sources s ON s.source_id=a.source_id
             WHERE a.source_id=?1 AND a.device_track_id=?2 AND a.archived=0 AND a.canonical_track_id IS NOT NULL",
            params![source_id, i64::from(device_track_id)],
            |row| Ok(DeviceAliasResolution {
                source_id: row.get(0)?, display_name: row.get(1)?, device_track_id,
                canonical_track_id: TrackId::new(u64::try_from(row.get::<_, i64>(3)?).map_err(|_| {
                    rusqlite::Error::IntegralValueOutOfRange(3, row.get::<_, i64>(3).unwrap_or(-1))
                })?),
                analysis_revision: row.get(4)?, match_kind: row.get(5)?,
            }),
        ).optional().map_err(Into::into)
    }
}

fn valid_media_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

fn valid_usb_source(value: &str) -> bool {
    (8..=200).contains(&value.len())
        && (value.starts_with("usb-fs:") || value.starts_with("usb-local:"))
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b":-_".contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;
    const MEDIA: &str = "cb28a682-9327-4fda-b38c-8fbd8eacc92e";
    const MARKER: &str = "usb-fs:v2-marker";
    const LEGACY: &str = "usb-fs:hardware-existing";

    fn source(repository: &SqliteLibraryRepository, id: &str) -> Result<(), SqliteLibraryError> {
        repository.connection.execute(
            "INSERT INTO device_library_sources(source_id,display_name,database_revision) VALUES (?1,?1,'old')", [id])?;
        Ok(())
    }

    #[test]
    fn reader_snapshot_does_not_mix_a_concurrent_sync_and_is_released_on_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("lumi-consistent-read-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let path = root.join("library.sqlite");
        let writer = SqliteLibraryRepository::open(&path)?;
        source(&writer, LEGACY)?;
        let reader = SqliteLibraryRepository::open_read_only(&path)?;
        let version = reader.data_version()?;
        reader.with_consistent_read(|| -> Result<(), SqliteLibraryError> {
            let name: String = reader.connection.query_row(
                "SELECT display_name FROM device_library_sources",
                [],
                |r| r.get(0),
            )?;
            writer.connection.execute(
                "UPDATE device_library_sources SET display_name='Changed'",
                [],
            )?;
            let within: String = reader.connection.query_row(
                "SELECT display_name FROM device_library_sources",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(within, name);
            Ok(())
        })?;
        assert!(reader.data_version()? > version);
        let failed = reader.with_consistent_read(|| -> Result<(), SqliteLibraryError> {
            Err(SqliteLibraryError::CorruptData(
                "injected read fault".into(),
            ))
        });
        assert!(failed.is_err());
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            reader.with_consistent_read(|| -> Result<(), SqliteLibraryError> {
                panic!("injected parser fault");
            })
        }));
        assert!(panic.is_err());
        reader.with_consistent_read(|| -> Result<(), SqliteLibraryError> {
            let name: String = reader.connection.query_row(
                "SELECT display_name FROM device_library_sources",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(name, "Changed");
            Ok(())
        })?;
        drop(reader);
        drop(writer);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn network_marker_cannot_authorize_a_source() -> Result<(), SqliteLibraryError> {
        let mut repository = SqliteLibraryRepository::in_memory()?;
        assert_eq!(
            repository.trusted_usb_media(MEDIA, MARKER)?,
            UsbMediaTrust::Unknown
        );
        assert!(!repository.trust_local_usb_media(MEDIA, MARKER, LEGACY, MARKER)?);
        assert!(repository.device_source_summaries()?.is_empty());
        assert!(
            repository
                .trust_local_usb_media("invalid", MARKER, LEGACY, MARKER)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn local_binding_preserves_legacy_source_and_rejects_copied_markers()
    -> Result<(), SqliteLibraryError> {
        let mut repository = SqliteLibraryRepository::in_memory()?;
        source(&repository, LEGACY)?;
        source(&repository, "usb-fs:v2-other")?;
        assert!(repository.trust_local_usb_media(MEDIA, MARKER, LEGACY, MARKER)?);
        assert_eq!(
            repository.trusted_usb_media(MEDIA, MARKER)?,
            UsbMediaTrust::Trusted(TrustedUsbMedia {
                source_id: LEGACY.to_owned(),
                display_name: LEGACY.to_owned(),
            })
        );
        assert_eq!(
            repository.trusted_usb_media(MEDIA, "usb-fs:forged")?,
            UsbMediaTrust::Conflict
        );
        repository.trust_local_usb_media(MEDIA, MARKER, "usb-fs:v2-other", "usb-fs:v2-other")?;
        repository.trust_local_usb_media(MEDIA, MARKER, LEGACY, MARKER)?;
        assert_eq!(
            repository.trusted_usb_media(MEDIA, MARKER)?,
            UsbMediaTrust::Conflict
        );
        assert_eq!(repository.device_source_summaries()?.len(), 2);
        Ok(())
    }

    #[test]
    fn separate_reader_is_read_only_and_supports_older_schema()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("lumi-readonly-media-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let path = root.join("library.sqlite");
        {
            let mut repository = SqliteLibraryRepository::open(&path)?;
            source(&repository, LEGACY)?;
            repository.trust_local_usb_media(MEDIA, MARKER, LEGACY, MARKER)?;
            let mut reader = SqliteLibraryRepository::open_read_only(&path)?;
            assert!(matches!(
                reader.trusted_usb_media(MEDIA, MARKER)?,
                UsbMediaTrust::Trusted(_)
            ));
            assert!(
                reader
                    .trust_local_usb_media(MEDIA, MARKER, LEGACY, MARKER)
                    .is_err()
            );
            drop(reader);
            repository
                .connection
                .execute_batch("DROP TABLE usb_media_bindings; DROP TABLE track_audio_fingerprints; PRAGMA user_version=18;")?;
        }
        {
            let reader = SqliteLibraryRepository::open_read_only(&path)?;
            assert_eq!(reader.schema_version()?, 18);
            assert_eq!(
                reader.trusted_usb_media(MEDIA, MARKER)?,
                UsbMediaTrust::Unknown
            );
        }
        assert_eq!(SqliteLibraryRepository::open(&path)?.schema_version()?, 21);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn colliding_track_ids_resolve_only_inside_the_verified_source()
    -> Result<(), Box<dyn std::error::Error>> {
        use lumi_library_source::MusicLibrarySourceProvider as _;
        let mut repository = SqliteLibraryRepository::in_memory()?;
        repository.import_baseline(
            &lumi_library_demo::DemoLibrarySourceProvider::curated().load_baseline()?,
        )?;
        source(&repository, LEGACY)?;
        source(&repository, "usb-fs:other")?;
        for (id, offset) in [(LEGACY, 0), ("usb-fs:other", 1)] {
            repository.connection.execute(
                "INSERT INTO device_library_track_aliases(source_id,device_track_id,simulator_signature,
                 canonical_track_id,match_kind,title,artist,bpm_milli,duration_millis,file_size,
                 metadata_revision,analysis_revision,analyzed_at,sync_disposition,archived)
                 SELECT ?1,42,0,id,'exact','title','artist',130000,100000,100,'metadata','analysis','date','current',0
                 FROM tracks ORDER BY id LIMIT 1 OFFSET ?2", params![id, offset])?;
        }
        let one = repository
            .resolve_device_alias_for_source(LEGACY, 42)?
            .ok_or("missing alias")?;
        let two = repository
            .resolve_device_alias_for_source("usb-fs:other", 42)?
            .ok_or("missing alias")?;
        assert_ne!(one.canonical_track_id, two.canonical_track_id);
        assert!(repository.resolve_device_alias(42, 0)?.is_none());
        assert!(
            repository
                .resolve_device_alias_for_source("usb-fs:unknown", 42)?
                .is_none()
        );
        repository.connection.execute(
            "UPDATE device_library_track_aliases SET archived=1 WHERE source_id=?1",
            [LEGACY],
        )?;
        assert!(
            repository
                .resolve_device_alias_for_source(LEGACY, 42)?
                .is_none()
        );
        assert!(
            repository
                .resolve_device_alias_for_source("usb-fs:other", 42)?
                .is_some()
        );
        Ok(())
    }
}
