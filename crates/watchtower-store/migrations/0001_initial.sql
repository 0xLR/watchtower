CREATE TABLE cameras (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  sort_order INTEGER NOT NULL,
  host TEXT NOT NULL,
  port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
  path_main TEXT NOT NULL,
  path_sub TEXT,
  username TEXT NOT NULL,
  source_kind TEXT NOT NULL CHECK (source_kind IN ('rtsp','manual_url','onvif_discovered')),
  is_local INTEGER NOT NULL
);
CREATE TABLE camera_identity (
  camera_id TEXT PRIMARY KEY NOT NULL REFERENCES cameras(id) ON DELETE CASCADE,
  mac TEXT,
  onvif_endpoint_uuid TEXT,
  serial TEXT,
  model TEXT,
  last_seen_at INTEGER,
  last_seen_host TEXT
);
CREATE TABLE settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
CREATE INDEX camera_sort ON cameras(sort_order, created_at);
