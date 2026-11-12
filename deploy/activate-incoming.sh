#!/bin/bash
# Activates a Scribe release uploaded to /opt/scribe-be/ci-incoming/<release-id> (by CI as `scribe-deploy`, or by
# deploy/deploy.sh as root). Installed as root at /usr/local/sbin/scribe-activate-incoming; `scribe-deploy` may run
# only this command through sudo (deploy/setup-deploy-user.sh).
#   scribe-activate-incoming <release-id>
# A release is `scribe-be` (static musl binary) plus `web/` (the built SPA), packed by deploy/pack-release.sh.
#
# One activation at a time (flock). The upload moves to a root-owned staging directory and is rejected if it holds
# links or special files or misses the binary or web/index.html. Before anything live changes, the NEW binary runs
# `admin busy` as the service user (this also proves it starts with the server's config and reaches Mongo through
# /opt/scribe-be/.mongo-uri); while recordings, uploads or transcriptions are open it polls every 15 s for up to
# 10 minutes, then gives up with exit 1 and the running release untouched. Then the binary is replaced atomically
# (previous one kept as scribe-be.prev), the web root gets the new assets/ first without --delete (tabs still on the
# previous build keep loading their chunks) and the rest with --delete (previous web root kept as dist.prev), RELEASE
# holds the id, scribe-be restarts and must answer /api/health within 30 s, otherwise the previous binary and web
# root come back, scribe-be restarts and the script exits 1. nginx and the systemd unit are not touched here
# (docs/deploy-web-production.md, one-time setup).
set -euo pipefail
APP_DIR=/opt/scribe-be
BIN="$APP_DIR/scribe-be"
MONGO_URI_FILE="$APP_DIR/.mongo-uri"
WEB=/var/www/scribe/dist
STAGE_ROOT=/var/lib/scribe-release
SERVICE=scribe-be
RUN_AS=scribe
HEALTH=http://127.0.0.1:8787/api/health
BUSY_POLL_S=15
BUSY_WAIT_S=600

ID="${1:?usage: scribe-activate-incoming <release-id>}"
[[ "$ID" =~ ^[0-9]{8}-[0-9]{6}-[0-9a-f]{12}$ ]] || { echo "bad release id" >&2; exit 2; }
[ "$(id -u)" -eq 0 ] || { echo "run as root" >&2; exit 1; }

# One activation at a time: a CI deploy and a break-glass deploy from a dev machine may meet.
exec 9>/run/scribe-activate.lock
flock -w 600 9 || { echo "another activation still holds /run/scribe-activate.lock" >&2; exit 1; }

if [ ! -f "$MONGO_URI_FILE" ]; then
	echo "no $MONGO_URI_FILE: the binary reads the Mongo URI from it at startup. Create it once as root, value over" >&2
	echo "stdin, never argv: printf '%s\\n' \"\$MONGO_URI\" | install -m 600 -o $RUN_AS -g $RUN_AS /dev/stdin $MONGO_URI_FILE" >&2
	exit 1
fi

SRC="$APP_DIR/ci-incoming/$ID"
[ -d "$SRC" ] && [ ! -L "$SRC" ] || { echo "incoming release $ID missing" >&2; exit 2; }

install -d -m 700 -o root -g root "$STAGE_ROOT"
NEW="$STAGE_ROOT/incoming"
rm -rf "$NEW"
mv "$SRC" "$NEW"
chown -R root:root "$NEW"

switched=0
healthy() {
	for _ in $(seq 1 30); do
		curl -fsS "$HEALTH" >/dev/null 2>&1 && return 0
		sleep 1
	done
	return 1
}
restore() {
	echo "switching back to the previous release" >&2
	if [ -f "$BIN.prev" ]; then
		install -m 700 -o "$RUN_AS" -g "$RUN_AS" "$BIN.prev" "$BIN.new"
		mv -f "$BIN.new" "$BIN"
	fi
	if [ -d "$WEB.prev" ]; then rsync -a --delete "$WEB.prev/" "$WEB/"; fi
	systemctl restart "$SERVICE" 9>&-
	if healthy; then echo "previous release is back and healthy" >&2; else echo "previous release is not healthy either" >&2; fi
}
on_exit() {
	local rc=$?
	if [ "$rc" -ne 0 ] && [ "$switched" = 1 ]; then
		journalctl -u "$SERVICE" -n 40 --no-pager || true
		restore || true
	fi
	rm -f "$BIN.new"
	rm -rf "$NEW"
	exit "$rc"
}
trap on_exit EXIT

reject() {
	echo "incoming release $ID $1" >&2
	exit 2
}
[ -z "$(find "$NEW" ! -type f ! -type d -print -quit)" ] || reject "has links or special files"
for f in scribe-be web/index.html; do
	[ -f "$NEW/$f" ] || reject "has no $f"
done
printf '%s\n' "$ID" >"$NEW/web/RELEASE"
chmod -R u=rwX,go=rX "$NEW/web"

# The new binary sits next to the live one, owned by the service user, so it can run `admin busy` as that user.
install -m 700 -o "$RUN_AS" -g "$RUN_AS" "$NEW/scribe-be" "$BIN.new"
cd "$APP_DIR"
waited=0
while :; do
	rc=0
	out="$(runuser -u "$RUN_AS" -- "$BIN.new" admin busy 2>&1 9>&-)" || rc=$?
	out="$(printf '%s' "$out" | tr -s ' \n' ' ')"
	case "$rc" in
		0) break ;;
		3) ;;
		*)
			echo "the new binary failed 'admin busy' (exit $rc), release not activated: $out" >&2
			exit 1
			;;
	esac
	if [ "$waited" -ge "$BUSY_WAIT_S" ]; then
		echo "still busy after $((BUSY_WAIT_S / 60)) min, release not activated (running release untouched): $out" >&2
		exit 1
	fi
	echo "busy, checking again in $BUSY_POLL_S s: $out"
	sleep "$BUSY_POLL_S"
	waited=$((waited + BUSY_POLL_S))
done

if [ -f "$BIN" ]; then cp -p "$BIN" "$BIN.prev"; fi
rm -rf "$WEB.prev"
if [ -d "$WEB" ]; then cp -a "$WEB" "$WEB.prev"; fi
switched=1
mv -f "$BIN.new" "$BIN"
mkdir -p "$WEB/assets"
rsync -a "$NEW/web/assets/" "$WEB/assets/"
rsync -a --delete --exclude /assets/ "$NEW/web/" "$WEB/"

systemctl restart "$SERVICE" 9>&-
if ! healthy; then
	echo "/api/health did not answer within 30 s" >&2
	exit 1
fi
switched=0
echo "health ok: scribe.hoandat.com now serves $ID"
