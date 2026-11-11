ALTER TABLE cameras ADD COLUMN media_engine TEXT CHECK (media_engine IN ('retina','go2rtc','mediamtx'));
-- Profiles with cameras but no explicit preferences previously used go2rtc.
INSERT INTO settings(key,value)
SELECT 'preferences','{"media_engine":"go2rtc","layout":"auto","default_substream":true,"background_sweep":false,"selected_interfaces":[]}'
WHERE EXISTS (SELECT 1 FROM cameras)
  AND NOT EXISTS (SELECT 1 FROM settings WHERE key='preferences');
