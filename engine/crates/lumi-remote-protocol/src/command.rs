use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationTarget {
    Off,
    Armed,
    Live,
    Paused,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum RemoteCommandKind {
    SetOperationState {
        operation_state: OperationTarget,
        expected_state_revision: u64,
    },
    SetAbletonLinkEnabled {
        enabled: bool,
        expected_state_revision: u64,
    },
    SetOutputTimingOffset {
        millis: i16,
        expected_state_revision: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_timing_offset_millis: Option<i16>,
    },
    SelectThemeFromPhrase {
        plan_id: String,
        track_load_id: u64,
        expected_plan_revision: u64,
        phrase_index: u16,
        theme_id: u64,
    },
    SelectAutoloopForPhrase {
        plan_id: String,
        track_load_id: u64,
        expected_plan_revision: u64,
        phrase_index: u16,
        autoloop_number: u8,
    },
    ChangePhraseRole {
        plan_id: String,
        track_load_id: u64,
        expected_plan_revision: u64,
        phrase_index: u16,
        role_id: String,
    },
    SetCueLock {
        plan_id: String,
        track_load_id: u64,
        expected_plan_revision: u64,
        phrase_index: u16,
        locked: bool,
    },
    RequestSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCommand {
    pub command_id: String,
    pub controller_lease_id: String,
    pub issued_at_unix_millis: u64,
    pub command: RemoteCommandKind,
}

impl RemoteCommand {
    pub fn validate(&self) -> Result<(), RemoteCommandError> {
        validate_identifier("commandId", &self.command_id, 128)?;
        validate_identifier("controllerLeaseId", &self.controller_lease_id, 128)?;
        match &self.command {
            RemoteCommandKind::SetOutputTimingOffset {
                millis,
                expected_timing_offset_millis,
                ..
            } if !(-250..=250).contains(millis)
                || expected_timing_offset_millis
                    .is_some_and(|value| !(-250..=250).contains(&value)) =>
            {
                Err(RemoteCommandError::TimingOffsetOutOfRange)
            }
            RemoteCommandKind::SelectAutoloopForPhrase {
                plan_id,
                autoloop_number,
                ..
            } => {
                validate_identifier("planId", plan_id, 128)?;
                if !(1..=32).contains(autoloop_number) {
                    return Err(RemoteCommandError::AutoloopOutOfRange);
                }
                Ok(())
            }
            RemoteCommandKind::SelectThemeFromPhrase { plan_id, .. }
            | RemoteCommandKind::SetCueLock { plan_id, .. } => {
                validate_identifier("planId", plan_id, 128)
            }
            RemoteCommandKind::ChangePhraseRole {
                plan_id, role_id, ..
            } => {
                validate_identifier("planId", plan_id, 128)?;
                validate_identifier("roleId", role_id, 128)
            }
            _ => Ok(()),
        }
    }

    pub const fn is_mutating(&self) -> bool {
        !matches!(self.command, RemoteCommandKind::RequestSnapshot)
    }
}

fn validate_identifier(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), RemoteCommandError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(RemoteCommandError::InvalidIdentifier(field));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RemoteCommandError {
    #[error("{0} is empty, oversized, or contains control characters")]
    InvalidIdentifier(&'static str),
    #[error("timing offset must be between -250 and 250 milliseconds")]
    TimingOffsetOutOfRange,
    #[error("AutoLoop number must be between 1 and 32")]
    AutoloopOutOfRange,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteCommandResultStatus {
    Accepted,
    Duplicate,
    Conflict,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCommandResult {
    pub command_id: String,
    pub status: RemoteCommandResultStatus,
    pub state_revision: Option<u64>,
    pub plan_revision: Option<u64>,
    pub reason_code: Option<String>,
}

impl RemoteCommandResult {
    pub fn validate(&self) -> Result<(), RemoteCommandError> {
        validate_identifier("commandId", &self.command_id, 128)?;
        if let Some(reason) = &self.reason_code {
            validate_identifier("reasonCode", reason, 64)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RemoteCommand, RemoteCommandError, RemoteCommandKind, RemoteCommandResult,
        RemoteCommandResultStatus,
    };

    #[test]
    fn rejects_output_offset_outside_the_booth_safe_range() {
        let command = RemoteCommand {
            command_id: "offset-1".to_owned(),
            controller_lease_id: "lease-1".to_owned(),
            issued_at_unix_millis: 1,
            command: RemoteCommandKind::SetOutputTimingOffset {
                millis: 251,
                expected_state_revision: 7,
                expected_timing_offset_millis: None,
            },
        };

        assert_eq!(
            command.validate(),
            Err(RemoteCommandError::TimingOffsetOutOfRange)
        );
    }

    #[test]
    fn accepts_a_revision_bound_future_phrase_choice() {
        let command = RemoteCommand {
            command_id: "choice-1".to_owned(),
            controller_lease_id: "lease-1".to_owned(),
            issued_at_unix_millis: 1,
            command: RemoteCommandKind::SelectAutoloopForPhrase {
                plan_id: "plan-9".to_owned(),
                track_load_id: 88,
                expected_plan_revision: 4,
                phrase_index: 5,
                autoloop_number: 32,
            },
        };

        assert_eq!(command.validate(), Ok(()));
        assert!(command.is_mutating());
    }

    #[test]
    fn command_result_has_a_bounded_machine_readable_reason() {
        let result = RemoteCommandResult {
            command_id: "choice-1".to_owned(),
            status: RemoteCommandResultStatus::Conflict,
            state_revision: Some(9),
            plan_revision: Some(4),
            reason_code: Some("planRevisionConflict".to_owned()),
        };
        assert_eq!(result.validate(), Ok(()));
    }
}
