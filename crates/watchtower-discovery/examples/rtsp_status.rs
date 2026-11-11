//! Read-only RTSP status probe. Optional Basic authorization arrives only on stdin.
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use watchtower_core::Discoverer;

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let mut selected: Vec<String> = std::env::args().skip(1).collect();
    let authenticated = selected.iter().any(|arg| arg == "--authorization-stdin");
    let context_only = selected.iter().any(|arg| arg == "--connection-context");
    selected.retain(|arg| arg != "--authorization-stdin");
    selected.retain(|arg| arg != "--connection-context");
    let mut authorization = String::new();
    if authenticated {
        std::io::stdin()
            .read_line(&mut authorization)
            .map_err(|_| "No authorization input")?;
        authorization = authorization.trim().to_owned();
        if !authorization.starts_with("Basic ") || authorization.contains(['\r', '\n']) {
            return Err("Invalid authorization input");
        }
    }
    if selected.is_empty() {
        return Err("Pass explicitly selected interface names");
    }
    let result = timeout(
        Duration::from_secs(15),
        watchtower_discovery::LocalNetwork.discover(&selected, false),
    )
    .await
    .map_err(|_| "Discovery timed out")?
    .map_err(|_| "Discovery failed")?;
    let cameras: Vec<_> = result
        .devices
        .iter()
        .filter(|device| {
            device
                .model
                .as_deref()
                .is_some_and(|model| model.contains("C530WS"))
        })
        .collect();
    if cameras.len() != 1 {
        return Err("Expected exactly one discovered C530WS");
    }
    let camera = cameras[0];
    if context_only {
        let host = camera
            .host
            .parse::<std::net::IpAddr>()
            .map_err(|_| "Invalid discovered address")?;
        println!("{{\"host\":\"{host}\",\"port\":{}}}", camera.port);
        return Ok(());
    }
    let mut socket = timeout(
        Duration::from_secs(3),
        TcpStream::connect((camera.host.as_str(), camera.port)),
    )
    .await
    .map_err(|_| "Connection timed out")?
    .map_err(|_| "Connection failed")?;
    let header = if authenticated {
        format!("Authorization: {authorization}\r\n")
    } else {
        String::new()
    };
    let request = format!("DESCRIBE rtsp://{}:{}/stream1 RTSP/1.0\r\nCSeq: 1\r\nAccept: application/sdp\r\nUser-Agent: go2rtc/1.9.15\r\n{header}\r\n", camera.host, camera.port);
    socket
        .write_all(request.as_bytes())
        .await
        .map_err(|_| "Request failed")?;
    let mut bytes = Vec::new();
    timeout(Duration::from_secs(5), async {
        while bytes.len() < 16_384 {
            let mut byte = [0];
            if socket
                .read(&mut byte)
                .await
                .map_err(|_| "Response failed")?
                == 0
            {
                break;
            }
            bytes.push(byte[0]);
            if bytes.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        Ok::<_, &'static str>(())
    })
    .await
    .map_err(|_| "Response timed out")??;
    let response = String::from_utf8_lossy(&bytes);
    let status = response.lines().next().ok_or("No status")?;
    let code: u16 = status
        .split_whitespace()
        .nth(1)
        .ok_or("Invalid status")?
        .parse()
        .map_err(|_| "Invalid code")?;
    if !(100..600).contains(&code) {
        return Err("Invalid code");
    }
    let locked = status.to_ascii_lowercase().contains("locked");
    let auth_headers: Vec<_> = response
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("www-authenticate")
                .then_some(value.trim())
        })
        .collect();
    let basic_challenges = auth_headers
        .iter()
        .filter(|value| value.starts_with("Basic"))
        .count();
    let digest_challenges = auth_headers
        .iter()
        .filter(|value| value.starts_with("Digest"))
        .count();
    let mut engine_accepts_challenge = false;
    let authentication = response
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            if !key.eq_ignore_ascii_case("www-authenticate") {
                return None;
            }
            let value = value.trim();
            engine_accepts_challenge =
                value.len() >= 6 && (value.starts_with("Basic ") || value.starts_with("Digest"));
            Some(if value.starts_with("Digest") {
                "Digest"
            } else if value.starts_with("Basic") {
                "Basic"
            } else {
                "Other"
            })
        })
        .unwrap_or("None");
    println!(
        "model=C530WS path=/stream1 credentials_sent={authenticated} status={code} locked={locked} authentication={authentication} engine_accepts_challenge={engine_accepts_challenge} basic_challenges={basic_challenges} digest_challenges={digest_challenges}"
    );
    Ok(())
}
