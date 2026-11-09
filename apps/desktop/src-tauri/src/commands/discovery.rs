use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use watchtower_core::{Discoverer, NetworkInterface, ObservedDevice, ScanResult};
#[tauri::command]
pub async fn network_interfaces() -> Result<Vec<NetworkInterface>, String> {
    tokio::task::spawn_blocking(watchtower_discovery::interfaces)
        .await
        .map_err(|_| "Interface enumeration failed".to_string())?
}
#[tauri::command]
pub async fn scan_cameras(
    state: State<'_, Arc<AppState>>,
    interfaces: Vec<String>,
    sweep: bool,
) -> Result<ScanResult, String> {
    let _scan = state
        .scan_lock
        .try_lock()
        .map_err(|_| "A network scan is already running")?;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(55),
        watchtower_discovery::LocalNetwork.discover(&interfaces, sweep),
    )
    .await
    .map_err(|_| "Scan timed out. Try selecting fewer interfaces.")??;
    *state.observations.write().await = result.devices.clone();
    Ok(result)
}
pub async fn capture_identity(
    state: &AppState,
    camera: &watchtower_core::Camera,
    password: Option<&str>,
) -> watchtower_core::CameraIdentity {
    use watchtower_core::NeighborTable;
    let previous = state
        .store
        .cameras()
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|c| c.id == camera.id);
    let mut identity = if previous.is_some_and(|old| old.host == camera.host) {
        state.store.identity(&camera.id).await.unwrap_or_default()
    } else {
        watchtower_core::CameraIdentity::default()
    };
    identity.camera_id = camera.id.clone();
    if !camera.is_local {
        return identity;
    }
    let _ = watchtower_discovery::probe(&camera.host, camera.port).await;
    let devices = watchtower_discovery::LocalNetwork
        .neighbors()
        .await
        .unwrap_or_default();
    if let Some(device) = devices.iter().find(|d| d.host == camera.host) {
        if device.mac.as_ref().is_some_and(|mac| {
            devices
                .iter()
                .filter(|d| d.mac.as_ref() == Some(mac))
                .count()
                == 1
        }) {
            identity.mac = device.mac.clone();
        }
    }
    let mut observed = state.observations.read().await.clone();
    let selected = state
        .store
        .settings()
        .await
        .map(|s| s.selected_interfaces)
        .unwrap_or_default();
    if !selected.is_empty() {
        if let Ok(Ok(scan)) = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            watchtower_discovery::LocalNetwork.discover(&selected, false),
        )
        .await
        {
            observed.extend(scan.devices);
        }
    }
    if let Some(device) = observed
        .iter()
        .find(|d: &&ObservedDevice| d.host == camera.host)
    {
        identity.onvif_endpoint_uuid = device
            .endpoint_uuid
            .clone()
            .or(identity.onvif_endpoint_uuid);
        identity.serial = device.serial.clone().or(identity.serial);
        identity.model = device.model.clone().or(identity.model);
    }
    let discovered_port = observed
        .iter()
        .find(|d| d.host == camera.host)
        .and_then(|d| d.onvif_port);
    let mut ports = vec![discovered_port.unwrap_or(2020), 80, 8080];
    ports.dedup();
    for port in ports {
        if let Ok(info) =
            watchtower_onvif::device_info(&camera.host, port, &camera.username, password).await
        {
            identity.serial = info.serial.or(identity.serial);
            identity.model = info.model.or(identity.model);
            break;
        }
    }
    identity.last_seen_at = Some(crate::state::now());
    identity.last_seen_host = Some(camera.host.clone());
    identity
}
