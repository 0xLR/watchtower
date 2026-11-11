use watchtower_core::{parse_address, redact, CameraInput};
fn input(address: &str) -> CameraInput {
    CameraInput {
        media_engine: None,
        id: None,
        name: "Camera".into(),
        address: address.into(),
        username: "".into(),
        password: None,
        path_main: None,
        path_sub: None,
        source_kind: None,
    }
}
#[test]
fn tapo_defaults_and_url_credentials() {
    let parsed = parse_address(&input("192.168.1.30")).expect("IP");
    assert_eq!(parsed.host, "192.168.1.30");
    assert_eq!(parsed.path_main, "/stream1");
    assert_eq!(parsed.path_sub.as_deref(), Some("/stream2"));
    assert!(parsed.is_local);
    let parsed = parse_address(&input("rtsp://user:p%40ss@192.168.1.30:554/live")).expect("URL");
    assert_eq!(parsed.password.as_deref(), Some("p@ss"));
    assert_eq!(parsed.username, "user");
    assert!(!parsed.path_main.contains("p@ss"));
}
#[test]
fn reject_unsafe_urls_and_preserve_remote_hosts() {
    for address in [
        "http://example.com",
        "rtsp://host/live?token=secret",
        "rtsp://host/live#secret",
        "rtsp://host:0/live",
        "",
        "rtsp://host/live\nsecret",
    ] {
        assert!(
            parse_address(&input(address)).is_err(),
            "accepted unsafe address"
        );
    }
    assert!(
        !parse_address(&input("camera.example.com"))
            .expect("host")
            .is_local
    );
    assert!(
        !parse_address(&input("203.0.113.2"))
            .expect("remote")
            .is_local
    );
    assert!(parse_address(&input("[fd00::1]")).expect("IPv6").is_local);
}
#[test]
fn redact_all_url_material_in_diagnostics() {
    let text = "failed rtsp://user:p%40ss@host/live?token=abc and http://host/secret";
    let clean = redact(text);
    for secret in ["user", "p%40ss", "host", "token", "abc", "/secret"] {
        assert!(!clean.contains(secret));
    }
}

#[test]
fn credential_percent_sequences_survive_url_reconstruction() {
    let mut value = input("rtsp://viewer:p%2541%40ss@192.168.1.30/live");
    let parsed = parse_address(&value).unwrap();
    let password = parsed.password.clone().unwrap();
    let camera = parsed.into_camera("fixture".into(), "Fixture".into(), 1, 0);
    let source = camera.source_url(Some(&password), true).unwrap();
    let source = url::Url::parse(&source).unwrap();
    assert_eq!(
        percent_encoding::percent_decode_str(source.password().unwrap())
            .decode_utf8()
            .unwrap(),
        "p%41@ss"
    );
    value.username = "viewer%41".into();
    value.password = Some("pass%20word".into());
    let parsed = parse_address(&value).unwrap();
    let password = parsed.password.clone().unwrap();
    let camera = parsed.into_camera("fixture".into(), "Fixture".into(), 1, 0);
    let source = url::Url::parse(&camera.source_url(Some(&password), true).unwrap()).unwrap();
    assert_eq!(
        percent_encoding::percent_decode_str(source.username())
            .decode_utf8()
            .unwrap(),
        "viewer%41"
    );
    assert_eq!(
        percent_encoding::percent_decode_str(source.password().unwrap())
            .decode_utf8()
            .unwrap(),
        "pass%20word"
    );
}

#[test]
fn full_url_tests_only_the_supplied_stream_unless_substream_is_explicit() {
    let mut value = input("rtsp://192.168.1.30/stream1");
    assert!(parse_address(&value).unwrap().path_sub.is_none());
    value.path_sub = Some("/stream2".into());
    assert_eq!(
        parse_address(&value).unwrap().path_sub.as_deref(),
        Some("/stream2")
    );
}
