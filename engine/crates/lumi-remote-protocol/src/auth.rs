use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum RemoteClientHello {
    Authenticate {
        device_id: String,
        credential: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_version: Option<String>,
    },
    Pair {
        invitation_id: String,
        invitation_secret: String,
        device_id: String,
        display_name: String,
        device_credential: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_version: Option<String>,
    },
}

impl RemoteClientHello {
    pub fn validate(&self) -> Result<(), RemoteAuthenticationError> {
        match self {
            Self::Authenticate {
                device_id,
                credential,
                client_version,
            } => {
                validate_identifier(device_id, 128)?;
                validate_secret(credential)?;
                validate_version(client_version.as_deref())?;
            }
            Self::Pair {
                invitation_id,
                invitation_secret,
                device_id,
                display_name,
                device_credential,
                client_version,
            } => {
                validate_identifier(invitation_id, 128)?;
                validate_secret(invitation_secret)?;
                validate_identifier(device_id, 128)?;
                validate_identifier(display_name, 128)?;
                validate_secret(device_credential)?;
                validate_version(client_version.as_deref())?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum RemoteServerHello {
    Authenticated {
        installation_id: String,
        controller_lease_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        controller_display_name: Option<String>,
    },
    Paired {
        installation_id: String,
        controller_lease_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        controller_display_name: Option<String>,
    },
}

fn validate_version(value: Option<&str>) -> Result<(), RemoteAuthenticationError> {
    if let Some(value) = value {
        validate_identifier(value, 64)?;
    }
    Ok(())
}

fn validate_identifier(value: &str, maximum: usize) -> Result<(), RemoteAuthenticationError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(RemoteAuthenticationError::InvalidIdentifier);
    }
    Ok(())
}

fn validate_secret(value: &str) -> Result<(), RemoteAuthenticationError> {
    if !(32..=512).contains(&value.len()) || value.chars().any(char::is_control) {
        return Err(RemoteAuthenticationError::InvalidSecret);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RemoteAuthenticationError {
    #[error("remote authentication identifier is invalid")]
    InvalidIdentifier,
    #[error("remote authentication secret is invalid")]
    InvalidSecret,
}

#[cfg(test)]
mod tests {
    use super::{RemoteAuthenticationError, RemoteClientHello};

    #[test]
    fn rejects_short_credentials_before_gateway_work() {
        let hello = RemoteClientHello::Authenticate {
            device_id: "iphone-1".to_owned(),
            credential: "short".to_owned(),
            client_version: None,
        };
        assert_eq!(
            hello.validate(),
            Err(RemoteAuthenticationError::InvalidSecret)
        );
    }
}
