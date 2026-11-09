use watchtower_discovery::{parse_neighbors, parse_probe_matches, sweep_hosts};
#[test]
fn discovery_uses_endpoint_identity_and_sender_address() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing"><s:Body><d:ProbeMatches><d:ProbeMatch><a:EndpointReference><a:Address>urn:uuid:camera-1</a:Address></a:EndpointReference><d:Scopes>onvif://www.onvif.org/name/Front%20Door onvif://www.onvif.org/hardware/Tapo</d:Scopes><d:XAddrs>http://192.168.1.30:2020/onvif/device_service</d:XAddrs></d:ProbeMatch></d:ProbeMatches></s:Body></s:Envelope>"#;
    let devices = parse_probe_matches(xml, "192.168.1.30");
    assert_eq!(devices.len(), 1);
    assert_eq!(
        devices[0].endpoint_uuid.as_deref(),
        Some("urn:uuid:camera-1")
    );
    assert_eq!(devices[0].name.as_deref(), Some("Front Door"));
    assert_eq!(devices[0].onvif_port, Some(2020));
    assert!(parse_probe_matches(xml, "192.168.1.31").is_empty());
}
#[test]
fn bounded_sweep_and_neighbor_parsing() {
    let hosts = sweep_hosts("192.168.8.20", 16).unwrap();
    assert_eq!(hosts.len(), 254);
    assert!(hosts.contains(&"192.168.8.30".to_string()));
    assert!(!hosts.contains(&"192.168.9.30".to_string()));
    let devices=parse_neighbors(" 192.168.1.30 aa-bb-cc-dd-ee-ff dynamic\n192.168.1.31 dev eth0 lladdr 01:02:03:04:05:06 REACHABLE\n? (192.168.1.32) at aa:bb:cc:11:22:33 on en0");
    assert_eq!(devices.len(), 2); // multicast MAC is rejected
    assert_eq!(devices[0].mac.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
}
