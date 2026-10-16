#!/usr/bin/env bash
# Runs a local mongosh script against the production Scribe database (shared
# mongod 127.0.0.1:27017 on server .65, database $DB_NAME). The backend has no
# .env on the server (MONGO_URI is baked into the binary at build time), so
# the URI comes from the repo's .env.production. It travels inside the script
# over stdin into a temp file (mode 600, deleted on exit) — never argv — and
# `db` is connected before the script runs, so the script starts directly
# with queries:
#   prod-mongosh.sh /path/to/query.js
#   prod-mongosh.sh scripts/transcripts-audit.js      # read-only data audit
# Keep queries small and read-only: the server has ~3 GB of free RAM shared
# with other apps, and writes need an explicit request from the user.
set -euo pipefail

QUERY="${1:?usage: prod-mongosh.sh <query.js>}"
[ -f "$QUERY" ] || { echo "no such file: $QUERY" >&2; exit 2; }
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-toplevel)"
ENV_FILE="$ROOT/.env.production"
[ -f "$ENV_FILE" ] || { echo "missing $ENV_FILE (prod config, not committed)" >&2; exit 2; }
URI="$(sed -n 's/^MONGO_URI=//p' "$ENV_FILE" | tail -n 1)"; URI="${URI#\"}"; URI="${URI%\"}"
[ -n "$URI" ] || { echo "MONGO_URI not set in .env.production" >&2; exit 2; }

# shellcheck disable=SC2016 # Expanded on the server, not here.
REMOTE='set -euo pipefail
f="$(mktemp --suffix=.js)"; chmod 600 "$f"; trap '"'"'rm -f "$f"'"'"' EXIT
cat >"$f"
mongosh --nodb --quiet --file "$f"'

{
	printf 'db = connect(%s);\n' "$(jq -Rn --arg u "$URI" '$u')"
	cat "$QUERY"
} | "$HERE/prod-ssh.sh" "$REMOTE"
