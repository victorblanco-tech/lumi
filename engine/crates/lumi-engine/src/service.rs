//! Per-user launchd service bootstrap and discovery record.

#![forbid(unsafe_code)]

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use lumi_protocol::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::StartupReady;

pub const SERVICE_MODE_ENVIRONMENT_KEY: &str = "LUMI_SERVICE_MODE";
pub const DATA_DIRECTORY_ENVIRONMENT_KEY: &str = "LUMI_DATA_DIRECTORY_NAME";
pub const PRODUCT_VERSION_ENVIRONMENT_KEY: &str = "LUMI_PRODUCT_VERSION";
pub const BUILD_NUMBER_ENVIRONMENT_KEY: &str = "LUMI_BUILD_NUMBER";
pub const SESSION_TOKEN_FILE_NAME: &str = ".engine-session-token";
pub const SERVICE_RECORD_FILE_NAME: &str = "engine-service.json";

const LAUNCHD_SERVICE_MODE: &str = "launchd";
const SESSION_TOKEN_ENVIRONMENT_KEY: &str = "LUMI_SESSION_TOKEN";
const MINIMUM_SESSION_TOKEN_BYTES: usize = 32;
const MAXIMUM_SESSION_TOKEN_BYTES: usize = 256;

#[derive(Clone, Debug)]
pub struct ServiceBootstrap {
    pub session_token: String,
    pub record_path: Option<PathBuf>,
    pub product_version: Option<String>,
    pub build_number: Option<String>,
}

#[derive(Debug)]
pub struct ServiceRecordGuard {
    path: PathBuf,
    process_id: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceRecord {
    endpoint: StartupReady,
    session_token: String,
    #[serde(rename = "processID")]
    process_id: u32,
    product_version: String,
    service_identity: ServiceIdentity,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceIdentity {
    product_version: String,
    build_number: String,
    engine_executable_path: String,
    #[serde(rename = "engineExecutableSHA256")]
    engine_executable_sha256: String,
}

impl ServiceBootstrap {
    pub fn resolve() -> Result<Self, ServiceBootstrapError> {
        if env::var(SERVICE_MODE_ENVIRONMENT_KEY).as_deref() != Ok(LAUNCHD_SERVICE_MODE) {
            let session_token = env::var(SESSION_TOKEN_ENVIRONMENT_KEY)
                .map_err(|_| ServiceBootstrapError::MissingSessionToken)?;
            validate_session_token(&session_token)?;
            return Ok(Self {
                session_token,
                record_path: None,
                product_version: None,
                build_number: None,
            });
        }

        let data_directory = channel_data_directory()?;
        fs::create_dir_all(&data_directory)?;
        fs::set_permissions(&data_directory, fs::Permissions::from_mode(0o700))?;
        let token_path = data_directory.join(SESSION_TOKEN_FILE_NAME);
        let session_token = fs::read_to_string(&token_path)?.trim().to_owned();
        validate_session_token(&session_token)?;
        let product_version = required_environment(PRODUCT_VERSION_ENVIRONMENT_KEY)?;
        let build_number = required_environment(BUILD_NUMBER_ENVIRONMENT_KEY)?;
        Ok(Self {
            session_token,
            record_path: Some(data_directory.join(SERVICE_RECORD_FILE_NAME)),
            product_version: Some(product_version),
            build_number: Some(build_number),
        })
    }

    pub fn publish_record(
        &self,
        port: u16,
    ) -> Result<Option<ServiceRecordGuard>, ServiceBootstrapError> {
        let Some(record_path) = &self.record_path else {
            return Ok(None);
        };
        let product_version = self
            .product_version
            .as_ref()
            .ok_or(ServiceBootstrapError::MissingProductVersion)?;
        let build_number = self
            .build_number
            .as_ref()
            .ok_or(ServiceBootstrapError::MissingBuildNumber)?;
        let executable = env::current_exe()?.canonicalize()?;
        let executable_sha256 = sha256_file(&executable)?;
        let process_id = std::process::id();
        let record = ServiceRecord {
            endpoint: StartupReady {
                record_type: "engineReady".to_owned(),
                host: "127.0.0.1".to_owned(),
                port,
                protocol_version: PROTOCOL_VERSION,
            },
            session_token: self.session_token.clone(),
            process_id,
            product_version: product_version.clone(),
            service_identity: ServiceIdentity {
                product_version: product_version.clone(),
                build_number: build_number.clone(),
                engine_executable_path: executable.to_string_lossy().into_owned(),
                engine_executable_sha256: executable_sha256,
            },
        };
        let encoded = serde_json::to_vec(&record)?;
        let temporary_path =
            record_path.with_file_name(format!(".{}.{}.tmp", SERVICE_RECORD_FILE_NAME, process_id));
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&temporary_path)?;
        file.write_all(&encoded)?;
        file.sync_all()?;
        fs::rename(&temporary_path, record_path)?;
        Ok(Some(ServiceRecordGuard {
            path: record_path.clone(),
            process_id,
        }))
    }
}

impl Drop for ServiceRecordGuard {
    fn drop(&mut self) {
        let current_process = fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<ServiceRecord>(&bytes).ok())
            .map(|record| record.process_id);
        if current_process == Some(self.process_id) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn configured_database_path() -> Result<Option<PathBuf>, ServiceBootstrapError> {
    if let Some(path) = env::var_os("LUMI_LIBRARY_DATABASE_PATH") {
        return Ok(Some(PathBuf::from(path)));
    }
    if env::var(SERVICE_MODE_ENVIRONMENT_KEY).as_deref() == Ok(LAUNCHD_SERVICE_MODE) {
        return Ok(Some(channel_data_directory()?.join("library.sqlite")));
    }
    Ok(None)
}

fn channel_data_directory() -> Result<PathBuf, ServiceBootstrapError> {
    let home = env::var_os("HOME").ok_or(ServiceBootstrapError::MissingHomeDirectory)?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        return Err(ServiceBootstrapError::InvalidHomeDirectory);
    }
    let directory_name = required_environment(DATA_DIRECTORY_ENVIRONMENT_KEY)?;
    validate_directory_name(&directory_name)?;
    Ok(home
        .join("Library")
        .join("Application Support")
        .join(directory_name))
}

fn required_environment(key: &'static str) -> Result<String, ServiceBootstrapError> {
    env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or(match key {
            PRODUCT_VERSION_ENVIRONMENT_KEY => ServiceBootstrapError::MissingProductVersion,
            BUILD_NUMBER_ENVIRONMENT_KEY => ServiceBootstrapError::MissingBuildNumber,
            DATA_DIRECTORY_ENVIRONMENT_KEY => ServiceBootstrapError::MissingDataDirectory,
            _ => ServiceBootstrapError::MissingConfiguration(key),
        })
}

fn validate_directory_name(value: &str) -> Result<(), ServiceBootstrapError> {
    if value == "." || value == ".." || value.contains('/') || value.contains('\0') {
        return Err(ServiceBootstrapError::InvalidDataDirectory);
    }
    Ok(())
}

fn validate_session_token(token: &str) -> Result<(), ServiceBootstrapError> {
    if !(MINIMUM_SESSION_TOKEN_BYTES..=MAXIMUM_SESSION_TOKEN_BYTES).contains(&token.len()) {
        return Err(ServiceBootstrapError::InvalidSessionToken);
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, ServiceBootstrapError> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let bytes = digest.finalize();
    Ok(format!("{bytes:x}"))
}

#[derive(Debug, Error)]
pub enum ServiceBootstrapError {
    #[error("the app-scoped session token is missing")]
    MissingSessionToken,
    #[error("the app-scoped session token has an invalid length")]
    InvalidSessionToken,
    #[error("the launch agent has no user home directory")]
    MissingHomeDirectory,
    #[error("the launch agent user home directory is invalid")]
    InvalidHomeDirectory,
    #[error("the launch agent has no channel data directory")]
    MissingDataDirectory,
    #[error("the launch agent channel data directory is invalid")]
    InvalidDataDirectory,
    #[error("the launch agent has no product version")]
    MissingProductVersion,
    #[error("the launch agent has no build number")]
    MissingBuildNumber,
    #[error("the launch agent is missing {0}")]
    MissingConfiguration(&'static str),
    #[error("service bootstrap I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("service bootstrap JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::{ServiceIdentity, ServiceRecord};
    use crate::StartupReady;

    #[test]
    fn service_record_keeps_the_existing_swift_discovery_contract() -> Result<(), serde_json::Error>
    {
        let encoded = serde_json::to_value(ServiceRecord {
            endpoint: StartupReady {
                record_type: "engineReady".to_owned(),
                host: "127.0.0.1".to_owned(),
                port: 17_000,
                protocol_version: 1,
            },
            session_token: "a".repeat(64),
            process_id: 42,
            product_version: "0.5.0-dev-2".to_owned(),
            service_identity: ServiceIdentity {
                product_version: "0.5.0-dev-2".to_owned(),
                build_number: "158".to_owned(),
                engine_executable_path: "/Applications/Lumi/lumi-engine".to_owned(),
                engine_executable_sha256: "f".repeat(64),
            },
        })?;

        assert_eq!(encoded["processID"], 42);
        assert!(encoded.get("processId").is_none());
        assert_eq!(
            encoded["serviceIdentity"]["engineExecutableSHA256"],
            "f".repeat(64)
        );
        assert!(
            encoded["serviceIdentity"]
                .get("engineExecutableSha256")
                .is_none()
        );
        Ok(())
    }
}
