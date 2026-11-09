#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
exec node scripts/fetch-go2rtc.mjs "$@"
