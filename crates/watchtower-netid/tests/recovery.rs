use std::sync::{Arc, Mutex};
use watchtower_core::*;
use watchtower_netid::*;
use watchtower_store::Store;
struct FakeNetwork;
#[async_trait::async_trait]
impl NeighborTable for FakeNetwork {
    async fn neighbors(&self) -> Result<Vec<ObservedDevice>, String> {
        Ok(vec![ObservedDevice {
            host: "192.168.1.20".into(),
            mac: Some("aa:bb:cc:11:22:33".into()),
            ..Default::default()
        }])
    }
}
#[async_trait::async_trait]
impl Discoverer for FakeNetwork {
    async fn discover(&self, _: &[String], _: bool) -> Result<ScanResult, String> {
        Ok(ScanResult {
            devices: vec![ObservedDevice {
                host: "192.168.1.21".into(),
                mac: Some("aa:bb:cc:dd:ee:ff".into()),
                ..Default::default()
            }],
            warnings: vec![],
        })
    }
}
#[async_trait::async_trait]
impl RecoveryNetwork for FakeNetwork {
    async fn probe(&self, _: &str, _: u16) {}
}
struct Sink {
    store: Store,
    updates: Arc<Mutex<Vec<(String, String)>>>,
}
#[async_trait::async_trait]
impl RecoverySink for Sink {
    async fn commit(
        &self,
        camera: &Camera,
        identity: &CameraIdentity,
        old: &str,
    ) -> Result<(), String> {
        self.store
            .save(camera, identity)
            .await
            .map_err(|e| e.to_string())?;
        self.updates
            .lock()
            .unwrap()
            .push((old.into(), camera.host.clone()));
        Ok(())
    }
}
#[tokio::test]
async fn swapped_ip_updates_persistence_and_emits_one_change() {
    let store = Store::open(std::path::Path::new(":memory:")).await.unwrap();
    let input = CameraInput {
        id: None,
        name: "Front".into(),
        address: "192.168.1.20".into(),
        username: "".into(),
        password: None,
        path_main: None,
        path_sub: None,
        source_kind: None,
    };
    let camera = parse_address(&input)
        .unwrap()
        .into_camera("front".into(), "Front".into(), 1, 0);
    let identity = CameraIdentity {
        camera_id: camera.id.clone(),
        mac: Some("aa:bb:cc:dd:ee:ff".into()),
        ..Default::default()
    };
    store.save(&camera, &identity).await.unwrap();
    let updates = Arc::new(Mutex::new(Vec::new()));
    let sink = Sink {
        store: store.clone(),
        updates: updates.clone(),
    };
    assert_eq!(
        recover_camera(
            &camera,
            &identity,
            &FakeNetwork,
            &sink,
            &["ethernet".into()]
        )
        .await
        .unwrap(),
        Decision::Match("192.168.1.21".into())
    );
    assert_eq!(store.cameras().await.unwrap()[0].host, "192.168.1.21");
    assert_eq!(
        store
            .identity(&camera.id)
            .await
            .unwrap()
            .last_seen_host
            .as_deref(),
        Some("192.168.1.21")
    );
    assert_eq!(updates.lock().unwrap().len(), 1);
}
