#!/usr/bin/env bash
# Packs a Scribe release from a built checkout into <out-dir>: `scribe-be` (the musl binary) and `web/` (web/dist).
# Refuses a binary that embeds a Mongo URI with credentials: the repository is public and so are its build
# artifacts; production reads the URI from /opt/scribe-be/.mongo-uri (MONGO_URI_PATH). Used by
# .github/workflows/deploy.yml and by deploy/deploy.sh.
#   bash deploy/pack-release.sh <out-dir> <path-to-scribe-be-binary>
set -euo pipefail
out="${1:?usage: deploy/pack-release.sh <out-dir> <binary>}"
bin="${2:?usage: deploy/pack-release.sh <out-dir> <binary>}"
dist="$(cd "$(dirname "$0")/.." && pwd)/web/dist"
[ -f "$bin" ] || { echo "no binary at $bin" >&2; exit 1; }
[ -f "$dist/index.html" ] || { echo "no $dist/index.html: build the web first" >&2; exit 1; }
if LC_ALL=C grep -aqE 'mongodb(\+srv)?://[^/@[:space:]]+:[^@[:space:]]+@' "$bin"; then
	echo "$bin embeds a Mongo URI with a password: build with MONGO_URI_PATH, not MONGO_URI" >&2
	exit 1
fi
mkdir -p "$out/web"
cp "$bin" "$out/scribe-be"
COPYFILE_DISABLE=1 tar -cf - -C "$dist" --exclude '._*' --exclude .DS_Store . | tar -xf - -C "$out/web"
