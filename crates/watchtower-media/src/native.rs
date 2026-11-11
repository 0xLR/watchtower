//! Native RTSP/H.264 to WebRTC. No transcoding, helper executable or source files.
use crate::{Backend, MediaError};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, post},
    Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use retina::{
    client::{Credentials, Demuxed, Session, SessionOptions, SetupOptions},
    codec::{CodecItem, FrameFormat},
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::{broadcast, watch, Mutex, RwLock},
    task::JoinHandle,
    time::timeout,
};
use watchtower_core::{MediaEngine, Playback, StreamBackend, StreamStatus};
use webrtc::{
    api::{media_engine::MediaEngine as RtcMedia, setting_engine::SettingEngine, APIBuilder},
    ice::{mdns::MulticastDnsMode, network_type::NetworkType},
    media::Sample,
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription, RTCPeerConnection,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
};

#[derive(Clone)]
struct Frame {
    data: Bytes,
    ticks: i64,
    key: bool,
}
struct Source {
    url: String,
    frames: broadcast::Sender<Frame>,
    reader: Mutex<Option<JoinHandle<()>>>,
    bytes: AtomicU64,
    cancelled: watch::Sender<bool>,
}
impl Source {
    fn new(url: &str) -> Self {
        let (frames, _) = broadcast::channel(32);
        let (cancelled, _) = watch::channel(false);
        Self {
            url: url.into(),
            frames,
            reader: Mutex::new(None),
            bytes: AtomicU64::new(0),
            cancelled,
        }
    }
    async fn subscribe(self: &Arc<Self>) -> broadcast::Receiver<Frame> {
        let receiver = self.frames.subscribe();
        let mut reader = self.reader.lock().await;
        if reader.as_ref().is_none_or(JoinHandle::is_finished) {
            let source = self.clone();
            *reader = Some(tokio::spawn(async move {
                let Ok(Ok(mut session)) = timeout(Duration::from_secs(12), open(&source.url)).await
                else {
                    return;
                };
                let mut cancel = source.cancelled.subscribe();
                let mut idle = tokio::time::interval(Duration::from_secs(1));
                loop {
                    tokio::select! {
                        _=cancel.changed()=>break,
                        _=idle.tick()=>if source.frames.receiver_count()==0{break;},
                        item=timeout(Duration::from_secs(12),session.next())=>match item {
                            Ok(Some(Ok(CodecItem::VideoFrame(frame))))=>{
                                let ticks=frame.timestamp().elapsed(); let key=frame.is_random_access_point();
                                let data=Bytes::from(frame.into_data());
                                source.bytes.fetch_add(data.len() as u64,Ordering::Relaxed);
                                let _=source.frames.send(Frame{data,ticks,key});
                            },
                            Ok(Some(Ok(_)))=>{}, _=>break,
                        }
                    }
                }
            }));
        }
        receiver
    }
    async fn stop(&self) {
        let _ = self.cancelled.send(true);
        if let Some(reader) = self.reader.lock().await.take() {
            reader.abort();
        }
    }
}
async fn open(source: &str) -> Result<Demuxed, MediaError> {
    let mut url = url::Url::parse(source).map_err(|_| MediaError::Connection)?;
    let decode = |value: &str| {
        percent_encoding::percent_decode_str(value)
            .decode_utf8()
            .map(|v| v.into_owned())
            .map_err(|_| MediaError::Connection)
    };
    let creds = if url.username().is_empty() {
        None
    } else {
        Some(Credentials {
            username: decode(url.username())?,
            password: decode(url.password().unwrap_or(""))?,
        })
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    let mut session = Session::describe(
        url,
        SessionOptions::default()
            .creds(creds)
            .user_agent("Watchtower Retina".into()),
    )
    .await
    .map_err(|e| crate::probe_error::classify(&e.to_string()))?;
    let video = session
        .streams()
        .iter()
        .position(|s| s.media() == "video" && s.encoding_name() == "h264")
        .ok_or(MediaError::Unsupported)?;
    session
        .setup(
            video,
            SetupOptions::default().frame_format(FrameFormat::SIMPLE),
        )
        .await
        .map_err(|_| MediaError::SetupRejected)?;
    session
        .play(Default::default())
        .await
        .map_err(|_| MediaError::Connection)?
        .demuxed()
        .map_err(|_| MediaError::Connection)
}
struct Peer {
    pc: Arc<RTCPeerConnection>,
    task: JoinHandle<()>,
}
struct Shared {
    sources: RwLock<HashMap<String, Arc<Source>>>,
    peers: Mutex<HashMap<String, Peer>>,
    origin: String,
    cancelled: watch::Sender<bool>,
}
pub(crate) struct Native {
    shared: Arc<Shared>,
    server: JoinHandle<()>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.server.abort();
        let _ = self.shared.cancelled.send(true);
    }
}
impl Native {
    pub async fn launch() -> Result<Self, MediaError> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|_| MediaError::Unavailable)?;
        let origin = format!(
            "http://127.0.0.1:{}/{}",
            listener
                .local_addr()
                .map_err(|_| MediaError::Unavailable)?
                .port(),
            uuid::Uuid::new_v4().simple()
        );
        let token = origin.rsplit('/').next().unwrap();
        let (cancelled, _) = watch::channel(false);
        let shared = Arc::new(Shared {
            sources: RwLock::new(HashMap::new()),
            peers: Mutex::new(HashMap::new()),
            origin: origin.clone(),
            cancelled,
        });
        let router = Router::new()
            .route(
                &format!("/{token}/{{id}}/whep"),
                post(offer).options(preflight),
            )
            .route(
                &format!("/{token}/sessions/{{id}}"),
                delete(close).options(preflight),
            )
            .with_state(shared.clone());
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self { shared, server })
    }
}
fn cors(mut response: Response, headers: &HeaderMap) -> Response {
    if let Some(origin) = headers.get("origin") {
        response
            .headers_mut()
            .insert("access-control-allow-origin", origin.clone());
    }
    for (key, value) in [
        ("access-control-allow-methods", "POST, DELETE, OPTIONS"),
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
async fn preflight(headers: HeaderMap) -> Response {
    if !crate::gateway::allowed_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    cors(StatusCode::NO_CONTENT.into_response(), &headers)
}
async fn close(
    State(shared): State<Arc<Shared>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !crate::gateway::allowed_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(peer) = shared.peers.lock().await.remove(&id) {
        let _ = peer.pc.close().await;
        peer.task.abort();
    }
    cors(StatusCode::NO_CONTENT.into_response(), &headers)
}
async fn offer(
    State(shared): State<Arc<Shared>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    sdp: String,
) -> Response {
    if !crate::gateway::allowed_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if sdp.len() > 131_072 {
        return cors(StatusCode::PAYLOAD_TOO_LARGE.into_response(), &headers);
    }
    let source = shared.sources.read().await.get(&id).cloned();
    let Some(source) = source else {
        return cors(StatusCode::NOT_FOUND.into_response(), &headers);
    };
    let result = create_peer(shared.clone(), source, sdp).await;
    match result {
        Ok((answer, id)) => {
            let mut response = (
                StatusCode::CREATED,
                [("content-type", "application/sdp")],
                answer,
            )
                .into_response();
            response.headers_mut().insert(
                "location",
                format!("{}/sessions/{id}", shared.origin).parse().unwrap(),
            );
            cors(response, &headers)
        }
        Err(_) => cors(
            (StatusCode::BAD_GATEWAY, "WebRTC connection failed").into_response(),
            &headers,
        ),
    }
}
async fn create_peer(
    shared: Arc<Shared>,
    source: Arc<Source>,
    sdp: String,
) -> Result<(String, String), MediaError> {
    let mut media = RtcMedia::default();
    media
        .register_default_codecs()
        .map_err(|_| MediaError::Connection)?;
    let mut settings = SettingEngine::default();
    settings.set_include_loopback_candidate(true);
    settings.set_ip_filter(Box::new(|ip| ip.is_loopback()));
    settings.set_network_types(vec![NetworkType::Udp4]);
    settings.set_ice_multicast_dns_mode(MulticastDnsMode::Disabled);
    settings.set_ice_timeouts(
        Some(Duration::from_secs(3)),
        Some(Duration::from_secs(5)),
        Some(Duration::from_secs(1)),
    );
    let api = APIBuilder::new()
        .with_media_engine(media)
        .with_setting_engine(settings)
        .build();
    let pc = Arc::new(
        api.new_peer_connection(RTCConfiguration::default())
            .await
            .map_err(|_| MediaError::Connection)?,
    );
    let track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: "video/H264".into(),
            clock_rate: 90000,
            sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
                .into(),
            ..Default::default()
        },
        "video".into(),
        "watchtower".into(),
    ));
    let result=async {
        let sender=pc.add_track(track.clone() as Arc<dyn TrackLocal+Send+Sync>).await.map_err(|_|MediaError::Connection)?;
        pc.set_remote_description(RTCSessionDescription::offer(sdp).map_err(|_|MediaError::Connection)?).await.map_err(|_|MediaError::Connection)?;
        let mut gathering=pc.gathering_complete_promise().await;
        let answer=pc.create_answer(None).await.map_err(|_|MediaError::Connection)?;
        pc.set_local_description(answer).await.map_err(|_|MediaError::Connection)?;
        timeout(Duration::from_secs(5),gathering.recv()).await.map_err(|_|MediaError::Timeout)?;
        let answer=pc.local_description().await.ok_or(MediaError::Connection)?.sdp;
        let mut frames=source.subscribe().await;
        let peer=pc.clone();let mut cancelled=shared.cancelled.subscribe();let mut source_cancelled=source.cancelled.subscribe();
        let session_id=uuid::Uuid::new_v4().simple().to_string();let finished_id=session_id.clone();let weak=Arc::downgrade(&shared);
        let task=tokio::spawn(async move {
            let rtcp=tokio::spawn(async move{while sender.read_rtcp().await.is_ok(){}});
            let mut previous=None;let mut key_required=true;let started=tokio::time::Instant::now();
            let mut health=tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    _=cancelled.changed()=>break,
                    _=source_cancelled.changed()=>break,
                    _=health.tick()=>{
                        if matches!(peer.connection_state(),RTCPeerConnectionState::Closed|RTCPeerConnectionState::Failed|RTCPeerConnectionState::Disconnected)||started.elapsed()>Duration::from_secs(15)&&peer.connection_state()!=RTCPeerConnectionState::Connected {break;}
                    },
                    frame=frames.recv()=>match frame {
                        Ok(frame)=>{
                            if peer.connection_state()!=RTCPeerConnectionState::Connected {continue;}
                            if key_required&&!frame.key{continue;}key_required=false;
                            let duration=sample_duration(previous,frame.ticks);previous=Some(frame.ticks);
                            if track.write_sample(&Sample{data:frame.data,duration,..Default::default()}).await.is_err(){break;}
                        },
                        Err(broadcast::error::RecvError::Lagged(_))=>{key_required=true;previous=None;},
                        Err(_)=>break,
                    }
                }
            }
            rtcp.abort();let _=peer.close().await;
            if let Some(shared)=weak.upgrade(){shared.peers.lock().await.remove(&finished_id);}
        });
        shared.peers.lock().await.insert(session_id.clone(),Peer{pc:pc.clone(),task});
        Ok((answer,session_id))
    }.await;
    if result.is_err() {
        let _ = pc.close().await;
    }
    result
}
fn sample_duration(previous: Option<i64>, current: i64) -> Duration {
    let ticks = previous
        .map(|p| current.saturating_sub(p))
        .filter(|d| *d > 0 && *d < 90000)
        .unwrap_or(3000);
    Duration::from_nanos(ticks as u64 * 1_000_000_000 / 90000)
}
#[async_trait::async_trait]
impl StreamBackend for Native {
    async fn add_stream(&self, id: &str, source: &str) -> Result<(), String> {
        if let Some(old) = self
            .shared
            .sources
            .write()
            .await
            .insert(id.into(), Arc::new(Source::new(source)))
        {
            old.stop().await;
        }
        Ok(())
    }
    async fn update_stream(&self, id: &str, source: &str) -> Result<(), String> {
        self.add_stream(id, source).await
    }
    async fn remove_stream(&self, id: &str) -> Result<(), String> {
        if let Some(source) = self.shared.sources.write().await.remove(id) {
            source.stop().await;
        }
        Ok(())
    }
    async fn stream_status(&self, id: &str) -> Result<StreamStatus, String> {
        let source = self.shared.sources.read().await.get(id).cloned();
        let Some(source) = source else {
            return Ok(StreamStatus::Idle);
        };
        let active = source
            .reader
            .lock()
            .await
            .as_ref()
            .is_some_and(|r| !r.is_finished());
        Ok(if active && source.bytes.load(Ordering::Relaxed) > 0 {
            StreamStatus::Live
        } else {
            StreamStatus::Idle
        })
    }
}
#[async_trait::async_trait]
impl Backend for Native {
    fn playback(&self, id: &str) -> Playback {
        Playback {
            engine: MediaEngine::Retina,
            transports: vec!["webrtc".into()],
            whep_url: Some(format!("{}/{id}/whep", self.shared.origin)),
            websocket_url: String::new(),
            hls_url: String::new(),
        }
    }
    async fn test_stream(&self, id: &str) -> Result<(), MediaError> {
        let source = self
            .shared
            .sources
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or(MediaError::Connection)?;
        if source.bytes.load(Ordering::Relaxed) > 0 && source.frames.receiver_count() > 0 {
            // A camera may allow only one active RTSP session. Verify fresh
            // frames from the shared reader instead of opening another session.
            let mut frames = source.subscribe().await;
            return timeout(Duration::from_secs(12), frames.recv())
                .await
                .map_err(|_| MediaError::Timeout)?
                .map(|_| ())
                .map_err(|_| MediaError::Connection);
        }
        timeout(Duration::from_secs(12), async {
            let mut session = open(&source.url).await?;
            while let Some(item) = session.next().await {
                match item {
                    Ok(CodecItem::VideoFrame(_)) => return Ok(()),
                    Ok(_) => {}
                    Err(_) => return Err(MediaError::Connection),
                }
            }
            Err(MediaError::Connection)
        })
        .await
        .map_err(|_| MediaError::Timeout)?
    }
    async fn received_bytes(&self, id: &str) -> Result<Option<u64>, String> {
        let sources = self.shared.sources.read().await;
        Ok(sources
            .get(id)
            .map(|s| s.bytes.load(Ordering::Relaxed))
            .filter(|b| *b > 0))
    }
    async fn shutdown(&self) {
        let _ = self.shared.cancelled.send(true);
        self.server.abort();
        for source in self.shared.sources.read().await.values() {
            source.stop().await;
        }
        for (_, peer) in self.shared.peers.lock().await.drain() {
            let _ = peer.pc.close().await;
            peer.task.abort();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_do_not_create_unbounded_or_negative_durations() {
        assert_eq!(sample_duration(Some(0), 9000), Duration::from_millis(100));
        assert_eq!(sample_duration(Some(100), 90), sample_duration(None, 0));
    }

    #[tokio::test]
    #[ignore = "requires local synthetic RTSP fixture"]
    async fn readers_are_shared_and_stop_after_the_last_viewer_leaves() {
        let source = Arc::new(Source::new("rtsp://127.0.0.1:8554/watchtower-test"));
        let mut first = source.subscribe().await;
        timeout(Duration::from_secs(6), first.recv())
            .await
            .unwrap()
            .unwrap();
        let reader_id = source.reader.lock().await.as_ref().unwrap().id();
        let mut second = source.subscribe().await;
        assert_eq!(source.reader.lock().await.as_ref().unwrap().id(), reader_id);
        drop(first);
        timeout(Duration::from_secs(2), second.recv())
            .await
            .unwrap()
            .unwrap();
        drop(second);
        tokio::time::sleep(Duration::from_millis(2200)).await;
        assert!(source.reader.lock().await.as_ref().unwrap().is_finished());
        let bytes = source.bytes.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(source.bytes.load(Ordering::Relaxed), bytes);
    }

    #[tokio::test]
    #[ignore = "requires local synthetic RTSP fixture"]
    async fn testing_a_live_stream_does_not_open_a_second_camera_connection() {
        use std::sync::atomic::AtomicUsize;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = attempts.clone();
        let proxy = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                let (mut camera, _) = listener.accept().await.unwrap();
                let number = observed.fetch_add(1, Ordering::SeqCst);
                if number > 0 {
                    drop(camera);
                    continue;
                }
                tasks.spawn(async move {
                    let mut fixture = tokio::net::TcpStream::connect(("127.0.0.1", 8554))
                        .await
                        .unwrap();
                    let _ = tokio::io::copy_bidirectional(&mut camera, &mut fixture).await;
                });
            }
        });
        let media = Native::launch().await.unwrap();
        media
            .add_stream("live", &format!("rtsp://127.0.0.1:{port}/watchtower-test"))
            .await
            .unwrap();
        let source = media.shared.sources.read().await["live"].clone();
        let mut viewer = source.subscribe().await;
        timeout(Duration::from_secs(6), viewer.recv())
            .await
            .unwrap()
            .unwrap();
        let result = media.test_stream("live").await;
        media.shutdown().await;
        proxy.abort();
        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
