//! Crash-safe persistence for revocable device credential verifiers.

#![forbid(unsafe_code)]

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{PairingError, PairingRegistry, PairingRegistrySnapshot};

const TRUST_STORE_VERSION: u16 = 1;
const MAXIMUM_TRUST_STORE_BYTES: u64 = 64 * 1_024;

#[derive(Clone, Debug)]
pub struct PersistentTrustStore {
    path: PathBuf,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrustStoreFile {
    version: u16,
    registry: PairingRegistrySnapshot,
}

impl PersistentTrustStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<PairingRegistry, TrustStoreError> {
        if !self.path.exists() {
            return Ok(PairingRegistry::default());
        }
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.len() > MAXIMUM_TRUST_STORE_BYTES
        {
            return Err(TrustStoreError::UntrustedPath(self.path.clone()));
        }
        let store: TrustStoreFile = serde_json::from_slice(&fs::read(&self.path)?)?;
        if store.version != TRUST_STORE_VERSION {
            return Err(TrustStoreError::UnsupportedVersion(store.version));
        }
        PairingRegistry::from_snapshot(store.registry).map_err(TrustStoreError::InvalidRegistry)
    }

    pub fn save(&self, registry: &PairingRegistry) -> Result<(), TrustStoreError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| TrustStoreError::UntrustedPath(self.path.clone()))?;
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let parent_metadata = fs::symlink_metadata(parent)?;
        if !parent_metadata.is_dir()
            || parent_metadata.file_type().is_symlink()
            || parent_metadata.permissions().mode() & 0o077 != 0
        {
            return Err(TrustStoreError::UntrustedPath(parent.to_owned()));
        }
        let bytes = serde_json::to_vec(&TrustStoreFile {
            version: TRUST_STORE_VERSION,
            registry: registry.snapshot(),
        })?;
        if bytes.len() as u64 > MAXIMUM_TRUST_STORE_BYTES {
            return Err(TrustStoreError::Oversized);
        }
        let file_name = self
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| TrustStoreError::UntrustedPath(self.path.clone()))?;
        let temporary = self.path.with_file_name(format!(
            ".{file_name}.{}.tmp",
            crate::random_hex(12).map_err(std::io::Error::other)?
        ));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temporary)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            // Rename is the commit point; no fallible operation follows it.
            fs::rename(&temporary, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(TrustStoreError::Io)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Error)]
pub enum TrustStoreError {
    #[error("remote trust-store path is not protected: {0}")]
    UntrustedPath(PathBuf),
    #[error("remote trust-store version {0} is unsupported")]
    UnsupportedVersion(u16),
    #[error("remote trust store exceeds its bounded size")]
    Oversized,
    #[error("remote trust store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("remote trust store JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("remote trust registry is invalid: {0}")]
    InvalidRegistry(PairingError),
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;

    use crate::{PairingInvitationRequest, PairingRegistry, PersistentTrustStore, random_hex};

    #[test]
    fn stores_only_the_device_credential_verifier_and_survives_restart()
    -> Result<(), Box<dyn Error>> {
        let directory =
            std::env::temp_dir().join(format!("lumi-remote-trust-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let store = PersistentTrustStore::new(directory.join("trust.json"));
        let mut registry = PairingRegistry::default();
        let first_invitation_secret = random_hex(32)?;
        registry.create_invitation(PairingInvitationRequest {
            invitation_id: "invitation-123456".to_owned(),
            invitation_secret: first_invitation_secret,
            short_code: "123456".to_owned(),
            certificate_fingerprint_sha256: "a".repeat(64),
            created_at_unix_millis: 10,
            expires_at_unix_millis: 1_000,
        })?;
        registry.approve("invitation-123456", "123456")?;
        let credential = random_hex(32)?;
        assert!(
            registry
                .exchange(
                    "invitation-123456",
                    // Obtain a deterministic secret for this fixture by replacing
                    // the registry above with one built around this value.
                    "not-the-secret",
                    "iphone-1".to_owned(),
                    "Test iPhone".to_owned(),
                    &credential,
                    20,
                )
                .is_err()
        );
        let invitation_secret = random_hex(32)?;
        registry.create_invitation(PairingInvitationRequest {
            invitation_id: "invitation-654321".to_owned(),
            invitation_secret: invitation_secret.clone(),
            short_code: "654321".to_owned(),
            certificate_fingerprint_sha256: "b".repeat(64),
            created_at_unix_millis: 10,
            expires_at_unix_millis: 1_000,
        })?;
        registry.approve("invitation-654321", "654321")?;
        registry.exchange(
            "invitation-654321",
            &invitation_secret,
            "iphone-1".to_owned(),
            "Test iPhone".to_owned(),
            &credential,
            20,
        )?;
        store.save(&registry)?;
        let bytes = fs::read(store.path())?;
        assert!(!String::from_utf8_lossy(&bytes).contains(&credential));
        let mut reloaded = store.load()?;
        assert_eq!(reloaded.controller_device_id(), Some("iphone-1"));
        reloaded.authenticate("iphone-1", &credential, 30)?;
        let _ = fs::remove_dir_all(directory);
        Ok(())
    }
}
