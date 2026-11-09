//! SQLite persistence. Secrets are not part of this API or schema.
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    ConnectOptions, Row, SqlitePool,
};
use std::{path::Path, time::Duration};
use watchtower_core::{Camera, CameraIdentity, Settings};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Camera data could not be read or saved.")]
    Database(#[from] sqlx::Error),
    #[error("Camera data could not be upgraded.")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("The saved settings are invalid.")]
    Settings(#[from] serde_json::Error),
    #[error("Camera order is out of date. Refresh and try again.")]
    Order,
}
#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}
impl Store {
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            .disable_statement_logging();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }
    pub async fn close(&self) {
        self.pool.close().await;
    }
    pub async fn cameras(&self) -> Result<Vec<Camera>, StoreError> {
        let rows = sqlx::query("SELECT * FROM cameras ORDER BY sort_order, created_at, id")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(Camera {
                    id: row.try_get("id")?,
                    name: row.try_get("name")?,
                    created_at: row.try_get("created_at")?,
                    sort_order: row.try_get("sort_order")?,
                    host: row.try_get("host")?,
                    port: row.try_get("port")?,
                    path_main: row.try_get("path_main")?,
                    path_sub: row.try_get("path_sub")?,
                    username: row.try_get("username")?,
                    source_kind: row.try_get("source_kind")?,
                    is_local: row.try_get("is_local")?,
                })
            })
            .collect()
    }
    pub async fn save(&self, camera: &Camera, identity: &CameraIdentity) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO cameras (id,name,created_at,sort_order,host,port,path_main,path_sub,username,source_kind,is_local) VALUES (?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,host=excluded.host,port=excluded.port,path_main=excluded.path_main,path_sub=excluded.path_sub,username=excluded.username,source_kind=excluded.source_kind,is_local=excluded.is_local")
            .bind(&camera.id).bind(&camera.name).bind(camera.created_at).bind(camera.sort_order).bind(&camera.host).bind(camera.port).bind(&camera.path_main).bind(&camera.path_sub).bind(&camera.username).bind(&camera.source_kind).bind(camera.is_local).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO camera_identity (camera_id,mac,onvif_endpoint_uuid,serial,model,last_seen_at,last_seen_host) VALUES (?,?,?,?,?,?,?) ON CONFLICT(camera_id) DO UPDATE SET mac=excluded.mac,onvif_endpoint_uuid=excluded.onvif_endpoint_uuid,serial=excluded.serial,model=excluded.model,last_seen_at=excluded.last_seen_at,last_seen_host=excluded.last_seen_host")
            .bind(&camera.id).bind(&identity.mac).bind(&identity.onvif_endpoint_uuid).bind(&identity.serial).bind(&identity.model).bind(identity.last_seen_at).bind(&identity.last_seen_host).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn identity(&self, id: &str) -> Result<CameraIdentity, StoreError> {
        let row = sqlx::query("SELECT * FROM camera_identity WHERE camera_id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(CameraIdentity {
                camera_id: id.into(),
                ..Default::default()
            });
        };
        Ok(CameraIdentity {
            camera_id: id.into(),
            mac: row.try_get("mac")?,
            onvif_endpoint_uuid: row.try_get("onvif_endpoint_uuid")?,
            serial: row.try_get("serial")?,
            model: row.try_get("model")?,
            last_seen_at: row.try_get("last_seen_at")?,
            last_seen_host: row.try_get("last_seen_host")?,
        })
    }
    pub async fn delete(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM cameras WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn reorder(&self, ids: &[String]) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        let existing: Vec<String> = sqlx::query_scalar("SELECT id FROM cameras")
            .fetch_all(&mut *tx)
            .await?;
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        if unique.len() != ids.len()
            || existing.len() != ids.len()
            || existing.iter().any(|id| !unique.contains(id))
        {
            return Err(StoreError::Order);
        }
        for (order, id) in ids.iter().enumerate() {
            sqlx::query("UPDATE cameras SET sort_order=? WHERE id=?")
                .bind(order as i32)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn settings(&self) -> Result<Settings, StoreError> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT value FROM settings WHERE key='preferences'")
                .fetch_optional(&self.pool)
                .await?;
        value
            .map(|v| serde_json::from_str(&v))
            .transpose()
            .map(|s| s.unwrap_or_default())
            .map_err(Into::into)
    }
    pub async fn save_settings(&self, settings: &Settings) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO settings(key,value) VALUES('preferences',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value").bind(serde_json::to_string(settings)?).execute(&self.pool).await?;
        Ok(())
    }
}
