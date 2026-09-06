use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostSnapshot {
    pub name: String,
    pub os: String,
    pub architecture: String,
    pub address: Option<String>,
    pub connection: ConnectionState,
    pub tailscale: bool,
    pub latency_ms: Option<u64>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub cpu_percent: Option<f32>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub load_average: Option<f32>,
    pub uptime_seconds: Option<u64>,
}
