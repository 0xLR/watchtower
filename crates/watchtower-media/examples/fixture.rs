use std::path::PathBuf;
use watchtower_core::StreamBackend;
use watchtower_media::Media;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let binary = PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR")?);
    let media = Media::launch(&binary).await?;
    for (id, path) in [
        ("fixture_main", "watchtower-test"),
        ("fixture_sub", "watchtower-sub"),
    ] {
        media
            .add_stream(id, &format!("rtsp://127.0.0.1:8554/{path}"))
            .await
            .map_err(std::io::Error::other)?;
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
