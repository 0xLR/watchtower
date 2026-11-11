use std::path::PathBuf;
use watchtower_core::MediaEngine;
use watchtower_media::Media;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let binary = PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR")?);
    let mtx = PathBuf::from(std::env::var("WATCHTOWER_TEST_MEDIAMTX").unwrap_or_default());
    let media = Media::new(&binary, &mtx);
    let engine = match std::env::var("WATCHTOWER_TEST_ENGINE").as_deref() {
        Ok("retina") => MediaEngine::Retina,
        Ok("mediamtx") => MediaEngine::MediaMtx,
        _ => MediaEngine::Go2rtc,
    };
    let saved: Option<Vec<String>> = std::env::var_os("WATCHTOWER_SAVED_SOURCES")
        .map(|path| {
            std::fs::read(path)
                .and_then(|bytes| serde_json::from_slice(&bytes).map_err(std::io::Error::other))
        })
        .transpose()?;
    for (id, path) in [
        ("fixture_main", "watchtower-test"),
        ("fixture_sub", "watchtower-sub"),
    ] {
        let source = saved
            .as_ref()
            .and_then(|sources| sources.first())
            .cloned()
            .unwrap_or_else(|| format!("rtsp://127.0.0.1:8554/{path}"));
        if saved.is_some() {
            let parsed = url::Url::parse(&source)?;
            if !parsed.username().is_empty() || parsed.password().is_some() {
                return Err("Saved test stream must be credential-free".into());
            }
        }
        media
            .ensure_stream_on(id, &source, engine)
            .await
            .map_err(std::io::Error::other)?;
        if saved.is_some() {
            media.test_stream(id).await?;
            println!("Saved stream {engine:?} probe passed");
        }
    }
    let path = PathBuf::from(std::env::var("WATCHTOWER_TEST_PLAYBACK")?);
    std::fs::write(
        &path,
        serde_json::to_vec(
            &serde_json::json!({"main":media.playback("fixture_main"),"sub":media.playback("fixture_sub")}),
        )?,
    )?;
    println!("Synthetic camera gateway ready");
    tokio::signal::ctrl_c().await?;
    media.shutdown().await;
    let _ = std::fs::remove_file(path);
    Ok(())
}
