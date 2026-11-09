use watchtower_core::{parse_address, redact, CameraIdentity, CameraInput, Settings};
use watchtower_secrets::{MemorySecrets, SecretStore};
use watchtower_store::Store;
#[tokio::test]
async fn persistence_identity_order_and_secret_boundary() {
    let directory =
        std::env::temp_dir().join(format!("watchtower-store-test-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("test.sqlite");
    let store = Store::open(&path).await.unwrap();
    let secret = "unique-watchtower-secret-p@ss";
    let input = CameraInput {
        id: None,
        name: "Front".into(),
        address: "rtsp://viewer:unique-watchtower-secret-p%40ss@192.168.1.30/live".into(),
        username: "".into(),
        password: None,
        path_main: None,
        path_sub: None,
        source_kind: None,
    };
    let parsed = parse_address(&input).unwrap();
    let secrets = MemorySecrets::default();
    secrets
        .set("camera-a", parsed.password.as_deref().unwrap())
        .unwrap();
    let camera = parsed.into_camera("camera-a".into(), "Front".into(), 1, 0);
    let identity = CameraIdentity {
        camera_id: camera.id.clone(),
        mac: Some("aa:bb:cc:dd:ee:ff".into()),
        ..Default::default()
    };
    store.save(&camera, &identity).await.unwrap();
    let mut second = camera.clone();
    second.id = "camera-b".into();
    second.name = "Back".into();
    second.sort_order = 1;
    store
        .save(
            &second,
            &CameraIdentity {
                camera_id: second.id.clone(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(store
        .reorder(&[camera.id.clone(), camera.id.clone()])
        .await
        .is_err());
    store
        .reorder(&[second.id.clone(), camera.id.clone()])
        .await
        .unwrap();
    let settings = Settings {
        background_sweep: true,
        ..Default::default()
    };
    store.save_settings(&settings).await.unwrap();
    store.close().await;
    let reopened = Store::open(&path).await.unwrap();
    let cameras = reopened.cameras().await.unwrap();
    assert_eq!(cameras[0].id, second.id);
    assert_eq!(cameras[1].host, "192.168.1.30");
    assert_eq!(
        reopened.identity(&camera.id).await.unwrap().mac,
        identity.mac
    );
    assert!(reopened.settings().await.unwrap().background_sweep);
    assert_eq!(secrets.get(&camera.id).unwrap().as_deref(), Some(secret));
    reopened.delete(&camera.id).await.unwrap();
    assert!(reopened.identity(&camera.id).await.unwrap().mac.is_none());
    reopened.close().await;
    let bytes = std::fs::read(&path).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    for forbidden in [secret, "p%40ss", "rtsp://"] {
        assert!(!text.contains(forbidden), "secret leaked into DB");
    }
    let log = redact(&format!("camera connection failed {}", input.address));
    assert!(!log.contains(secret));
    assert!(!log.contains("rtsp://"));
    assert!(!log.contains("p%40ss"));
    // Delete only this test's explicitly created file and empty directory.
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&directory).unwrap();
}
