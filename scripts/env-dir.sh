#!/usr/bin/env bash
# Prints the folder that holds Scribe's env files. They live on iCloud, never
# in the repo, so the Mac and the Windows machine read the same copies:
#   .env.production   backend build for prod (.65)
#   .env.development  backend for dev local
#   web/.env          web and app builds (public values)
# Order: $SCRIBE_ENV_DIR, then the iCloud folder on macOS, then on Windows.
# web/vite.config.ts looks in the same places; keep the two lists together.
#   set -a; . "$(scripts/env-dir.sh)/.env.production"; set +a
set -euo pipefail

SUB="cong-viec/Cong viec/ENV production/one-scribe"
for dir in \
	"${SCRIBE_ENV_DIR:-}" \
	"$HOME/Library/Mobile Documents/com~apple~CloudDocs/$SUB" \
	"${USERPROFILE:+$USERPROFILE/iCloudDrive/$SUB}"; do
	if [ -n "$dir" ] && [ -d "$dir" ]; then
		printf '%s\n' "$dir"
		exit 0
	fi
done
echo "Không thấy thư mục env của Scribe trên iCloud (…/ENV production/one-scribe); đặt SCRIBE_ENV_DIR nếu để chỗ khác." >&2
exit 1
