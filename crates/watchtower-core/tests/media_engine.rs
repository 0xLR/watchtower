use watchtower_core::{MediaEngine, Settings};
#[test]
fn legacy_settings_keep_go2rtc_and_new_settings_prefer_retina() {
    let old = r#"{"layout":"auto","default_substream":true,"background_sweep":false,"selected_interfaces":[]}"#;
    assert_eq!(
        serde_json::from_str::<Settings>(old).unwrap().media_engine,
        MediaEngine::Go2rtc
    );
    assert_eq!(Settings::default().media_engine, MediaEngine::Retina);
    assert_eq!(
        serde_json::to_string(&MediaEngine::MediaMtx).unwrap(),
        "\"mediamtx\""
    );
    assert!(serde_json::from_str::<MediaEngine>("\"unknown\"").is_err());
}
