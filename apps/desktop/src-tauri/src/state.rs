use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, RwLock};
use watchtower_core::*;
use watchtower_media::Media;
use watchtower_secrets::{Keychain, SecretStore};
use watchtower_store::Store;

pub struct Tested {
    pub previous: Option<Camera>,
    pub camera: Camera,
    pub password: Option<String>,
    pub expires: Instant,
}
pub struct AppState {
    pub active_streams: RwLock<HashMap<String, bool>>,
    pub scan_lock: Mutex<()>,
    pub observations: RwLock<Vec<ObservedDevice>>,
    pub store: Store,
    pub media: Result<Arc<Media>, String>,
    pub secrets: Arc<dyn SecretStore>,
    pub tested: Mutex<HashMap<String, Tested>>,
    pub operation: Mutex<()>,
    pub statuses: RwLock<HashMap<String, StreamStatus>>,
}
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
impl AppState {
    pub async fn new(app: &AppHandle) -> Result<Arc<Self>, String> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| "Application data directory is unavailable")?;
        #[cfg(debug_assertions)]
        let directory = std::env::var_os("WATCHTOWER_DEV_DATA")
            .map(PathBuf::from)
            .unwrap_or(directory);
        std::fs::create_dir_all(&directory)
            .map_err(|_| "Application data directory could not be created")?;
        let store = Store::open(&directory.join("watchtower.sqlite"))
            .await
            .map_err(|e| e.to_string())?;
        let media = Ok(Arc::new(Media::new(
            &sidecar_path(app, "go2rtc"),
            &sidecar_path(app, "mediamtx"),
        )));
        let state = Arc::new(Self {
            store,
            media,
            active_streams: RwLock::new(HashMap::new()),
            scan_lock: Mutex::new(()),
            observations: RwLock::new(Vec::new()),
            secrets: Arc::new(Keychain),
            tested: Mutex::new(HashMap::new()),
            operation: Mutex::new(()),
            statuses: RwLock::new(HashMap::new()),
        });
        for camera in state.store.cameras().await.map_err(|e| e.to_string())? {
            if state.register(&camera).await.is_err() {
                state
                    .statuses
                    .write()
                    .await
                    .insert(camera.id, StreamStatus::Unreachable);
            }
        }
        Ok(state)
    }
    pub fn media(&self) -> Result<&Media, String> {
        self.media.as_ref().map(Arc::as_ref).map_err(Clone::clone)
    }
    pub async fn password(&self, id: &str) -> Result<Option<String>, String> {
        let secrets = self.secrets.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || secrets.get(&id))
            .await
            .map_err(|_| "Credential store task failed".to_string())?
            .map_err(|e| e.to_string())
    }
    pub async fn set_password(&self, id: &str, password: Option<String>) -> Result<(), String> {
        let secrets = self.secrets.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            if let Some(password) = password {
                secrets.set(&id, &password)
            } else {
                secrets.delete(&id)
            }
        })
        .await
        .map_err(|_| "Credential store task failed".to_string())?
        .map_err(|e| e.to_string())
    }
    pub async fn register(&self, camera: &Camera) -> Result<(), String> {
        let password = self.password(&camera.id).await?;
        self.register_with(camera, password.as_deref()).await
    }
    pub async fn register_with(
        &self,
        camera: &Camera,
        password: Option<&str>,
    ) -> Result<(), String> {
        let media = self.media()?;
        let engine = crate::engines::effective(
            camera.media_engine.unwrap_or(
                self.store
                    .settings()
                    .await
                    .map_err(|e| e.to_string())?
                    .media_engine,
            ),
        );
        for main in [true, false] {
            let url = camera
                .source_url(password, main)
                .map_err(|e| e.to_string())?;
            media
                .ensure_stream_on(&stream_id(&camera.id, main), &url, engine)
                .await?;
        }
        Ok(())
    }
    pub async fn snapshot(&self) -> Result<AppSnapshot, String> {
        let settings = self.store.settings().await.map_err(|e| e.to_string())?;
        let mut views = Vec::new();
        let statuses = self.statuses.read().await;
        for camera in self.store.cameras().await.map_err(|e| e.to_string())? {
            let identity = self
                .store
                .identity(&camera.id)
                .await
                .map_err(|e| e.to_string())?;
            let tracking = !settings.selected_interfaces.is_empty()
                && camera.is_local
                && (identity.mac.is_some()
                    || identity.onvif_endpoint_uuid.is_some()
                    || identity.serial.is_some());
            let status = statuses
                .get(&camera.id)
                .cloned()
                .unwrap_or(StreamStatus::Idle);
            views.push(CameraView {
                camera,
                identity,
                tracking,
                status,
            });
        }
        let warning = self.media.as_ref().err().cloned().or_else(|| {
                let unavailable = crate::engines::effective(settings.media_engine) != settings.media_engine
                    || views.iter().any(|view| view.camera.media_engine.is_some_and(|engine| crate::engines::effective(engine) != engine));
                unavailable.then(|| "A saved media engine is not included in this build. Using Retina; saved engine preferences are preserved. Retina supports H.264 video only.".into())
            });
        Ok(AppSnapshot {
            available_engines: crate::engines::available(),
            cameras: views,
            settings,
            media_ready: self.media.is_ok(),
            warning,
        })
    }
    pub async fn playback(&self, id: &str, main: bool) -> Result<Playback, String> {
        let _guard = self.operation.lock().await;
        let camera = self
            .store
            .cameras()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or("Camera not found")?;
        let password = self.password(id).await?;
        let source = camera
            .source_url(password.as_deref(), main)
            .map_err(|e| e.to_string())?;
        let stream = stream_id(id, main);
        let media = self.media()?;
        let engine = crate::engines::effective(
            camera.media_engine.unwrap_or(
                self.store
                    .settings()
                    .await
                    .map_err(|e| e.to_string())?
                    .media_engine,
            ),
        );
        media.ensure_stream_on(&stream, &source, engine).await?;
        Ok(media.playback(&stream))
    }
    pub async fn test_camera(&self, input: CameraInput) -> Result<String, String> {
        let _guard = self.operation.lock().await;
        let mut parsed = parse_address(&input).map_err(|e| e.to_string())?;
        if parsed.host.parse::<std::net::IpAddr>().is_err() {
            if let Ok(Ok(addresses)) = tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::lookup_host((parsed.host.clone(), parsed.port)),
            )
            .await
            {
                let addresses: Vec<_> = addresses.map(|a| a.ip()).collect();
                if !addresses.is_empty() && addresses.iter().all(|ip| is_private(&ip.to_string())) {
                    if let Some(ip) = addresses
                        .iter()
                        .find(|ip| ip.is_ipv4())
                        .or(addresses.first())
                    {
                        parsed.host = ip.to_string();
                        parsed.is_local = true;
                    }
                }
            }
        }
        let existing = if let Some(id) = &input.id {
            Some(
                self.store
                    .cameras()
                    .await
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .find(|c| &c.id == id)
                    .ok_or("Camera no longer exists")?,
            )
        } else {
            None
        };
        let id = existing
            .as_ref()
            .map(|c| c.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let password = if parsed.password.is_some() {
            parsed.password.clone()
        } else if existing.is_some() {
            self.password(&id).await?
        } else {
            None
        };
        let count = self.store.cameras().await.map_err(|e| e.to_string())?.len() as i32;
        let camera = parsed.into_camera(
            id,
            input.name,
            existing.as_ref().map(|c| c.created_at).unwrap_or_else(now),
            existing.as_ref().map(|c| c.sort_order).unwrap_or(count),
        );
        let media = self.media()?;
        let test_id = format!("test_{}", uuid::Uuid::new_v4().simple());
        for main in [true, false] {
            let url = camera
                .source_url(password.as_deref(), main)
                .map_err(|e| e.to_string())?;
            let engine = crate::engines::effective(
                camera.media_engine.unwrap_or(
                    self.store
                        .settings()
                        .await
                        .map_err(|e| e.to_string())?
                        .media_engine,
                ),
            );
            media.ensure_stream_on(&test_id, &url, engine).await?;
            let result = media.test_stream(&test_id).await.map_err(|e| e.to_string());
            let _ = media.remove_stream(&test_id).await;
            result.map_err(|error| if main {
                format!("Main stream: {error}")
            } else {
                format!("Main stream connected; substream failed: {error} Clear the optional substream under Stream paths to use the main stream only.")
            })?;
            if camera.path_sub.is_none() {
                break;
            }
        }
        let token = uuid::Uuid::new_v4().to_string();
        let mut tested = self.tested.lock().await;
        tested.retain(|_, test| test.expires > Instant::now());
        if tested.len() >= 32 {
            tested.clear();
        }
        tested.insert(
            token.clone(),
            Tested {
                previous: existing.clone(),
                camera,
                password,
                expires: Instant::now() + Duration::from_secs(120),
            },
        );
        Ok(token)
    }
    pub async fn save_camera(&self, token: &str) -> Result<String, String> {
        let _guard = self.operation.lock().await;
        let tested = self
            .tested
            .lock()
            .await
            .remove(token)
            .filter(|test| test.expires > Instant::now())
            .ok_or("Connection test expired. Test again before saving.")?;
        let current = self
            .store
            .cameras()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|c| c.id == tested.camera.id);
        if current != tested.previous {
            return Err(
                "Camera changed during its connection test. Test again before saving.".into(),
            );
        }
        let old = self.password(&tested.camera.id).await?;
        if tested.password.is_some() {
            self.set_password(&tested.camera.id, tested.password.clone())
                .await?;
        }
        let identity = crate::commands::discovery::capture_identity(
            self,
            &tested.camera,
            tested.password.as_deref(),
        )
        .await;
        if let Err(error) = self.store.save(&tested.camera, &identity).await {
            let _ = self.set_password(&tested.camera.id, old).await;
            return Err(error.to_string());
        }
        if self
            .register_with(&tested.camera, tested.password.as_deref())
            .await
            .is_err()
        {
            // Persistence succeeded: report the saved camera, then let playback reconcile
            // its source after the engine returns. Returning a save error here invites duplicates.
            self.statuses
                .write()
                .await
                .insert(tested.camera.id.clone(), StreamStatus::Unreachable);
        }
        Ok(tested.camera.id)
    }
}
pub fn stream_id(id: &str, main: bool) -> String {
    format!("{}_{}", id, if main { "main" } else { "sub" })
}
fn sidecar_path(app: &AppHandle, name: &str) -> PathBuf {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join(format!("{name}{suffix}"));
            if sibling.exists() {
                return sibling;
            }
        }
    }
    if let Ok(resources) = app.path().resource_dir() {
        let bundled = resources.join(format!("{name}{suffix}"));
        if bundled.exists() {
            return bundled;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(format!("{name}-{}{suffix}", env!("WATCHTOWER_TARGET")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use watchtower_secrets::{MemorySecrets, SecretError};
    async fn test_state(secrets: Arc<dyn SecretStore>) -> AppState {
        let binary =
            PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR").expect("test sidecar path"));
        AppState {
            store: Store::open(std::path::Path::new(":memory:")).await.unwrap(),
            media: Ok(Arc::new(Media::launch(&binary).await.unwrap())),
            secrets,
            tested: Mutex::new(HashMap::new()),
            operation: Mutex::new(()),
            statuses: RwLock::new(HashMap::new()),
            scan_lock: Mutex::new(()),
            observations: RwLock::new(Vec::new()),
            active_streams: RwLock::new(HashMap::new()),
        }
    }
    fn input() -> CameraInput {
        CameraInput {
            media_engine: Some(MediaEngine::Go2rtc),
            id: None,
            name: "Synthetic camera".into(),
            address: "rtsp://127.0.0.1:8554/watchtower-test".into(),
            username: "viewer".into(),
            password: Some("unique-test-p@ssword".into()),
            path_main: None,
            path_sub: None,
            source_kind: None,
        }
    }
    #[tokio::test]
    #[ignore = "requires local synthetic RTSP server and fetched sidecar"]
    async fn connection_receipt_save_and_stale_edit_guard() {
        let secrets = Arc::new(MemorySecrets::default());
        let state = test_state(secrets.clone()).await;
        assert!(state.save_camera("no-test").await.is_err());
        let token = state.test_camera(input()).await.unwrap();
        let id = state.save_camera(&token).await.unwrap();
        assert_eq!(
            secrets.get(&id).unwrap().as_deref(),
            Some("unique-test-p@ssword")
        );
        assert!(state.save_camera(&token).await.is_err());
        let snapshot = state.snapshot().await.unwrap();
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains("unique-test"));
        assert!(!json.contains("rtsp://"));
        assert_eq!(snapshot.cameras.len(), 1);
        let mut edit = input();
        edit.id = Some(id.clone());
        let token = state.test_camera(edit).await.unwrap();
        state.store.delete(&id).await.unwrap();
        assert!(state.save_camera(&token).await.is_err());
        assert!(state.store.cameras().await.unwrap().is_empty());
        state.media().unwrap().shutdown().await;
    }
    #[tokio::test]
    #[ignore = "requires local synthetic RTSP server and fetched sidecar"]
    async fn saving_during_media_crash_keeps_one_camera_and_recovers_playback() {
        let state = test_state(Arc::new(MemorySecrets::default())).await;
        let token = state.test_camera(input()).await.unwrap();
        state.media().unwrap().simulate_crash().await;
        let id = state
            .save_camera(&token)
            .await
            .expect("saved configuration must not be reported as an unsaved camera");
        assert_eq!(state.store.cameras().await.unwrap().len(), 1);
        tokio::time::sleep(Duration::from_secs(4)).await;
        state
            .playback(&id, true)
            .await
            .expect("re-register saved source after crash");
        state
            .media()
            .unwrap()
            .test_stream(&stream_id(&id, true))
            .await
            .unwrap();
        state.media().unwrap().shutdown().await;
    }
    struct LockedSecrets;
    #[tokio::test]
    #[ignore = "requires local synthetic RTSP server and fetched sidecar"]
    async fn optional_substream_failure_identifies_working_main_and_can_be_cleared() {
        let state = test_state(Arc::new(MemorySecrets::default())).await;
        let mut value = input();
        value.path_sub = Some("/missing-substream".into());
        let error = state.test_camera(value.clone()).await.unwrap_err();
        assert!(error.contains("Main stream connected; substream failed"));
        assert!(error.contains("stream path"));
        assert!(!error.contains("unique-test"));
        value.path_sub = Some(String::new());
        assert!(state.test_camera(value).await.is_ok());
        state.media().unwrap().shutdown().await;
    }
    impl SecretStore for LockedSecrets {
        fn get(&self, _: &str) -> Result<Option<String>, SecretError> {
            Ok(None)
        }
        fn set(&self, _: &str, _: &str) -> Result<(), SecretError> {
            Err(SecretError)
        }
        fn delete(&self, _: &str) -> Result<(), SecretError> {
            Err(SecretError)
        }
    }
    #[tokio::test]
    #[ignore = "requires local synthetic RTSP server and fetched sidecar"]
    async fn locked_keychain_never_creates_a_camera() {
        let state = test_state(Arc::new(LockedSecrets)).await;
        let token = state.test_camera(input()).await.unwrap();
        assert!(state.save_camera(&token).await.is_err());
        assert!(state.store.cameras().await.unwrap().is_empty());
        state.media().unwrap().shutdown().await;
    }
}
