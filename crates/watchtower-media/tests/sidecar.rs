use std::{path::PathBuf, time::Duration};
use watchtower_core::{StreamBackend, StreamStatus};
use watchtower_media::Media;

#[tokio::test]
#[ignore = "requires fetched sidecar; set WATCHTOWER_TEST_SIDECAR"]
async fn sidecar_lifecycle_and_playback_boundary() {
    let binary = PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR").expect("sidecar path"));
    let media = Media::launch(&binary).await.expect("launch");
    media
        .add_stream("fixture", "rtsp://127.0.0.1:8554/watchtower-test")
        .await
        .expect("add");
    assert_eq!(
        media.stream_status("fixture").await.expect("status"),
        StreamStatus::Idle
    );
    let playback = media.playback("fixture");
    assert!(playback.websocket_url.starts_with("ws://127.0.0.1:"));
    let http = format!(
        "{}/streams",
        playback.hls_url.split("/stream.m3u8").next().unwrap()
    );
    assert_eq!(reqwest::get(http).await.expect("gateway").status(), 404);
    media.simulate_crash().await;
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(media.stream_status("fixture").await.is_ok());
    media.remove_stream("fixture").await.expect("remove");
    media.shutdown().await;
}

#[tokio::test]
#[ignore = "requires local RTSP fixture and fetched sidecar"]
async fn rtsp_fixture_produces_video() {
    let binary = PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR").expect("sidecar path"));
    let media = Media::launch(&binary).await.expect("launch");
    media
        .add_stream("fixture", "rtsp://127.0.0.1:8554/watchtower-test")
        .await
        .expect("add");
    media
        .test_stream("fixture")
        .await
        .expect("RTSP video probe");
    media.shutdown().await;
}

#[tokio::test]
#[ignore = "requires local RTSP fixture and fetched sidecar"]
async fn missing_rtsp_path_has_safe_specific_error() {
    let binary = PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR").expect("sidecar path"));
    let media = Media::launch(&binary).await.unwrap();
    media
        .add_stream(
            "missing",
            "rtsp://synthetic-user:synthetic-password@127.0.0.1:8554/missing-path",
        )
        .await
        .unwrap();
    let error = media.test_stream("missing").await.unwrap_err();
    assert!(matches!(
        error,
        watchtower_media::MediaError::DescribeRejected
    ));
    assert!(!error.to_string().contains("synthetic"));
    media.shutdown().await;
}

#[tokio::test]
#[ignore = "requires local synthetic RTSP source and fetched sidecar"]
async fn mse_gateway_delivers_video_and_health_bytes() {
    use futures_util::{SinkExt, StreamExt};
    let binary = PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR").expect("sidecar path"));
    let media = Media::launch(&binary).await.expect("launch");
    media
        .add_stream("fixture", "rtsp://127.0.0.1:8554/watchtower-test")
        .await
        .expect("add");
    let (mut socket, _) = tokio_tungstenite::connect_async(media.playback("fixture").websocket_url)
        .await
        .expect("playback socket");
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"type":"mse","value":"avc1.640029,avc1.64002A,avc1.640033"}"#.into(),
        ))
        .await
        .unwrap();
    let binary = tokio::time::timeout(Duration::from_secs(8), async {
        while let Some(Ok(message)) = socket.next().await {
            if message.is_binary() {
                return true;
            }
        }
        false
    })
    .await
    .unwrap();
    assert!(binary);
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(media
        .received_bytes("fixture")
        .await
        .unwrap()
        .is_some_and(|bytes| bytes > 0));
    socket.close(None).await.unwrap();
    media.shutdown().await;
}
