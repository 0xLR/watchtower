use crate::state::{stream_id, AppState};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use watchtower_core::*;
use watchtower_netid::{Decision, RecoverySink, RetryState};

struct Sink {
    state: Arc<AppState>,
    app: AppHandle,
}
#[async_trait::async_trait]
impl RecoverySink for Sink {
    async fn commit(
        &self,
        camera: &Camera,
        identity: &CameraIdentity,
        old_host: &str,
    ) -> Result<(), String> {
        let _guard = self.state.operation.lock().await;
        let previous = self
            .state
            .store
            .cameras()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|c| c.id == camera.id)
            .ok_or("Camera was removed")?;
        let mut expected = camera.clone();
        expected.host = old_host.into();
        expected.is_local = watchtower_core::is_private(old_host);
        if previous != expected {
            return Err("Camera was edited during recovery".into());
        }
        let stored_identity = self
            .state
            .store
            .identity(&camera.id)
            .await
            .map_err(|e| e.to_string())?;
        if stored_identity.mac != identity.mac
            || stored_identity.onvif_endpoint_uuid != identity.onvif_endpoint_uuid
            || stored_identity.serial != identity.serial
        {
            return Err("Camera identity changed during recovery".into());
        }
        self.state.register(camera).await?;
        if let Err(error) = self.state.store.save(camera, identity).await {
            let _ = self.state.register(&previous).await;
            return Err(error.to_string());
        }
        let _ = self.app.emit(
            "camera:address-changed",
            AddressChanged {
                camera_id: camera.id.clone(),
                old_host: old_host.into(),
                new_host: camera.host.clone(),
            },
        );
        Ok(())
    }
}
struct CameraNetwork {
    username: String,
    password: Option<String>,
    serial: bool,
    port: u16,
}
#[async_trait::async_trait]
impl NeighborTable for CameraNetwork {
    async fn neighbors(&self) -> Result<Vec<ObservedDevice>, String> {
        watchtower_discovery::LocalNetwork.neighbors().await
    }
}
#[async_trait::async_trait]
impl Discoverer for CameraNetwork {
    async fn discover(&self, interfaces: &[String], sweep: bool) -> Result<ScanResult, String> {
        use futures_util::{stream, StreamExt};
        let mut result = watchtower_discovery::LocalNetwork
            .discover(interfaces, sweep)
            .await?;
        if sweep && !matches!(self.port, 554 | 2020) {
            for interface in watchtower_discovery::interfaces()?
                .into_iter()
                .filter(|i| interfaces.contains(&i.id))
                .take(4)
            {
                let hosts =
                    watchtower_discovery::sweep_hosts(&interface.address, interface.prefix)?;
                let found: Vec<_> = stream::iter(hosts)
                    .map(|host| async move {
                        if watchtower_discovery::probe(&host, self.port).await {
                            Some(host)
                        } else {
                            None
                        }
                    })
                    .buffer_unordered(48)
                    .filter_map(|host| async move { host })
                    .collect()
                    .await;
                let neighbors = self.neighbors().await.unwrap_or_default();
                for host in found {
                    if !result.devices.iter().any(|d| d.host == host) {
                        result.devices.push(
                            neighbors
                                .iter()
                                .find(|d| d.host == host)
                                .cloned()
                                .unwrap_or(ObservedDevice {
                                    host,
                                    port: self.port,
                                    ..Default::default()
                                }),
                        );
                    }
                }
            }
        }
        if self.serial {
            result.devices = stream::iter(result.devices)
                .map(|mut device| async move {
                    for port in [device.onvif_port.unwrap_or(2020), 80, 8080] {
                        if !watchtower_discovery::probe(&device.host, port).await {
                            continue;
                        }
                        if let Ok(info) = watchtower_onvif::device_info(
                            &device.host,
                            port,
                            &self.username,
                            self.password.as_deref(),
                        )
                        .await
                        {
                            device.serial = info.serial;
                            device.model = info.model.or(device.model);
                            break;
                        }
                    }
                    device
                })
                .buffer_unordered(8)
                .collect()
                .await;
        }
        Ok(result)
    }
}
#[async_trait::async_trait]
impl watchtower_netid::RecoveryNetwork for CameraNetwork {
    async fn probe(&self, host: &str, port: u16) {
        watchtower_discovery::probe(host, port).await;
    }
}

pub fn start(state: Arc<AppState>, app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut retry = HashMap::<String, RetryState>::new();
        let mut bytes = HashMap::<String, u64>::new();
        let recovering = Arc::new(tokio::sync::Mutex::new(HashSet::<String>::new()));
        let mut last_sweep = Instant::now();
        loop {
            interval.tick().await;
            state
                .tested
                .lock()
                .await
                .retain(|_, test| test.expires > Instant::now());
            let Ok(cameras) = state.store.cameras().await else {
                continue;
            };
            let Ok(settings) = state.store.settings().await else {
                continue;
            };
            let sweep =
                settings.background_sweep && last_sweep.elapsed() >= Duration::from_secs(180);
            if sweep {
                last_sweep = Instant::now();
            }
            let ids: HashSet<_> = cameras.iter().map(|c| c.id.clone()).collect();
            retry.retain(|id, _| ids.contains(id));
            bytes.retain(|id, _| ids.contains(id));
            for camera in cameras {
                let active = state.active_streams.read().await.get(&camera.id).copied();
                if active.is_none() && !sweep {
                    retry.remove(&camera.id);
                    bytes.remove(&camera.id);
                    continue;
                }
                let policy = retry.entry(camera.id.clone()).or_default();
                let mut healthy = false;
                if let (Some(main), Ok(media)) = (active, state.media()) {
                    if let Ok(Some(current)) =
                        media.received_bytes(&stream_id(&camera.id, main)).await
                    {
                        healthy = current > 0
                            && bytes
                                .get(&camera.id)
                                .is_none_or(|previous| current > *previous);
                        bytes.insert(camera.id.clone(), current);
                    }
                }
                if healthy && !sweep {
                    policy.healthy();
                    continue;
                }
                if !sweep && !policy.failed(Instant::now()) {
                    continue;
                }
                if !policy.due(Instant::now()) || recovering.lock().await.contains(&camera.id) {
                    continue;
                }
                policy.defer(Instant::now());
                state
                    .statuses
                    .write()
                    .await
                    .insert(camera.id.clone(), StreamStatus::Unreachable);
                if !camera.is_local || settings.selected_interfaces.is_empty() {
                    continue;
                }
                let Ok(identity) = state.store.identity(&camera.id).await else {
                    continue;
                };
                if identity.mac.is_none()
                    && identity.onvif_endpoint_uuid.is_none()
                    && identity.serial.is_none()
                {
                    continue;
                }
                let state = state.clone();
                let app = app.clone();
                let recovering = recovering.clone();
                let interfaces = settings.selected_interfaces.clone();
                recovering.lock().await.insert(camera.id.clone());
                tauri::async_runtime::spawn(async move {
                    let _scan = state.scan_lock.lock().await;
                    let sink = Sink {
                        state: state.clone(),
                        app: app.clone(),
                    };
                    let network = CameraNetwork {
                        username: camera.username.clone(),
                        password: state.password(&camera.id).await.ok().flatten(),
                        serial: identity.serial.is_some(),
                        port: camera.port,
                    };
                    let decision = tokio::time::timeout(
                        Duration::from_secs(55),
                        watchtower_netid::recover_camera(
                            &camera,
                            &identity,
                            &network,
                            &sink,
                            &interfaces,
                        ),
                    )
                    .await;
                    if let Ok(Ok(Decision::Match(host))) = decision {
                        if host == camera.host {
                            let _=app.emit("camera:recovery-notice",format!("{} is still at its saved address. Check its stream settings and credentials.",camera.name));
                        }
                    }
                    recovering.lock().await.remove(&camera.id);
                });
            }
        }
    });
}
