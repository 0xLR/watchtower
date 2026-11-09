//! Read-only acceptance probe: pass explicitly selected interface names as arguments.
use watchtower_core::Discoverer;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selected: Vec<String> = std::env::args().skip(1).collect();
    if selected.is_empty() {
        return Err("Pass one or more explicitly selected interface names".into());
    }
    let started = std::time::Instant::now();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(55),
        watchtower_discovery::LocalNetwork.discover(&selected, false),
    )
    .await?
    .map_err(std::io::Error::other)?;
    let models: Vec<_> = result
        .devices
        .iter()
        .filter_map(|device| device.model.as_ref())
        .collect();
    println!(
        "devices={} models={models:?} warnings={} elapsed_ms={}",
        result.devices.len(),
        result.warnings.len(),
        started.elapsed().as_millis()
    );
    Ok(())
}
