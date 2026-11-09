//! Pure domain types. Network, database and operating-system access belong elsewhere.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/AppInfo.ts")]
pub struct AppInfo {
    pub version: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../../packages/ipc-types/StreamStatus.ts")]
#[serde(rename_all = "snake_case")]
pub enum StreamStatus {
    Idle,
    Connecting,
    Live,
    Unreachable,
}

#[derive(Clone, Serialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/Playback.ts")]
pub struct Playback {
    pub websocket_url: String,
    pub hls_url: String,
}

/// Implementations sanitize errors and never expose source URLs.
#[async_trait::async_trait]
pub trait StreamBackend: Send + Sync {
    async fn add_stream(&self, id: &str, source: &str) -> Result<(), String>;
    async fn update_stream(&self, id: &str, source: &str) -> Result<(), String>;
    async fn remove_stream(&self, id: &str) -> Result<(), String>;
    async fn stream_status(&self, id: &str) -> Result<StreamStatus, String>;
}
mod camera;
pub use camera::*;
