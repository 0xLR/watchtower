use crate::state::{stream_id, AppState};
use std::sync::Arc;
use tauri::State;
use watchtower_core::*;

#[tauri::command]
pub async fn snapshot(state: State<'_, Arc<AppState>>) -> Result<AppSnapshot, String> {
    state.snapshot().await
}
#[tauri::command]
pub async fn test_camera(
    state: State<'_, Arc<AppState>>,
    input: CameraInput,
) -> Result<String, String> {
    state.test_camera(input).await
}
#[tauri::command]
pub async fn save_camera(state: State<'_, Arc<AppState>>, token: String) -> Result<String, String> {
    state.save_camera(&token).await
}
#[tauri::command]
pub async fn playback(
    state: State<'_, Arc<AppState>>,
    id: String,
    main: bool,
) -> Result<Playback, String> {
    state.playback(&id, main).await
}
#[tauri::command]
pub async fn delete_camera(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let _guard = state.operation.lock().await;
    let old = state.password(&id).await?;
    state.set_password(&id, None).await?;
    if let Err(error) = state.store.delete(&id).await {
        let _ = state.set_password(&id, old).await;
        return Err(error.to_string());
    }
    state.active_streams.write().await.remove(&id);
    if let Ok(media) = state.media() {
        for main in [true, false] {
            let _ = media.remove_stream(&stream_id(&id, main)).await;
        }
    }
    state.statuses.write().await.remove(&id);
    Ok(())
}
#[tauri::command]
pub async fn reorder_cameras(
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<(), String> {
    let _guard = state.operation.lock().await;
    state.store.reorder(&ids).await.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn save_settings(
    state: State<'_, Arc<AppState>>,
    settings: Settings,
) -> Result<(), String> {
    let _guard = state.operation.lock().await;
    if !matches!(settings.layout.as_str(), "auto" | "1" | "2" | "3" | "4") {
        return Err("Unknown grid layout".into());
    }
    if settings.selected_interfaces.len() > 4 {
        return Err("Select at most four network interfaces".into());
    }
    state.tested.lock().await.clear();
    state
        .store
        .save_settings(&settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn player_state(
    state: State<'_, Arc<AppState>>,
    id: String,
    main: bool,
    status: StreamStatus,
) -> Result<(), String> {
    if !state
        .store
        .cameras()
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .any(|c| c.id == id)
    {
        return Err("Camera not found".into());
    }
    if status == StreamStatus::Idle {
        state.active_streams.write().await.remove(&id);
    } else {
        state.active_streams.write().await.insert(id.clone(), main);
    }
    state.statuses.write().await.insert(id, status);
    Ok(())
}
