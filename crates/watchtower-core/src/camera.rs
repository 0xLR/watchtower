use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../../packages/ipc-types/Camera.ts")]
pub struct Camera {
    #[serde(default)]
    pub media_engine: Option<crate::MediaEngine>,
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub sort_order: i32,
    pub host: String,
    pub port: u16,
    pub path_main: String,
    pub path_sub: Option<String>,
    pub username: String,
    pub source_kind: String,
    pub is_local: bool,
}
// Deliberately no Debug: this value can contain credentials.
#[derive(Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/CameraInput.ts")]
pub struct CameraInput {
    #[serde(default)]
    pub media_engine: Option<crate::MediaEngine>,
    pub id: Option<String>,
    pub name: String,
    pub address: String,
    pub username: String,
    pub password: Option<String>,
    pub path_main: Option<String>,
    pub path_sub: Option<String>,
    pub source_kind: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../../packages/ipc-types/CameraIdentity.ts")]
pub struct CameraIdentity {
    pub camera_id: String,
    pub mac: Option<String>,
    pub onvif_endpoint_uuid: Option<String>,
    pub serial: Option<String>,
    pub model: Option<String>,
    #[ts(type = "number | null")]
    pub last_seen_at: Option<i64>,
    pub last_seen_host: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/CameraView.ts")]
pub struct CameraView {
    pub camera: Camera,
    pub identity: CameraIdentity,
    pub tracking: bool,
    pub status: crate::StreamStatus,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/Settings.ts")]
pub struct Settings {
    #[serde(default)]
    pub media_engine: crate::MediaEngine,
    pub layout: String,
    pub default_substream: bool,
    pub background_sweep: bool,
    pub selected_interfaces: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            media_engine: crate::MediaEngine::Retina,
            layout: "auto".into(),
            default_substream: true,
            background_sweep: false,
            selected_interfaces: Vec::new(),
        }
    }
}
#[derive(Clone, Serialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/AppSnapshot.ts")]
pub struct AppSnapshot {
    pub cameras: Vec<CameraView>,
    pub settings: Settings,
    pub media_ready: bool,
    pub warning: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/AddressChanged.ts")]
pub struct AddressChanged {
    pub camera_id: String,
    pub old_host: String,
    pub new_host: String,
}

pub struct ParsedAddress {
    pub media_engine: Option<crate::MediaEngine>,
    pub host: String,
    pub port: u16,
    pub path_main: String,
    pub path_sub: Option<String>,
    pub username: String,
    pub password: Option<String>,
    pub source_kind: String,
    pub is_local: bool,
}
#[derive(Debug, thiserror::Error)]
#[error("Enter a camera IP, hostname or RTSP URL. Use separate credential fields; query parameters may only be channel, subtype, unicast or proto.")]
pub struct AddressError;

pub fn is_private(host: &str) -> bool {
    match host.trim_matches(['[', ']']).parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_private() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => ip.is_unique_local() || ip.is_unicast_link_local(),
        _ => false,
    }
}
fn safe_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    path.starts_with('/')
        && !path.chars().any(char::is_control)
        && !path.contains(['@', '#'])
        && !["password=", "passwd=", "token=", "secret="]
            .iter()
            .any(|part| lower.contains(part))
        && path.split_once('?').is_none_or(|(_, query)| {
            url::form_urlencoded::parse(query.as_bytes())
                .all(|(key, _)| matches!(key.as_ref(), "channel" | "subtype" | "unicast" | "proto"))
        })
}
pub fn parse_address(input: &CameraInput) -> Result<ParsedAddress, AddressError> {
    let address = input.address.trim();
    if address.is_empty()
        || address.chars().any(char::is_control)
        || input.name.trim().is_empty()
        || input.name.len() > 120
        || input.name.contains("://")
    {
        return Err(AddressError);
    }
    let full = address.contains("://");
    let url = url::Url::parse(&if full {
        address.into()
    } else {
        format!("rtsp://{address}")
    })
    .map_err(|_| AddressError)?;
    if !matches!(url.scheme(), "rtsp" | "rtsps")
        || url.fragment().is_some()
        || url.port() == Some(0)
    {
        return Err(AddressError);
    }
    // RTSPS is supported upstream but schema retains a scheme field via source_kind.
    if url.scheme() != "rtsp" {
        return Err(AddressError);
    }
    let host = url
        .host_str()
        .ok_or(AddressError)?
        .trim_matches(['[', ']'])
        .to_string();
    let path = if url.path().is_empty() || url.path() == "/" {
        "/stream1".into()
    } else {
        format!(
            "{}{}",
            url.path(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default()
        )
    };
    let path_main = input
        .path_main
        .as_ref()
        .filter(|p| !p.is_empty())
        .cloned()
        .unwrap_or(path);
    let path_sub = if let Some(path) = &input.path_sub {
        if path.is_empty() {
            None
        } else {
            Some(path.clone())
        }
    } else if !full {
        Some("/stream2".into())
    } else {
        None
    };
    if !safe_path(&path_main)
        || path_sub.as_ref().is_some_and(|p| !safe_path(p))
        || url.query().is_some_and(|q| !safe_path(&format!("/x?{q}")))
    {
        return Err(AddressError);
    }
    let username = if !input.username.is_empty() {
        input.username.clone()
    } else {
        percent_encoding::percent_decode_str(url.username())
            .decode_utf8()
            .map_err(|_| AddressError)?
            .into_owned()
    };
    let password = input.password.clone().or_else(|| {
        url.password().and_then(|p| {
            percent_encoding::percent_decode_str(p)
                .decode_utf8()
                .ok()
                .map(|p| p.into_owned())
        })
    });
    Ok(ParsedAddress {
        media_engine: input.media_engine,
        is_local: is_private(&host),
        host,
        port: url.port().unwrap_or(554),
        path_main,
        path_sub,
        username,
        password,
        source_kind: if input.source_kind.as_deref() == Some("onvif_discovered") {
            "onvif_discovered".into()
        } else if full {
            "manual_url".into()
        } else {
            "rtsp".into()
        },
    })
}
impl ParsedAddress {
    pub fn into_camera(self, id: String, name: String, created_at: i64, sort_order: i32) -> Camera {
        Camera {
            media_engine: self.media_engine,
            id,
            name: name.trim().into(),
            created_at,
            sort_order,
            host: self.host,
            port: self.port,
            path_main: self.path_main,
            path_sub: self.path_sub,
            username: self.username,
            source_kind: self.source_kind,
            is_local: self.is_local,
        }
    }
}
impl Camera {
    pub fn source_url(&self, password: Option<&str>, main: bool) -> Result<String, AddressError> {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let path = if main {
            &self.path_main
        } else {
            self.path_sub.as_ref().unwrap_or(&self.path_main)
        };
        let mut url = url::Url::parse(&format!("rtsp://{host}:{}{path}", self.port))
            .map_err(|_| AddressError)?;
        // URL's userinfo setters preserve existing percent escapes. These fields
        // are decoded credentials, so literal percent signs must be escaped first.
        url.set_username(&self.username.replace('%', "%25"))
            .map_err(|_| AddressError)?;
        if !self.username.is_empty() {
            let encoded_password = password.map(|value| value.replace('%', "%25"));
            url.set_password(encoded_password.as_deref())
                .map_err(|_| AddressError)?;
        }
        Ok(url.into())
    }
}
/// Discard complete URLs, including path and query. No diagnostic needs them.
pub fn redact(message: &str) -> String {
    message
        .split_whitespace()
        .map(|part| {
            if part.contains("://") {
                "[redacted-url]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/NetworkInterface.ts")]
pub struct NetworkInterface {
    pub id: String,
    pub name: String,
    pub address: String,
    pub prefix: u8,
    pub virtual_hint: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../../packages/ipc-types/ObservedDevice.ts")]
pub struct ObservedDevice {
    pub host: String,
    pub port: u16,
    pub mac: Option<String>,
    pub endpoint_uuid: Option<String>,
    pub serial: Option<String>,
    pub model: Option<String>,
    pub name: Option<String>,
    pub onvif_port: Option<u16>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/ipc-types/ScanResult.ts")]
pub struct ScanResult {
    pub devices: Vec<ObservedDevice>,
    pub warnings: Vec<String>,
}
#[async_trait::async_trait]
pub trait NeighborTable: Send + Sync {
    async fn neighbors(&self) -> Result<Vec<ObservedDevice>, String>;
}
#[async_trait::async_trait]
pub trait Discoverer: Send + Sync {
    async fn discover(&self, interfaces: &[String], sweep: bool) -> Result<ScanResult, String>;
}
