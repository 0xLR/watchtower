use crate::{Engine, MediaError};
use axum::{
    extract::{ws::Message, Path, Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as Upstream};
type Params = std::collections::HashMap<String, String>;
pub async fn start(
    engine: Arc<Engine>,
) -> Result<(String, tokio::task::JoinHandle<()>), MediaError> {
    let token = uuid::Uuid::new_v4().simple().to_string();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|_| MediaError::Unavailable)?;
    let port = listener
        .local_addr()
        .map_err(|_| MediaError::Unavailable)?
        .port();
    let router = Router::new()
        .route(&format!("/{token}/api/ws"), get(websocket))
        .route(&format!("/{token}/api/{{*path}}"), get(http))
        .with_state(engine);
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Ok((format!("http://127.0.0.1:{port}/{token}"), task))
}
async fn websocket(
    State(engine): State<Arc<Engine>>,
    Query(params): Query<Params>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !allowed_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(id) = params.get("src") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !engine.sources.read().await.contains_key(id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut url =
        match reqwest::Url::parse(&format!("{}/api/ws", engine.api.replace("http:", "ws:"))) {
            Ok(u) => u,
            Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
        };
    url.query_pairs_mut().append_pair("src", id);
    let mut request = match url.as_str().into_client_request() {
        Ok(r) => r,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };
    let auth = engine
        .request(reqwest::Method::GET, "/")
        .build()
        .ok()
        .and_then(|r| r.headers().get("authorization").cloned());
    if let Some(auth) = auth {
        request.headers_mut().insert("authorization", auth);
    }
    let Ok((upstream, _)) = tokio_tungstenite::connect_async(request).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    ws.max_message_size(1024*1024).on_upgrade(move |socket|async move {
        let (mut client_send,mut client_recv)=socket.split();
        let (mut upstream_send,mut upstream_recv)=upstream.split();
        loop {
            tokio::select! {
                input=client_recv.next()=>match input {
                    Some(Ok(Message::Text(text)))=>{
                        let Ok(value)=serde_json::from_str::<serde_json::Value>(&text) else {break;};
                        let kind=value["type"].as_str().unwrap_or("");
                        if !matches!(kind,"mse"|"hls"|"webrtc/offer"|"webrtc/candidate") {break;}
                        if upstream_send.send(Upstream::Text(text.to_string().into())).await.is_err() {break;}
                    },
                    Some(Ok(Message::Ping(_)))=>{}, _=>break,
                },
                output=upstream_recv.next()=>match output {
                    Some(Ok(Upstream::Binary(bytes)))=>if client_send.send(Message::Binary(bytes)).await.is_err() {break;},
                    Some(Ok(Upstream::Text(text)))=>{
                        let Ok(mut value)=serde_json::from_str::<serde_json::Value>(&text) else {break;};
                        if value["type"]=="error" {value["value"]="Camera connection failed".into();}
                        if client_send.send(Message::Text(value.to_string().into())).await.is_err() {break;}
                    },
                    Some(Ok(Upstream::Ping(_)))=>{}, _=>break,
                }
            }
        }
        let _=upstream_send.close().await;
    })
}
fn allowed_origin(headers: &HeaderMap) -> bool {
    headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|origin| {
            matches!(
                origin,
                "tauri://localhost"
                    | "http://tauri.localhost"
                    | "https://tauri.localhost"
                    | "http://127.0.0.1:1420"
            )
        })
}
async fn http(
    State(engine): State<Arc<Engine>>,
    Path(path): Path<String>,
    Query(params): Query<Params>,
    headers: HeaderMap,
) -> Response {
    if !allowed_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !matches!(
        path.as_str(),
        "stream.m3u8" | "hls/playlist.m3u8" | "hls/segment.ts" | "hls/init.mp4" | "hls/segment.m4s"
    ) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path == "stream.m3u8"
        && !engine
            .sources
            .read()
            .await
            .contains_key(params.get("src").map(String::as_str).unwrap_or(""))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(response) = engine
        .request(reqwest::Method::GET, &format!("/api/{path}"))
        .query(&params)
        .send()
        .await
    else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if !response.status().is_success() {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let content_type = response.headers().get("content-type").cloned();
    let mut result = axum::body::Body::from_stream(response.bytes_stream()).into_response();
    if let Some(value) = content_type {
        result.headers_mut().insert("content-type", value);
    }
    if let Some(origin) = headers.get("origin") {
        result
            .headers_mut()
            .insert("access-control-allow-origin", origin.clone());
    }
    result.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    result
}
