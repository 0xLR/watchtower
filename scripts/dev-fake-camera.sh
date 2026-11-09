#!/usr/bin/env bash
set -euo pipefail
# Start MediaMTX separately with its RTSP listener on 127.0.0.1:8554.
# Requires an ffmpeg build with libx264 (development fixture only; not bundled).
exec ffmpeg -re -f lavfi -i testsrc2=size=1280x720:rate=15 \
  -an -c:v libx264 -preset ultrafast -tune zerolatency -pix_fmt yuv420p \
  -f rtsp -rtsp_transport tcp rtsp://127.0.0.1:8554/watchtower-test
