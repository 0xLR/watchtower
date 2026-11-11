use std::path::PathBuf;
use watchtower_core::{MediaEngine, StreamBackend};
use watchtower_media::Media;

#[tokio::test]
#[ignore = "requires synthetic RTSP fixture"]
async fn retina_works_without_installed_executables() {
    let missing = std::env::temp_dir().join(format!(
        "watchtower-not-installed-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let media = Media::new(&missing, &missing);
    media
        .ensure_stream_on(
            "native_only",
            "rtsp://127.0.0.1:8554/watchtower-test",
            MediaEngine::Retina,
        )
        .await
        .unwrap();
    media.test_stream("native_only").await.unwrap();
    assert!(media
        .ensure_stream_on(
            "unavailable",
            "rtsp://127.0.0.1:8554/watchtower-test",
            MediaEngine::Go2rtc
        )
        .await
        .is_err());
    media.test_stream("native_only").await.unwrap();
    media.shutdown().await;
}

#[tokio::test]
#[ignore = "requires synthetic RTSP fixture and bundled engines"]
async fn native_and_mediamtx_probe_switch_and_recovery() {
    let go = PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR").unwrap());
    let mtx = PathBuf::from(std::env::var("WATCHTOWER_TEST_MEDIAMTX").unwrap());
    let media = Media::new(&go, &mtx);
    for engine in [
        MediaEngine::Retina,
        MediaEngine::MediaMtx,
        MediaEngine::Go2rtc,
        MediaEngine::Retina,
    ] {
        media
            .ensure_stream_on("switch", "rtsp://127.0.0.1:8554/watchtower-test", engine)
            .await
            .unwrap();
        media
            .test_stream("switch")
            .await
            .unwrap_or_else(|error| panic!("{engine:?} probe failed: {error}"));
        assert_eq!(media.playback("switch").engine, engine);
        if engine == MediaEngine::MediaMtx {
            assert!(media
                .received_bytes("switch")
                .await
                .unwrap()
                .is_some_and(|bytes| bytes > 0));
        }
        media
            .ensure_stream_on("duplicate", "rtsp://127.0.0.1:8554/watchtower-test", engine)
            .await
            .unwrap();
        assert_eq!(
            media.playback("switch").hls_url,
            media.playback("duplicate").hls_url
        );
        assert_eq!(
            media.playback("switch").whep_url,
            media.playback("duplicate").whep_url
        );
        media.remove_stream("duplicate").await.unwrap();
        media.test_stream("switch").await.unwrap();
        assert!(!serde_json::to_string(&media.playback("switch"))
            .unwrap()
            .contains("watchtower-test"));
    }
    media
        .ensure_stream_on(
            "replay",
            "rtsp://127.0.0.1:8554/watchtower-sub",
            MediaEngine::MediaMtx,
        )
        .await
        .unwrap();
    media.simulate_crash().await;
    media
        .ensure_stream_on(
            "switch",
            "rtsp://127.0.0.1:8554/watchtower-test",
            MediaEngine::Retina,
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    media.test_stream("replay").await.unwrap();
    media.remove_stream("switch").await.unwrap();
    assert!(media.test_stream("switch").await.is_err());
    media.shutdown().await;
}
