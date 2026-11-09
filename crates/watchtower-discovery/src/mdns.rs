//! Minimal bounded DNS-SD support for _rtsp._tcp.local. No hostname resolution or HTTP.
use std::{collections::HashMap, net::Ipv4Addr, time::Duration};
use watchtower_core::{NetworkInterface, ObservedDevice};
fn number(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
}
fn name(bytes: &[u8], cursor: &mut usize, depth: u8) -> Option<String> {
    if depth > 12 {
        return None;
    }
    let mut labels = Vec::new();
    loop {
        let length = *bytes.get(*cursor)?;
        *cursor += 1;
        if length == 0 {
            break;
        }
        if length & 0xc0 == 0xc0 {
            let low = *bytes.get(*cursor)?;
            *cursor += 1;
            let mut target = (((length & 0x3f) as usize) << 8) | low as usize;
            labels.push(name(bytes, &mut target, depth + 1)?);
            break;
        }
        if length > 63 {
            return None;
        }
        let end = cursor.checked_add(length as usize)?;
        labels.push(
            std::str::from_utf8(bytes.get(*cursor..end)?)
                .ok()?
                .to_string(),
        );
        *cursor = end;
        if labels.len() > 32 {
            return None;
        }
    }
    Some(labels.join(".").to_ascii_lowercase())
}
pub fn parse_mdns(bytes: &[u8]) -> Vec<ObservedDevice> {
    let Some(questions) = number(bytes, 4) else {
        return Vec::new();
    };
    let count = (number(bytes, 6).unwrap_or(0) as usize)
        + (number(bytes, 8).unwrap_or(0) as usize)
        + (number(bytes, 10).unwrap_or(0) as usize);
    if questions > 64 || count > 256 || bytes.len() > 65536 {
        return Vec::new();
    }
    let mut cursor = 12;
    for _ in 0..questions {
        if name(bytes, &mut cursor, 0).is_none() {
            return Vec::new();
        }
        cursor += 4;
    }
    let mut addresses = HashMap::new();
    let mut services = Vec::new();
    for _ in 0..count {
        let Some(owner) = name(bytes, &mut cursor, 0) else {
            break;
        };
        let (Some(kind), Some(length)) = (number(bytes, cursor), number(bytes, cursor + 8)) else {
            break;
        };
        cursor += 10;
        let Some(end) = cursor.checked_add(length as usize) else {
            break;
        };
        if end > bytes.len() {
            break;
        }
        if kind == 1 && length == 4 {
            addresses.insert(
                owner.clone(),
                Ipv4Addr::new(
                    bytes[cursor],
                    bytes[cursor + 1],
                    bytes[cursor + 2],
                    bytes[cursor + 3],
                )
                .to_string(),
            );
        }
        if kind == 33 && length >= 7 && owner.ends_with("._rtsp._tcp.local") {
            let port = number(bytes, cursor + 4).unwrap_or(554);
            let mut at = cursor + 6;
            if let Some(target) = name(bytes, &mut at, 0) {
                services.push((
                    owner.trim_end_matches("._rtsp._tcp.local").to_string(),
                    target,
                    port,
                ));
            }
        }
        cursor = end;
    }
    services
        .into_iter()
        .filter_map(|(name, target, port)| {
            addresses
                .get(&target)
                .filter(|host| watchtower_core::is_private(host))
                .map(|host| ObservedDevice {
                    host: host.clone(),
                    port,
                    name: Some(name),
                    ..Default::default()
                })
        })
        .collect()
}
pub async fn discover(interface: &NetworkInterface) -> Vec<ObservedDevice> {
    let Ok(socket) = tokio::net::UdpSocket::bind((interface.address.as_str(), 0)).await else {
        return Vec::new();
    };
    let _ = socket.set_multicast_ttl_v4(1);
    let mut query = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in ["_rtsp", "_tcp", "local"] {
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    query.extend_from_slice(&[0, 0, 12, 0x80, 1]);
    if socket.send_to(&query, "224.0.0.251:5353").await.is_err() {
        return Vec::new();
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut buffer = [0u8; 65536];
    let mut devices = Vec::new();
    while tokio::time::Instant::now() < deadline {
        let Ok(Ok((length, sender))) = tokio::time::timeout(
            deadline.saturating_duration_since(tokio::time::Instant::now()),
            socket.recv_from(&mut buffer),
        )
        .await
        else {
            break;
        };
        if crate::belongs(&sender.ip().to_string(), interface) {
            devices.extend(
                parse_mdns(&buffer[..length])
                    .into_iter()
                    .filter(|d| crate::belongs(&d.host, interface)),
            );
        }
        if devices.len() > 512 {
            break;
        }
    }
    devices
}
#[cfg(test)]
mod tests {
    use super::*;
    fn label(value: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        for part in value.split('.') {
            bytes.push(part.len() as u8);
            bytes.extend_from_slice(part.as_bytes());
        }
        bytes.push(0);
        bytes
    }
    #[test]
    fn pairs_rtsp_services_with_addresses_and_bounds_pointer_loops() {
        let mut bytes = vec![0, 0, 0x84, 0, 0, 0, 0, 2, 0, 0, 0, 0];
        bytes.extend(label("front._rtsp._tcp.local"));
        bytes.extend([0, 33, 0, 1, 0, 0, 0, 120]);
        let mut value = vec![0, 0, 0, 0, 2, 42];
        value.extend(label("front.local"));
        bytes.extend((value.len() as u16).to_be_bytes());
        bytes.extend(value);
        bytes.extend(label("front.local"));
        bytes.extend([0, 1, 0, 1, 0, 0, 0, 120, 0, 4, 192, 168, 1, 30]);
        let devices = parse_mdns(&bytes);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].port, 554);
        assert_eq!(devices[0].host, "192.168.1.30");
        let mut cursor = 0;
        assert!(name(&[0xc0, 0], &mut cursor, 0).is_none());
        assert!(parse_mdns(&[0; 4]).is_empty());
    }
}
