"""Seed synthetic cameras into an isolated development database, never user app data."""
import sqlite3, sys, uuid
from pathlib import Path
root = Path(__file__).resolve().parents[1]
folder = (root / '.local' / 'native-test').resolve()
folder.mkdir(parents=True, exist_ok=True)
path = folder / 'watchtower.sqlite'
connection = sqlite3.connect(path)
# Run the checked-in schema only for an empty fixture DB; Rust migrates real app DBs.
if not connection.execute("SELECT name FROM sqlite_master WHERE name='cameras'").fetchone():
    raise SystemExit('Start the app with WATCHTOWER_DEV_DATA=.local/native-test once so Rust can migrate this fixture database.')
count = int(sys.argv[1]) if len(sys.argv) > 1 else 9
for index in range(count):
    camera_id = str(uuid.uuid5(uuid.NAMESPACE_DNS, f'watchtower-test-{index}'))
    connection.execute('INSERT OR IGNORE INTO cameras VALUES (?,?,?,?,?,?,?,?,?,?,?)', (camera_id, f'Test camera {index + 1:02d}', 1, index, '127.0.0.1', 8554, '/watchtower-test', '/watchtower-sub', '', 'rtsp', 0))
    connection.execute('INSERT OR IGNORE INTO camera_identity(camera_id) VALUES (?)', (camera_id,))
connection.commit()
connection.close()
print(path)
