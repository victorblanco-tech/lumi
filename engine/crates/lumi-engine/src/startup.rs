use serde::{Deserialize, Serialize};

/// One-time stdout record used only to discover the local engine endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupReady {
    pub record_type: String,
    pub host: String,
    pub port: u16,
    pub protocol_version: u16,
}
