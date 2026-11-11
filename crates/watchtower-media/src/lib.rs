//! Engine-independent media facade. Source credentials remain in memory.
mod gateway;
mod go2rtc;
mod mediamtx;
mod native;
mod probe_error;
mod process;
pub use go2rtc::MediaError;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use tokio::sync::Mutex;
use watchtower_core::{MediaEngine, Playback, StreamBackend, StreamStatus};

#[async_trait::async_trait]
pub(crate) trait Backend: StreamBackend {
    fn playback(&self, id: &str) -> Playback;
    async fn test_stream(&self, id: &str) -> Result<(), MediaError>;
    async fn received_bytes(&self, id: &str) -> Result<Option<u64>, String>;
    async fn shutdown(&self);
    async fn simulate_crash(&self) {}
}
#[derive(Clone)]
struct Assignment {
    engine: MediaEngine,
    physical: String,
    source: String,
    playback: Playback,
}
pub struct Media {
    adapters: Mutex<HashMap<MediaEngine, Arc<dyn Backend>>>,
    sources: RwLock<HashMap<String, Assignment>>,
    operations: Mutex<()>,
    go2rtc: PathBuf,
    mediamtx: PathBuf,
}
impl Media {
    /// Compatibility constructor for go2rtc integration fixtures.
    pub async fn launch(binary: &Path) -> Result<Self, MediaError> {
        let media = Self::new(
            binary,
            &binary.with_file_name(if cfg!(windows) {
                "mediamtx.exe"
            } else {
                "mediamtx"
            }),
        );
        media.adapter(MediaEngine::Go2rtc).await?;
        Ok(media)
    }
    /// Engines launch on demand. Native Retina requires no executable.
    pub fn new(go2rtc: &Path, mediamtx: &Path) -> Self {
        Self {
            adapters: Mutex::new(HashMap::new()),
            sources: RwLock::new(HashMap::new()),
            operations: Mutex::new(()),
            go2rtc: go2rtc.into(),
            mediamtx: mediamtx.into(),
        }
    }
    async fn adapter(&self, engine: MediaEngine) -> Result<Arc<dyn Backend>, MediaError> {
        let mut adapters = self.adapters.lock().await;
        if let Some(adapter) = adapters.get(&engine) {
            return Ok(adapter.clone());
        }
        let adapter: Arc<dyn Backend> = match engine {
            MediaEngine::Go2rtc => Arc::new(go2rtc::Media::launch(&self.go2rtc).await?),
            MediaEngine::Retina => Arc::new(native::Native::launch().await?),
            MediaEngine::MediaMtx => Arc::new(mediamtx::MediaMtx::launch(&self.mediamtx).await?),
        };
        adapters.insert(engine, adapter.clone());
        Ok(adapter)
    }
    fn assigned(&self, id: &str) -> Option<MediaEngine> {
        self.sources
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .map(|a| a.engine)
    }
    fn resolved(&self, id: &str) -> Option<(MediaEngine, String)> {
        self.sources
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .map(|a| (a.engine, a.physical.clone()))
    }
    fn in_use(&self, engine: MediaEngine, physical: &str) -> bool {
        self.sources
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .any(|a| a.engine == engine && a.physical == physical)
    }
    pub async fn ensure_stream_on(
        &self,
        id: &str,
        source: &str,
        engine: MediaEngine,
    ) -> Result<(), String> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("Invalid stream identifier".into());
        }
        let _guard = self.operations.lock().await;
        let (old, shared) = {
            let sources = self.sources.read().unwrap_or_else(|e| e.into_inner());
            if sources
                .get(id)
                .is_some_and(|a| a.engine == engine && a.source == source)
            {
                return Ok(());
            }
            (
                sources.get(id).cloned(),
                sources
                    .values()
                    .find(|a| a.engine == engine && a.source == source)
                    .cloned(),
            )
        };
        let adapter = self.adapter(engine).await.map_err(|e| e.to_string())?;
        let physical = if let Some(shared) = shared {
            shared.physical
        } else {
            let physical = format!("media_{}", uuid::Uuid::new_v4().simple());
            adapter.update_stream(&physical, source).await?;
            physical
        };
        self.sources
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id.into(),
                Assignment {
                    engine,
                    playback: adapter.playback(&physical),
                    physical,
                    source: source.into(),
                },
            );
        if let Some(old) = old {
            if !self.in_use(old.engine, &old.physical) {
                if let Ok(previous) = self.adapter(old.engine).await {
                    let _ = previous.remove_stream(&old.physical).await;
                }
            }
        }
        Ok(())
    }
    pub async fn ensure_stream(&self, id: &str, source: &str) -> Result<(), String> {
        self.ensure_stream_on(id, source, self.assigned(id).unwrap_or_default())
            .await
    }
    pub fn playback(&self, id: &str) -> Playback {
        self.sources
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .map(|a| a.playback.clone())
            .unwrap_or(Playback {
                engine: MediaEngine::Go2rtc,
                transports: vec![],
                websocket_url: String::new(),
                hls_url: String::new(),
                whep_url: None,
            })
    }
    pub async fn test_stream(&self, id: &str) -> Result<(), MediaError> {
        let (engine, physical) = self.resolved(id).ok_or(MediaError::Connection)?;
        self.adapter(engine).await?.test_stream(&physical).await
    }
    pub async fn received_bytes(&self, id: &str) -> Result<Option<u64>, String> {
        let Some((engine, physical)) = self.resolved(id) else {
            return Ok(None);
        };
        self.adapter(engine)
            .await
            .map_err(|e| e.to_string())?
            .received_bytes(&physical)
            .await
    }
    pub async fn shutdown(&self) {
        for adapter in self.adapters.lock().await.values() {
            adapter.shutdown().await;
        }
    }
    pub async fn simulate_crash(&self) {
        for adapter in self.adapters.lock().await.values() {
            adapter.simulate_crash().await;
        }
    }
}
#[async_trait::async_trait]
impl StreamBackend for Media {
    async fn add_stream(&self, id: &str, source: &str) -> Result<(), String> {
        self.ensure_stream(id, source).await
    }
    async fn update_stream(&self, id: &str, source: &str) -> Result<(), String> {
        self.ensure_stream(id, source).await
    }
    async fn remove_stream(&self, id: &str) -> Result<(), String> {
        let _guard = self.operations.lock().await;
        let old = self
            .sources
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        if let Some(old) = old.filter(|old| !self.in_use(old.engine, &old.physical)) {
            self.adapter(old.engine)
                .await
                .map_err(|e| e.to_string())?
                .remove_stream(&old.physical)
                .await?;
        }
        Ok(())
    }
    async fn stream_status(&self, id: &str) -> Result<StreamStatus, String> {
        let Some((engine, physical)) = self.resolved(id) else {
            return Ok(StreamStatus::Idle);
        };
        self.adapter(engine)
            .await
            .map_err(|e| e.to_string())?
            .stream_status(&physical)
            .await
    }
}
#[async_trait::async_trait]
impl Backend for go2rtc::Media {
    fn playback(&self, id: &str) -> Playback {
        self.playback(id)
    }
    async fn test_stream(&self, id: &str) -> Result<(), MediaError> {
        self.test_stream(id).await
    }
    async fn received_bytes(&self, id: &str) -> Result<Option<u64>, String> {
        self.received_bytes(id).await
    }
    async fn shutdown(&self) {
        self.shutdown().await
    }
    async fn simulate_crash(&self) {
        self.simulate_crash().await
    }
}
