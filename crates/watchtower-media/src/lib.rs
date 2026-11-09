//! Supervised go2rtc; source URLs stay in memory. All errors are sanitized.
mod gateway;
mod process;
use process::SupervisedChild;
use std::{collections::HashMap, path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    process::Command,
    sync::{Mutex, RwLock},
    time::sleep,
};
use uuid::Uuid;
use watchtower_core::{Playback, StreamBackend, StreamStatus};

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("Media engine is unavailable. Fetch the bundled go2rtc binary and restart.")]
    Unavailable,
    #[error("The media engine could not connect. Check the camera address and credentials.")]
    Connection,
}
pub(crate) struct Engine {
    pub client: reqwest::Client,
    pub api: String,
    pub password: String,
    pub sources: RwLock<HashMap<String, String>>,
    operations: Mutex<()>,
    child: Mutex<SupervisedChild>,
}
pub struct Media {
    pub(crate) engine: Arc<Engine>,
    playback_origin: String,
    supervisor: tokio::task::JoinHandle<()>,
    gateway: tokio::task::JoinHandle<()>,
}
impl Drop for Media {
    fn drop(&mut self) {
        self.supervisor.abort();
        self.gateway.abort();
    }
}
fn free_port() -> Result<u16, MediaError> {
    let socket =
        std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|_| MediaError::Unavailable)?;
    socket
        .local_addr()
        .map(|a| a.port())
        .map_err(|_| MediaError::Unavailable)
}
fn spawn(
    binary: &Path,
    api_port: u16,
    rtc_port: u16,
    password: &str,
) -> Result<SupervisedChild, MediaError> {
    // Only launch credentials in process arguments; camera secrets arrive over HTTP.
    let config = serde_json::json!({
        "app":{"modules":["api","ws","rtsp","webrtc","mp4","hls"]},
        "api":{"listen":format!("127.0.0.1:{api_port}"),"username":"watchtower","password":password,"local_auth":true},
        "rtsp":{"listen":""},"srt":{"listen":""},
        "webrtc":{"listen":format!("127.0.0.1:{rtc_port}"),"candidates":[format!("127.0.0.1:{rtc_port}")],"ice_servers":[],"filters":{"ips":["127.0.0.1"]}},
        "log":{"level":"disabled"},"streams":{}
    });
    let mut command = Command::new(binary);
    command
        .args(["-config", &config.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    #[cfg(target_os = "linux")]
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() == 1 {
                return Err(std::io::Error::other("Parent exited"));
            }
            Ok(())
        });
    }
    process::own(command.spawn().map_err(|_| MediaError::Unavailable)?)
}
impl Media {
    pub async fn launch(binary: &Path) -> Result<Self, MediaError> {
        for _ in 0..4 {
            let port = free_port()?;
            let rtc_port = free_port()?;
            let password = Uuid::new_v4().simple().to_string();
            let child = spawn(binary, port, rtc_port, &password)?;
            let engine = Arc::new(Engine {
                client: reqwest::Client::builder()
                    .no_proxy()
                    .timeout(Duration::from_secs(12))
                    .build()
                    .map_err(|_| MediaError::Unavailable)?,
                api: format!("http://127.0.0.1:{port}"),
                password,
                sources: RwLock::new(HashMap::new()),
                operations: Mutex::new(()),
                child: Mutex::new(child),
            });
            let mut ready = false;
            for _ in 0..40 {
                if engine.ready().await {
                    ready = true;
                    break;
                }
                sleep(Duration::from_millis(100)).await;
            }
            if !ready {
                let _ = engine.child.lock().await.kill().await;
                continue;
            }
            let (playback_origin, gateway) = gateway::start(engine.clone()).await?;
            let supervised = engine.clone();
            let binary = binary.to_owned();
            let supervisor = tokio::spawn(async move {
                loop {
                    sleep(Duration::from_secs(1)).await;
                    let exited = supervised
                        .child
                        .lock()
                        .await
                        .try_wait()
                        .map_or(true, |s| s.is_some());
                    if !exited {
                        continue;
                    }
                    tracing::warn!("Media engine stopped; restarting");
                    sleep(Duration::from_secs(1)).await;
                    if let Ok(child) = spawn(&binary, port, rtc_port, &supervised.password) {
                        *supervised.child.lock().await = child;
                        for _ in 0..30 {
                            sleep(Duration::from_millis(100)).await;
                            if supervised.ready().await {
                                break;
                            }
                        }
                        let _guard = supervised.operations.lock().await;
                        let sources = supervised.sources.read().await.clone();
                        for (id, source) in sources {
                            let _ = supervised.put(&id, &source).await;
                        }
                    }
                }
            });
            return Ok(Self {
                engine,
                playback_origin,
                supervisor,
                gateway,
            });
        }
        Err(MediaError::Unavailable)
    }
    /// Reconcile a saved source lazily after a failed registration or crash.
    pub async fn ensure_stream(&self, id: &str, source: &str) -> Result<(), String> {
        let _guard = self.engine.operations.lock().await;
        if self
            .engine
            .sources
            .read()
            .await
            .get(id)
            .is_some_and(|current| current == source)
        {
            return Ok(());
        }
        self.engine
            .put(id, source)
            .await
            .map_err(|e| e.to_string())?;
        self.engine
            .sources
            .write()
            .await
            .insert(id.into(), source.into());
        Ok(())
    }
    pub fn playback(&self, id: &str) -> Playback {
        Playback {
            websocket_url: format!(
                "{}/api/ws?src={id}",
                self.playback_origin.replace("http:", "ws:")
            ),
            hls_url: format!("{}/api/stream.m3u8?src={id}&mp4", self.playback_origin),
        }
    }
    pub async fn test_stream(&self, id: &str) -> Result<(), MediaError> {
        self.engine
            .request(reqwest::Method::GET, "/api/streams")
            .query(&[("src", id), ("video", "all")])
            .send()
            .await
            .map_err(|_| MediaError::Connection)?
            .error_for_status()
            .map_err(|_| MediaError::Connection)?;
        Ok(())
    }
    /// Aggregate bytes from live producers; never expose the underlying JSON.
    pub async fn received_bytes(&self, id: &str) -> Result<Option<u64>, String> {
        let value: serde_json::Value = self
            .engine
            .request(reqwest::Method::GET, "/api/streams")
            .query(&[("src", id)])
            .send()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?
            .json()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?;
        let Some(producers) = value["producers"].as_array() else {
            return Ok(None);
        };
        let active: Vec<_> = producers
            .iter()
            .filter(|p| p["medias"].as_array().is_some_and(|m| !m.is_empty()))
            .collect();
        if active.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            active
                .iter()
                .map(|p| p["bytes_recv"].as_u64().unwrap_or(0))
                .sum(),
        ))
    }
    pub async fn shutdown(&self) {
        self.supervisor.abort();
        self.gateway.abort();
        let _ = self.engine.child.lock().await.kill().await;
    }
    #[doc(hidden)]
    pub async fn simulate_crash(&self) {
        let _ = self.engine.child.lock().await.kill().await;
    }
}
impl Engine {
    async fn ready(&self) -> bool {
        // The HTTP listener precedes transport initialization in go2rtc's main.
        // Require registered MP4 and HLS handlers, rather than accepting an early API response.
        for path in [
            "/api/frame.mp4?src=__watchtower_health",
            "/api/stream.m3u8?src=__watchtower_health",
        ] {
            let Ok(response) = self.request(reqwest::Method::GET, path).send().await else {
                return false;
            };
            if response.status() != reqwest::StatusCode::NOT_FOUND
                || !response
                    .text()
                    .await
                    .is_ok_and(|body| body.trim() == "stream not found")
            {
                return false;
            }
        }
        true
    }

    pub(crate) fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{path}", self.api))
            .basic_auth("watchtower", Some(&self.password))
    }
    async fn put(&self, id: &str, source: &str) -> Result<(), MediaError> {
        let response = self
            .request(reqwest::Method::PUT, "/api/streams")
            .query(&[("name", id), ("src", source)])
            .send()
            .await
            .map_err(|_| MediaError::Unavailable)?;
        // Stream is created before this error: config file deliberately disabled.
        if !response.status().is_success()
            && response
                .text()
                .await
                .map_err(|_| MediaError::Unavailable)?
                .trim()
                != "config file disabled"
        {
            return Err(MediaError::Connection);
        }
        Ok(())
    }
}
#[async_trait::async_trait]
impl StreamBackend for Media {
    async fn add_stream(&self, id: &str, source: &str) -> Result<(), String> {
        let _guard = self.engine.operations.lock().await;
        self.engine
            .put(id, source)
            .await
            .map_err(|e| e.to_string())?;
        self.engine
            .sources
            .write()
            .await
            .insert(id.into(), source.into());
        Ok(())
    }
    async fn update_stream(&self, id: &str, source: &str) -> Result<(), String> {
        self.add_stream(id, source).await
    }
    async fn remove_stream(&self, id: &str) -> Result<(), String> {
        let _guard = self.engine.operations.lock().await;
        self.engine.sources.write().await.remove(id);
        self.engine
            .request(reqwest::Method::DELETE, "/api/streams")
            .query(&[("src", id)])
            .send()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?;
        Ok(())
    }
    async fn stream_status(&self, id: &str) -> Result<StreamStatus, String> {
        let value: serde_json::Value = self
            .engine
            .request(reqwest::Method::GET, "/api/streams")
            .query(&[("src", id)])
            .send()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?
            .json()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?;
        let live = value["producers"].as_array().is_some_and(|p| {
            p.iter()
                .any(|v| v["medias"].as_array().is_some_and(|m| !m.is_empty()))
        });
        Ok(if live {
            StreamStatus::Live
        } else {
            StreamStatus::Idle
        })
    }
}
