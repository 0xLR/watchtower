use std::time::{Duration, Instant};
use watchtower_core::{CameraIdentity, ObservedDevice};
use watchtower_netid::{match_identity, Decision, RetryState};
fn identity() -> CameraIdentity {
    CameraIdentity {
        mac: Some("aa:bb:cc:dd:ee:ff".into()),
        onvif_endpoint_uuid: Some("urn:uuid:front".into()),
        ..Default::default()
    }
}
fn device(host: &str, mac: Option<&str>, uuid: Option<&str>) -> ObservedDevice {
    ObservedDevice {
        host: host.into(),
        mac: mac.map(str::to_string),
        endpoint_uuid: uuid.map(str::to_string),
        ..Default::default()
    }
}
#[test]
fn follows_identity_across_swapped_ips() {
    let observed = vec![
        device(
            "192.168.1.20",
            Some("aa:bb:cc:11:22:33"),
            Some("urn:uuid:back"),
        ),
        device(
            "192.168.1.21",
            Some("AA-BB-CC-DD-EE-FF"),
            Some("URN:UUID:FRONT"),
        ),
    ];
    assert_eq!(
        match_identity(&identity(), &observed),
        Decision::Match("192.168.1.21".into())
    );
}
#[test]
fn falls_back_only_without_contradictory_identity() {
    assert_eq!(
        match_identity(
            &identity(),
            &[device("192.168.1.21", None, Some("urn:uuid:front"))]
        ),
        Decision::Match("192.168.1.21".into())
    );
    assert_eq!(
        match_identity(
            &identity(),
            &[device(
                "192.168.1.21",
                Some("aa:bb:cc:11:22:33"),
                Some("urn:uuid:front")
            )]
        ),
        Decision::NoMatch
    );
    assert_eq!(
        match_identity(&CameraIdentity::default(), &[]),
        Decision::Untracked
    );
}
#[test]
fn rejects_ambiguous_and_proxy_arp_results() {
    let observed = vec![
        device("192.168.1.20", Some("aa:bb:cc:dd:ee:ff"), None),
        device("192.168.1.21", Some("aa:bb:cc:dd:ee:ff"), None),
    ];
    assert_eq!(match_identity(&identity(), &observed), Decision::Ambiguous);
}
#[test]
fn serial_fallback_and_failure_backoff() {
    let stored = CameraIdentity {
        serial: Some("serial-1".into()),
        ..Default::default()
    };
    let observed = ObservedDevice {
        host: "192.168.1.21".into(),
        serial: Some("serial-1".into()),
        ..Default::default()
    };
    assert_eq!(
        match_identity(&stored, &[observed]),
        Decision::Match("192.168.1.21".into())
    );
    let start = Instant::now();
    let mut retry = RetryState::default();
    assert!(!retry.failed(start));
    assert!(!retry.failed(start + Duration::from_secs(5)));
    assert!(retry.failed(start + Duration::from_secs(15)));
    retry.defer(start);
    assert!(!retry.due(start));
    assert!(retry.due(start + Duration::from_secs(301)));
    retry.healthy();
    assert!(!retry.failed(start));
}
