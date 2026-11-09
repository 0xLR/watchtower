//! Identity matching and recovery policy. Network and persistence sit behind traits.
use std::{
    collections::HashSet,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use watchtower_core::{Camera, CameraIdentity, Discoverer, NeighborTable, ObservedDevice};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Match(String),
    NoMatch,
    Ambiguous,
    Untracked,
}
fn canonical(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}
fn same(a: &Option<String>, b: &Option<String>, mac: bool) -> Option<bool> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if mac {
            match (
                watchtower_discovery::normalize_mac(a),
                watchtower_discovery::normalize_mac(b),
            ) {
                (Some(a), Some(b)) => a == b,
                _ => return None,
            }
        } else {
            canonical(a) == canonical(b)
        }),
        _ => None,
    }
}
pub fn match_identity(identity: &CameraIdentity, observed: &[ObservedDevice]) -> Decision {
    if identity.mac.is_none() && identity.onvif_endpoint_uuid.is_none() && identity.serial.is_none()
    {
        return Decision::Untracked;
    }
    let mut hosts = HashSet::new();
    for device in observed {
        if !watchtower_core::is_private(&device.host) {
            continue;
        }
        let evidence = [
            same(&identity.mac, &device.mac, true),
            same(&identity.onvif_endpoint_uuid, &device.endpoint_uuid, false),
            same(&identity.serial, &device.serial, false),
        ];
        // Any contradiction vetoes a weaker match. Address alone is never evidence.
        if evidence.contains(&Some(false)) || !evidence.contains(&Some(true)) {
            continue;
        }
        hosts.insert(device.host.clone());
    }
    match hosts.len() {
        0 => Decision::NoMatch,
        1 => Decision::Match(hosts.into_iter().next().unwrap_or_default()),
        _ => Decision::Ambiguous,
    }
}
#[derive(Default)]
pub struct RetryState {
    failures: u32,
    first_failure: Option<Instant>,
    next: Option<Instant>,
    attempts: u32,
}
impl RetryState {
    pub fn healthy(&mut self) {
        *self = Self::default();
    }
    pub fn failed(&mut self, now: Instant) -> bool {
        self.failures = self.failures.saturating_add(1);
        let first = *self.first_failure.get_or_insert(now);
        self.failures >= 3
            && now.saturating_duration_since(first) >= Duration::from_secs(15)
            && self.due(now)
    }
    pub fn due(&self, now: Instant) -> bool {
        self.next.is_none_or(|next| now >= next)
    }
    pub fn defer(&mut self, now: Instant) {
        self.attempts = self.attempts.saturating_add(1);
        let seconds = (5u64.saturating_mul(1u64 << self.attempts.min(6))).min(300);
        self.next = Some(now + Duration::from_secs(seconds));
    }
}
#[async_trait::async_trait]
pub trait RecoveryNetwork: NeighborTable + Discoverer {
    async fn probe(&self, host: &str, port: u16);
}
#[async_trait::async_trait]
impl RecoveryNetwork for watchtower_discovery::LocalNetwork {
    async fn probe(&self, host: &str, port: u16) {
        watchtower_discovery::probe(host, port).await;
    }
}
#[async_trait::async_trait]
pub trait RecoverySink: Send + Sync {
    async fn commit(
        &self,
        camera: &Camera,
        identity: &CameraIdentity,
        old_host: &str,
    ) -> Result<(), String>;
}

pub async fn resolve(
    network: &impl RecoveryNetwork,
    identity: &CameraIdentity,
    last_host: &str,
    port: u16,
    interfaces: &[String],
) -> Result<Decision, String> {
    if !watchtower_core::is_private(last_host) || interfaces.is_empty() {
        return Ok(Decision::Untracked);
    }
    if identity.mac.is_none() && identity.onvif_endpoint_uuid.is_none() && identity.serial.is_none()
    {
        return Ok(Decision::Untracked);
    }
    network.probe(last_host, port).await;
    let mut neighbors = network.neighbors().await.unwrap_or_default();
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for device in &neighbors {
        if let Some(mac) = &device.mac {
            *counts.entry(mac.clone()).or_default() += 1;
        }
    }
    for device in &mut neighbors {
        if device
            .mac
            .as_ref()
            .is_some_and(|mac| counts.get(mac).is_some_and(|count| *count > 1))
        {
            device.mac = None;
        }
    }
    // First inspect the old host only. Never attach to an unrelated neighbor.
    let old: Vec<_> = neighbors
        .iter()
        .filter(|d| d.host == last_host)
        .cloned()
        .collect();
    if matches!(match_identity(identity, &old), Decision::Match(_)) {
        return Ok(Decision::Match(last_host.into()));
    }
    let discovered = network.discover(interfaces, false).await?;
    let decision = match_identity(identity, &discovered.devices);
    if !matches!(decision, Decision::NoMatch) {
        return Ok(decision);
    }
    let swept = network.discover(interfaces, true).await?;
    Ok(match_identity(identity, &swept.devices))
}
pub async fn recover_camera(
    camera: &Camera,
    identity: &CameraIdentity,
    network: &impl RecoveryNetwork,
    sink: &impl RecoverySink,
    interfaces: &[String],
) -> Result<Decision, String> {
    let decision = resolve(network, identity, &camera.host, camera.port, interfaces).await?;
    if let Decision::Match(host) = &decision {
        if host != &camera.host {
            let mut changed = camera.clone();
            changed.host = host.clone();
            changed.is_local = watchtower_core::is_private(host);
            let mut identity = identity.clone();
            identity.last_seen_host = Some(host.clone());
            identity.last_seen_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
            );
            sink.commit(&changed, &identity, &camera.host).await?;
        }
    }
    Ok(decision)
}
