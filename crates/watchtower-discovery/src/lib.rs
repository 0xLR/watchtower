//! Bounded, user-selected-interface discovery. No raw sockets or elevation.
mod mdns;
use futures_util::{stream, StreamExt};
use network_interface::{Addr, NetworkInterfaceConfig};
use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    time::Duration,
};
use tokio::{
    net::{TcpStream, UdpSocket},
    time::{timeout, Instant},
};
use watchtower_core::{Discoverer, NeighborTable, NetworkInterface, ObservedDevice, ScanResult};

pub struct LocalNetwork;
pub fn interfaces() -> Result<Vec<NetworkInterface>, String> {
    let raw = network_interface::NetworkInterface::show()
        .map_err(|_| "Network interfaces could not be listed")?;
    let mut result = Vec::new();
    for interface in raw {
        if interface.internal {
            continue;
        }
        let lower = interface.name.to_ascii_lowercase();
        let virtual_hint = [
            "docker",
            "veth",
            "vmnet",
            "virtual",
            "vpn",
            "tun",
            "tap",
            "bridge",
            "wsl",
            "vethernet",
        ]
        .iter()
        .any(|word| lower.contains(word));
        for address in interface.addr {
            let Addr::V4(address) = address else {
                continue;
            };
            if !address.ip.is_private() && !address.ip.is_link_local() {
                continue;
            }
            result.push(NetworkInterface {
                id: interface.name.clone(),
                name: interface.name.clone(),
                address: address.ip.to_string(),
                prefix: address
                    .netmask
                    .map(|mask| u32::from(mask).count_ones() as u8)
                    .unwrap_or(24),
                virtual_hint,
            });
        }
    }
    Ok(result)
}
pub fn normalize_mac(value: &str) -> Option<String> {
    let value = value.replace('-', ":").to_lowercase();
    let pieces: Vec<_> = value.split(':').collect();
    if pieces.len() != 6 {
        return None;
    }
    let bytes: Option<Vec<_>> = pieces
        .iter()
        .map(|p| u8::from_str_radix(p, 16).ok())
        .collect();
    let bytes = bytes?;
    if bytes[0] & 1 != 0 || bytes.iter().all(|b| *b == 0) {
        return None;
    }
    Some(
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
}
pub fn parse_neighbors(text: &str) -> Vec<ObservedDevice> {
    let mut devices = Vec::new();
    for line in text.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        let ip = parts.iter().find_map(|part| {
            part.trim_matches(['(', ')'])
                .parse::<std::net::IpAddr>()
                .ok()
        });
        let mac = parts.iter().find_map(|part| normalize_mac(part));
        if let (Some(ip), Some(mac)) = (ip, mac) {
            if watchtower_core::is_private(&ip.to_string()) {
                devices.push(ObservedDevice {
                    host: ip.to_string(),
                    port: 554,
                    mac: Some(mac),
                    ..Default::default()
                });
            }
        }
    }
    devices
}
#[async_trait::async_trait]
impl NeighborTable for LocalNetwork {
    async fn neighbors(&self) -> Result<Vec<ObservedDevice>, String> {
        #[cfg(windows)]
        let (program, args) = ("arp.exe", vec!["-a"]);
        #[cfg(target_os = "macos")]
        let (program, args) = ("/usr/sbin/arp", vec!["-an"]);
        #[cfg(target_os = "linux")]
        let (program, args) = ("ip", vec!["neigh", "show"]);
        let mut command = tokio::process::Command::new(program);
        command.args(args).kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let output = timeout(Duration::from_secs(3), command.output())
            .await
            .map_err(|_| "Neighbor table timed out")?
            .map_err(|_| "Neighbor table unavailable")?;
        if !output.status.success() {
            return Err("Neighbor table unavailable".into());
        }
        let len = output.stdout.len().min(2 * 1024 * 1024);
        let mut devices = parse_neighbors(&String::from_utf8_lossy(&output.stdout[..len]));
        for device in &mut devices {
            if device.mac.as_ref().is_some_and(|mac| {
                u8::from_str_radix(&mac[..2], 16).is_ok_and(|byte| byte & 2 != 0)
            }) {
                device.mac = None;
            }
        }
        Ok(devices)
    }
}
pub fn sweep_hosts(address: &str, prefix: u8) -> Result<Vec<String>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "IPv4 interface required")?;
    if !ip.is_private() && !ip.is_link_local() {
        return Err("Only local subnets can be scanned".into());
    }
    let prefix = prefix.clamp(24, 32);
    let mask = u32::MAX.checked_shl((32 - prefix) as u32).unwrap_or(0);
    let network = u32::from(ip) & mask;
    let count = 1u32 << (32 - prefix);
    Ok((1..count.saturating_sub(1))
        .map(|n| Ipv4Addr::from(network + n).to_string())
        .collect())
}
pub fn belongs(host: &str, interface: &NetworkInterface) -> bool {
    let (Ok(host), Ok(local)) = (
        host.parse::<Ipv4Addr>(),
        interface.address.parse::<Ipv4Addr>(),
    ) else {
        return false;
    };
    let mask = u32::MAX
        .checked_shl((32 - interface.prefix.min(32)) as u32)
        .unwrap_or(0);
    u32::from(host) & mask == u32::from(local) & mask
}
pub fn parse_probe_matches(xml: &str, sender: &str) -> Vec<ObservedDevice> {
    if xml.len() > 65536 {
        return Vec::new();
    }
    let Ok(document) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for node in document
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "ProbeMatch")
    {
        let text = |tag: &str| {
            node.descendants()
                .find(|n| n.is_element() && n.tag_name().name() == tag)
                .and_then(|n| n.text())
        };
        let Some(url) = text("XAddrs").and_then(|s| {
            s.split_whitespace().find_map(|s| {
                url::Url::parse(s).ok().filter(|url| {
                    url.scheme() == "http"
                        && url.host_str() == Some(sender)
                        && url.username().is_empty()
                        && url.password().is_none()
                })
            })
        }) else {
            continue;
        };
        let scopes = text("Scopes").unwrap_or("");
        let scope = |kind: &str| {
            scopes.split_whitespace().find_map(|s| {
                s.strip_prefix(&format!("onvif://www.onvif.org/{kind}/"))
                    .and_then(|s| {
                        percent_encoding::percent_decode_str(s)
                            .decode_utf8()
                            .ok()
                            .map(|v| {
                                v.chars()
                                    .filter(|c| !c.is_control())
                                    .take(120)
                                    .collect::<String>()
                            })
                    })
            })
        };
        let endpoint_uuid = text("Address")
            .filter(|s| s.len() <= 256 && s.to_ascii_lowercase().starts_with("urn:uuid:"))
            .map(|s| s.to_lowercase());
        result.push(ObservedDevice {
            host: sender.into(),
            port: 554,
            endpoint_uuid,
            name: scope("name"),
            model: scope("hardware"),
            onvif_port: Some(url.port_or_known_default().unwrap_or(80)),
            ..Default::default()
        });
    }
    result
}
async fn ws_discover(interface: &NetworkInterface) -> Result<Vec<ObservedDevice>, String> {
    let socket = UdpSocket::bind((interface.address.as_str(), 0))
        .await
        .map_err(|_| "Discovery interface could not be bound")?;
    socket
        .set_multicast_ttl_v4(1)
        .map_err(|_| "Multicast unavailable")?;
    let message = format!(
        r#"<?xml version="1.0"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:dn="http://www.onvif.org/ver10/network/wsdl"><s:Header><a:MessageID>uuid:{}</a:MessageID><a:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</a:To><a:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</a:Action></s:Header><s:Body><d:Probe><d:Types>dn:NetworkVideoTransmitter</d:Types></d:Probe></s:Body></s:Envelope>"#,
        uuid::Uuid::new_v4()
    );
    for _ in 0..2 {
        socket
            .send_to(message.as_bytes(), "239.255.255.250:3702")
            .await
            .map_err(|_| "Multicast discovery could not send")?;
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut buffer = [0u8; 65536];
    let mut devices = Vec::new();
    while Instant::now() < deadline {
        let Ok(Ok((length, sender))) = timeout(
            deadline.saturating_duration_since(Instant::now()),
            socket.recv_from(&mut buffer),
        )
        .await
        else {
            break;
        };
        if belongs(&sender.ip().to_string(), interface) {
            if let Ok(xml) = std::str::from_utf8(&buffer[..length]) {
                devices.extend(parse_probe_matches(xml, &sender.ip().to_string()));
            }
        }
        if devices.len() > 512 {
            break;
        }
    }
    Ok(devices)
}
pub async fn probe(host: &str, port: u16) -> bool {
    timeout(Duration::from_millis(450), TcpStream::connect((host, port)))
        .await
        .is_ok_and(|r| r.is_ok())
}
#[async_trait::async_trait]
impl Discoverer for LocalNetwork {
    async fn discover(&self, selected: &[String], sweep: bool) -> Result<ScanResult, String> {
        let interfaces: Vec<_> = interfaces()?
            .into_iter()
            .filter(|i| selected.contains(&i.id))
            .take(4)
            .collect();
        if interfaces.is_empty() {
            return Err("Select a local interface before scanning".into());
        }
        let mut devices = HashMap::<String, ObservedDevice>::new();
        let mut warnings = Vec::new();
        for interface in &interfaces {
            match ws_discover(interface).await {
                Ok(observed) => {
                    for device in observed {
                        devices.insert(device.host.clone(), device);
                    }
                }
                Err(message) => warnings.push(message),
            }
            for device in mdns::discover(interface).await {
                devices.entry(device.host.clone()).or_insert(device);
            }
            if sweep {
                if interface.prefix < 24 {
                    warnings.push(format!(
                        "{}: /{} network; sweep limited to the interface's /24 (254 hosts).",
                        interface.name, interface.prefix
                    ));
                }
                let hosts = sweep_hosts(&interface.address, interface.prefix)?;
                let observed: Vec<_> = stream::iter(hosts)
                    .map(|host| async move {
                        let mut found = None;
                        for port in [554, 2020] {
                            if probe(&host, port).await {
                                found = Some(port);
                                break;
                            }
                        }
                        found.map(|port| ObservedDevice {
                            host,
                            port: if port == 2020 { 554 } else { port },
                            onvif_port: if port == 2020 { Some(2020) } else { None },
                            ..Default::default()
                        })
                    })
                    .buffer_unordered(48)
                    .filter_map(|device| async move { device })
                    .collect()
                    .await;
                for device in observed {
                    devices.entry(device.host.clone()).or_insert(device);
                }
            }
        }
        match self.neighbors().await {
            Ok(neighbors) => {
                for neighbor in neighbors {
                    if let Some(device) = devices.get_mut(&neighbor.host) {
                        device.mac = neighbor.mac;
                    }
                }
            }
            Err(message) => warnings.push(message),
        }
        let mut seen_macs = HashMap::<String, HashSet<String>>::new();
        for device in devices.values() {
            if let Some(mac) = &device.mac {
                seen_macs
                    .entry(mac.clone())
                    .or_default()
                    .insert(device.host.clone());
            }
        }
        for device in devices.values_mut() {
            if device
                .mac
                .as_ref()
                .is_some_and(|m| seen_macs.get(m).is_some_and(|hosts| hosts.len() > 1))
            {
                device.mac = None;
            }
        }
        let mut devices: Vec<_> = devices.into_values().collect();
        devices.sort_by(|a, b| a.host.cmp(&b.host));
        Ok(ScanResult { devices, warnings })
    }
}
