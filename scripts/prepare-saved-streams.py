"""Prepare explicitly authorized, credential-free saved cameras for local tests."""
import json
import os
import sqlite3
from pathlib import Path
from urllib.parse import urlsplit

root = Path(__file__).resolve().parent.parent
database = Path(os.environ["APPDATA"]) / "dev.watchtower.desktop" / "watchtower.sqlite"
connection = sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)
sources = []
for host, port, path, username in connection.execute("SELECT host,port,path_main,username FROM cameras ORDER BY sort_order"):
    if username:
        continue
    host = f"[{host}]" if ":" in host else host
    source = f"rtsp://{host}:{port}{path}"
    parsed = urlsplit(source)
    if parsed.username is not None or parsed.password is not None or parsed.fragment:
        continue
    if source not in sources:
        sources.append(source)
connection.close()
directory = root / ".local"
directory.mkdir(exist_ok=True)
(directory / "saved-streams.json").write_text(json.dumps(sources), encoding="utf-8")
print(f"Prepared {len(sources)} distinct credential-free saved streams for local playback tests.")
