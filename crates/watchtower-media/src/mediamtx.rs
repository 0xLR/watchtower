//! Supervised MediaMTX. Runtime path configuration is kept in memory.
use crate::{
    process::{self, SupervisedChild},
    Backend, MediaError,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use bytes::Bytes;
use std::{
    collections::HashMap,
    path::{Path as FilePath, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{
    process::Command,
    sync::{Mutex, RwLock},
    task::JoinHandle,
    time::sleep,
};
use watchtower_core::{MediaEngine, Playback, StreamBackend, StreamStatus};
struct Shared {
    client: reqwest::Client,
    api: String,
    hls: String,
    rtc: String,
    password: String,
    sources: RwLock<HashMap<String, String>>,
    sessions: Mutex<HashMap<String, String>>,
    operations: Mutex<()>,
    child: Mutex<SupervisedChild>,
}
pub(crate) struct MediaMtx {
    shared: Arc<Shared>,
    origin: String,
    server: JoinHandle<()>,
    supervisor: JoinHandle<()>,
    _config: EmptyConfig,
}
struct EmptyConfig(PathBuf);
impl Drop for EmptyConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
impl Drop for MediaMtx {
    fn drop(&mut self) {
        self.server.abort();
        self.supervisor.abort();
    }
}
fn port() -> Result<u16, MediaError> {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|s| s.local_addr())
        .map(|a| a.port())
        .map_err(|_| MediaError::Unavailable)
}
fn spawn(
    binary: &FilePath,
    api: u16,
    hls: u16,
    rtc: u16,
    ice: u16,
    password: &str,
    config: &FilePath,
) -> Result<SupervisedChild, MediaError> {
    let users = serde_json::json!([{"user":"watchtower","pass":password,"ips":["127.0.0.1"],"permissions":[{"action":"api"},{"action":"read"}]}]);
    let mut command = Command::new(binary);
    // No camera source is passed through the environment or command line.
    // The config stub contains only {}, never camera sources or credentials.
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("MTX_") {
            command.env_remove(name);
        }
    }
    command
        .arg(config)
        .envs([
            ("MTX_API", "yes".into()),
            ("MTX_APIADDRESS", format!("127.0.0.1:{api}")),
            ("MTX_HLSADDRESS", format!("127.0.0.1:{hls}")),
            ("MTX_WEBRTCADDRESS", format!("127.0.0.1:{rtc}")),
            ("MTX_WEBRTCLOCALUDPADDRESS", format!("127.0.0.1:{ice}")),
            ("MTX_WEBRTCIPSFROMINTERFACES", "no".into()),
            ("MTX_WEBRTCADDITIONALHOSTS", "127.0.0.1".into()),
            ("MTX_WEBRTCICESERVERS2", "[]".into()),
            ("MTX_AUTHINTERNALUSERS", users.to_string()),
            ("MTX_RTSP", "no".into()),
            ("MTX_RTMP", "no".into()),
            ("MTX_SRT", "no".into()),
            ("MTX_MOQ", "no".into()),
            ("MTX_PLAYBACK", "no".into()),
            ("MTX_METRICS", "no".into()),
            ("MTX_PPROF", "no".into()),
            ("MTX_LOGLEVEL", "error".into()),
            ("MTX_HLSVARIANT", "fmp4".into()),
            ("MTX_HLSMUXERCLOSEAFTER", "10s".into()),
        ])
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
impl Shared {
    fn request(&self, method: Method, url: String) -> reqwest::RequestBuilder {
        self.client
            .request(method, url)
            .basic_auth("watchtower", Some(&self.password))
    }
    async fn ready(&self) -> bool {
        self.request(Method::GET, format!("{}/v3/config/global/get", self.api))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }
    async fn put(&self, id: &str, source: &str) -> Result<(), String> {
        let body = serde_json::json!({"source":source,"sourceOnDemand":true,"sourceOnDemandCloseAfter":"2s","rtspTransport":"tcp"});
        let response = self
            .request(
                Method::POST,
                format!("{}/v3/config/paths/add/{id}", self.api),
            )
            .json(&body)
            .send()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?;
        if response.status().is_success() {
            return Ok(());
        }
        let response = self
            .request(
                Method::PATCH,
                format!("{}/v3/config/paths/patch/{id}", self.api),
            )
            .json(&body)
            .send()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(MediaError::Connection.to_string())
        }
    }
}
impl MediaMtx {
    pub async fn launch(binary: &FilePath) -> Result<Self, MediaError> {
        let config = EmptyConfig(std::env::temp_dir().join(format!(
            "watchtower-mediamtx-{}.yml",
            uuid::Uuid::new_v4().simple()
        )));
        std::fs::write(&config.0, b"{}\n").map_err(|_| MediaError::Unavailable)?;
        let (api, hls, rtc, ice) = (port()?, port()?, port()?, port()?);
        let password = uuid::Uuid::new_v4().simple().to_string();
        let child = spawn(binary, api, hls, rtc, ice, &password, &config.0)?;
        let shared = Arc::new(Shared {
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(14))
                .build()
                .map_err(|_| MediaError::Unavailable)?,
            api: format!("http://127.0.0.1:{api}"),
            hls: format!("http://127.0.0.1:{hls}"),
            rtc: format!("http://127.0.0.1:{rtc}"),
            password,
            sources: RwLock::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            operations: Mutex::new(()),
            child: Mutex::new(child),
        });
        let mut ready = false;
        for _ in 0..50 {
            if shared.ready().await {
                ready = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        if !ready {
            let _ = shared.child.lock().await.kill().await;
            return Err(MediaError::Unavailable);
        }
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|_| MediaError::Unavailable)?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let origin = format!(
            "http://127.0.0.1:{}/{token}",
            listener
                .local_addr()
                .map_err(|_| MediaError::Unavailable)?
                .port()
        );
        let router = Router::new()
            .route(&format!("/{token}/{{id}}/{{*path}}"), any(proxy))
            .with_state(shared.clone());
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        let supervised = shared.clone();
        let binary: PathBuf = binary.into();
        let config_path = config.0.clone();
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
                supervised.sessions.lock().await.clear();
                sleep(Duration::from_secs(1)).await;
                if let Ok(child) = spawn(
                    &binary,
                    api,
                    hls,
                    rtc,
                    ice,
                    &supervised.password,
                    &config_path,
                ) {
                    *supervised.child.lock().await = child;
                    for _ in 0..40 {
                        if supervised.ready().await {
                            break;
                        }
                        sleep(Duration::from_millis(100)).await;
                    }
                    let _guard = supervised.operations.lock().await;
                    for (id, source) in supervised.sources.read().await.clone() {
                        let _ = supervised.put(&id, &source).await;
                    }
                }
            }
        });
        Ok(Self {
            shared,
            origin,
            server,
            supervisor,
            _config: config,
        })
    }
}
fn cors(mut response: Response, headers: &HeaderMap) -> Response {
    if let Some(origin) = headers.get("origin") {
        response
            .headers_mut()
            .insert("access-control-allow-origin", origin.clone());
    }
    for (key, value) in [
        ("access-control-allow-methods", "POST, DELETE, GET, OPTIONS"),
        ("access-control-allow-headers", "Content-Type"),
        ("access-control-expose-headers", "Location"),
        ("cache-control", "no-store"),
    ] {
        response
            .headers_mut()
            .insert(key, axum::http::HeaderValue::from_static(value));
    }
    response
}
async fn proxy(
    State(shared): State<Arc<Shared>>,
    Path((id, path)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    uri: axum::http::Uri,
    body: Bytes,
) -> Response {
    if !crate::gateway::allowed_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if method == Method::OPTIONS {
        return cors(StatusCode::NO_CONTENT.into_response(), &headers);
    }
    if !shared.sources.read().await.contains_key(&id) {
        return cors(StatusCode::NOT_FOUND.into_response(), &headers);
    }
    let is_whep = path == "whep";
    let session = path.strip_prefix("sessions/");
    let target = if method == Method::POST && is_whep {
        format!("{}/{id}/whep", shared.rtc)
    } else if method == Method::DELETE && session.is_some() {
        let Some(url) = shared
            .sessions
            .lock()
            .await
            .remove(&format!("{id}/{}", session.unwrap()))
        else {
            return cors(StatusCode::NOT_FOUND.into_response(), &headers);
        };
        url
    } else if method == Method::GET
        && !path.contains('/')
        && !path.contains("..")
        && (path.ends_with(".m3u8")
            || path.ends_with(".mp4")
            || path.ends_with(".m4s")
            || path.ends_with(".ts"))
    {
        format!(
            "{}/{id}/{path}{}",
            shared.hls,
            uri.query().map(|q| format!("?{q}")).unwrap_or_default()
        )
    } else {
        return cors(StatusCode::NOT_FOUND.into_response(), &headers);
    };
    let mut request = shared.request(method.clone(), target);
    if method == Method::POST {
        if body.len() > 131_072 {
            return cors(StatusCode::PAYLOAD_TOO_LARGE.into_response(), &headers);
        }
        request = request.header("content-type", "application/sdp").body(body);
    }
    let Ok(upstream) = request.send().await else {
        return cors(StatusCode::BAD_GATEWAY.into_response(), &headers);
    };
    if !upstream.status().is_success() {
        return cors(
            (StatusCode::BAD_GATEWAY, "MediaMTX playback failed").into_response(),
            &headers,
        );
    }
    let status = upstream.status();
    let content_type = upstream.headers().get("content-type").cloned();
    let location = upstream
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let mut response = (
        status,
        axum::body::Body::from_stream(upstream.bytes_stream()),
    )
        .into_response();
    if let Some(value) = content_type {
        response.headers_mut().insert("content-type", value);
    }
    if is_whep && method == Method::POST {
        if let Some(location) = location {
            let target = if location.starts_with('/') {
                format!("{}{location}", shared.rtc)
            } else {
                location
            };
            if target.starts_with(&format!("{}/{id}/whep/", shared.rtc)) {
                let key = uuid::Uuid::new_v4().simple().to_string();
                shared
                    .sessions
                    .lock()
                    .await
                    .insert(format!("{id}/{key}"), target);
                let path = uri.path().trim_end_matches("whep");
                response
                    .headers_mut()
                    .insert("location", format!("{path}sessions/{key}").parse().unwrap());
            }
        }
    }
    cors(response, &headers)
}
#[async_trait::async_trait]
impl StreamBackend for MediaMtx {
    async fn add_stream(&self, id: &str, source: &str) -> Result<(), String> {
        let _guard = self.shared.operations.lock().await;
        self.shared.put(id, source).await?;
        self.shared
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
        let _guard = self.shared.operations.lock().await;
        self.shared.sources.write().await.remove(id);
        self.shared
            .sessions
            .lock()
            .await
            .retain(|key, _| !key.starts_with(&format!("{id}/")));
        self.shared
            .request(
                Method::DELETE,
                format!("{}/v3/config/paths/delete/{id}", self.shared.api),
            )
            .send()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?;
        Ok(())
    }
    async fn stream_status(&self, id: &str) -> Result<StreamStatus, String> {
        let response = self
            .shared
            .request(
                Method::GET,
                format!("{}/v3/paths/get/{id}", self.shared.api),
            )
            .send()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?;
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|_| MediaError::Connection.to_string())?;
        Ok(if value["ready"].as_bool() == Some(true) {
            StreamStatus::Live
        } else {
            StreamStatus::Idle
        })
    }
}
#[async_trait::async_trait]
impl Backend for MediaMtx {
    fn playback(&self, id: &str) -> Playback {
        Playback {
            engine: MediaEngine::MediaMtx,
            transports: vec!["webrtc".into(), "hls".into()],
            websocket_url: String::new(),
            hls_url: format!("{}/{id}/index.m3u8", self.origin),
            whep_url: Some(format!("{}/{id}/whep", self.origin)),
        }
    }
    async fn test_stream(&self, id: &str) -> Result<(), MediaError> {
        let response = self
            .shared
            .request(Method::GET, format!("{}/{id}/index.m3u8", self.shared.hls))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    MediaError::Timeout
                } else {
                    MediaError::Connection
                }
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(MediaError::EngineResponse(response.status().as_u16()))
        }
    }
    async fn received_bytes(&self, id: &str) -> Result<Option<u64>, String> {
        let value: serde_json::Value = self
            .shared
            .request(
                Method::GET,
                format!("{}/v3/paths/get/{id}", self.shared.api),
            )
            .send()
            .await
            .map_err(|_| MediaError::Unavailable.to_string())?
            .json()
            .await
            .map_err(|_| MediaError::Connection.to_string())?;
        Ok(value["bytesReceived"].as_u64().filter(|b| *b > 0))
    }
    async fn shutdown(&self) {
        self.supervisor.abort();
        self.server.abort();
        let _ = self.shared.child.lock().await.kill().await;
    }
    async fn simulate_crash(&self) {
        let _ = self.shared.child.lock().await.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires bundled MediaMTX"]
    async fn runtime_sources_never_enter_the_config_stub() {
        let binary = PathBuf::from(std::env::var("WATCHTOWER_TEST_MEDIAMTX").unwrap());
        let media = MediaMtx::launch(&binary).await.unwrap();
        let config = media._config.0.clone();
        media
            .add_stream(
                "privacy_fixture",
                "rtsp://synthetic-user:synthetic-password@127.0.0.1:8554/watchtower-test",
            )
            .await
            .unwrap();
        assert!(std::fs::read(&config).unwrap() == b"{}\n");
        media.remove_stream("privacy_fixture").await.unwrap();
        media.shutdown().await;
        drop(media);
        assert!(!config.exists());
    }
}
