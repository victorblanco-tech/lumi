//! Persistent channel-scoped TLS identity for the LAN-facing gateway.

#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rcgen::{CertifiedKey, generate_simple_self_signed};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};

const INSTALLATION_ID_FILE: &str = "installation-id";
const CERTIFICATE_FILE: &str = "certificate.der";
const PRIVATE_KEY_FILE: &str = "private-key.der";
const MAXIMUM_IDENTITY_FILE_BYTES: u64 = 64 * 1_024;

#[derive(Clone)]
pub struct InstallationIdentity {
    pub installation_id: String,
    pub certificate_der: Vec<u8>,
    private_key_der: Vec<u8>,
    pub certificate_fingerprint_sha256: String,
    pub server_name: String,
}

impl InstallationIdentity {
    pub fn load_or_create(directory: &Path) -> Result<Self, IdentityError> {
        ensure_private_directory(directory)?;
        let installation_path = directory.join(INSTALLATION_ID_FILE);
        let certificate_path = directory.join(CERTIFICATE_FILE);
        let private_key_path = directory.join(PRIVATE_KEY_FILE);
        let existing = [
            installation_path.exists(),
            certificate_path.exists(),
            private_key_path.exists(),
        ];
        if existing.iter().any(|value| *value) && !existing.iter().all(|value| *value) {
            return Err(IdentityError::IncompleteIdentity);
        }

        if existing.iter().all(|value| *value) {
            let installation_id = String::from_utf8(read_private_file(&installation_path)?)?
                .trim()
                .to_owned();
            validate_installation_id(&installation_id)?;
            let certificate_der = read_private_file(&certificate_path)?;
            let private_key_der = read_private_file(&private_key_path)?;
            return Self::from_parts(installation_id, certificate_der, private_key_der);
        }

        let installation_id = random_hex(16)?;
        let server_name = server_name(&installation_id);
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec![server_name.clone()])?;
        let certificate_der = cert.der().to_vec();
        let private_key_der = signing_key.serialize_der();
        atomic_write(&installation_path, installation_id.as_bytes())?;
        atomic_write(&certificate_path, &certificate_der)?;
        atomic_write(&private_key_path, &private_key_der)?;
        Self::from_parts(installation_id, certificate_der, private_key_der)
    }

    fn from_parts(
        installation_id: String,
        certificate_der: Vec<u8>,
        private_key_der: Vec<u8>,
    ) -> Result<Self, IdentityError> {
        if certificate_der.is_empty() || private_key_der.is_empty() {
            return Err(IdentityError::IncompleteIdentity);
        }
        let fingerprint = Sha256::digest(&certificate_der);
        let identity = Self {
            server_name: server_name(&installation_id),
            installation_id,
            certificate_der,
            private_key_der,
            certificate_fingerprint_sha256: format!("{fingerprint:x}"),
        };
        // Validates that the persisted certificate and key still form a
        // usable TLS identity before any LAN listener can be advertised.
        let _ = identity.tls_server_config()?;
        Ok(identity)
    }

    pub fn tls_server_config(&self) -> Result<Arc<ServerConfig>, IdentityError> {
        let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
        let builder = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(IdentityError::TlsConfiguration)?;
        let config = builder
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(self.certificate_der.clone())],
                PrivatePkcs8KeyDer::from(self.private_key_der.clone()).into(),
            )
            .map_err(IdentityError::TlsConfiguration)?;
        Ok(Arc::new(config))
    }
}

pub fn random_hex(byte_count: usize) -> Result<String, IdentityError> {
    if !(8..=256).contains(&byte_count) {
        return Err(IdentityError::InvalidRandomLength);
    }
    let mut bytes = vec![0_u8; byte_count];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn ensure_private_directory(directory: &Path) -> Result<(), IdentityError> {
    fs::create_dir_all(directory)?;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(IdentityError::UntrustedPath(directory.to_owned()));
    }
    Ok(())
}

fn read_private_file(path: &Path) -> Result<Vec<u8>, IdentityError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > MAXIMUM_IDENTITY_FILE_BYTES
    {
        return Err(IdentityError::UntrustedPath(path.to_owned()));
    }
    Ok(fs::read(path)?)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), IdentityError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| IdentityError::UntrustedPath(path.to_owned()))?;
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn validate_installation_id(value: &str) -> Result<(), IdentityError> {
    if value.len() != 32 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(IdentityError::InvalidInstallationIdentity);
    }
    Ok(())
}

fn server_name(installation_id: &str) -> String {
    let prefix = installation_id.get(..12).unwrap_or(installation_id);
    format!("lumi-{prefix}.local")
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("remote identity path is not a protected regular path: {0}")]
    UntrustedPath(PathBuf),
    #[error("remote installation identity is invalid")]
    InvalidInstallationIdentity,
    #[error("remote TLS identity is only partially present")]
    IncompleteIdentity,
    #[error("random secret length is invalid")]
    InvalidRandomLength,
    #[error("remote identity I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("remote installation identity is not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("remote certificate generation failed: {0}")]
    Certificate(#[from] rcgen::Error),
    #[error("remote TLS configuration failed: {0}")]
    TlsConfiguration(tokio_rustls::rustls::Error),
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    use super::InstallationIdentity;

    #[test]
    fn persists_one_private_channel_scoped_identity() -> Result<(), Box<dyn Error>> {
        let directory =
            std::env::temp_dir().join(format!("lumi-remote-identity-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let first = InstallationIdentity::load_or_create(&directory)?;
        let second = InstallationIdentity::load_or_create(&directory)?;
        assert_eq!(first.installation_id, second.installation_id);
        assert_eq!(
            first.certificate_fingerprint_sha256,
            second.certificate_fingerprint_sha256
        );
        assert_eq!(
            fs::metadata(directory.join("private-key.der"))?
                .permissions()
                .mode()
                & 0o077,
            0
        );
        let _ = fs::remove_dir_all(directory);
        Ok(())
    }
}
