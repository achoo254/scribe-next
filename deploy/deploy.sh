#!/usr/bin/env bash
# Break-glass deploy of Scribe to the shared server (.65), for when GitHub Actions cannot deploy. The normal path is a
# push to main: .github/workflows/deploy.yml builds, uploads and activates once CI is green.
#   bash deploy/deploy.sh            # from anywhere inside the repo, on a Mac or Linux dev machine
# Refuses a dirty tree, a commit that is not on origin/main, and a commit without a green CI run (the full test suites
# are not run on dev machines). Builds the backend with cargo zigbuild (musl) and the web with Vite from the iCloud
# env folder (scripts/env-dir.sh): .env.production minus MONGO_URI, which only the database helper scripts use
# (the binary reads /opt/scribe-be/.mongo-uri through MONGO_URI_PATH). Packs with deploy/pack-release.sh, uploads as
# root through the scribe-ops-prod skill's prod-ssh.sh, installs this commit's deploy/activate-incoming.sh and runs
# it, the same activation as CI (it waits for any other activation, for open recordings, uploads and transcriptions,
# and switches back to the previous release when /api/health fails).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
SSH=.claude/skills/scribe-ops-prod/scripts/prod-ssh.sh
TARGET=x86_64-unknown-linux-musl
export COPYFILE_DISABLE=1

echo "== guards"
[ -f "$SSH" ] || { echo "needs $SSH (root SSH to the server, from the local scribe-ops-prod skill)" >&2; exit 1; }
if [ -n "$(git status --porcelain)" ]; then
	echo "working tree is not clean: commit or stash first (git status)" >&2
	exit 1
fi
git fetch --quiet origin main
if ! git merge-base --is-ancestor HEAD origin/main; then
	echo "HEAD $(git rev-parse --short HEAD) is not on origin/main: push it first" >&2
	exit 1
fi
sha="$(git rev-parse HEAD)"
green="$(gh run list --workflow CI --commit "$sha" --status success --json databaseId --jq length)"
if [ "$green" = 0 ]; then
	echo "no green CI run for $sha: wait for CI (gh run list --commit $sha) or fix it first" >&2
	exit 1
fi

echo "== build"
env_file="$(bash scripts/env-dir.sh)/.env.production"
[ -f "$env_file" ] || { echo "missing $env_file" >&2; exit 1; }
work="$(mktemp -d)"
# Expanded now on purpose: $work is fixed for the rest of the script.
# shellcheck disable=SC2064
trap "rm -rf '$work'" EXIT
(
	set -a
	# shellcheck disable=SC1090 # The env file lives on iCloud, outside the repo.
	. "$env_file"
	set +a
	unset MONGO_URI
	[ -n "${MONGO_URI_PATH:-}" ] || { echo "MONGO_URI_PATH is not set in $env_file" >&2; exit 1; }
	CARGO_TARGET_DIR="$work/target" cargo zigbuild --release --target "$TARGET" -p scribe-be
)
(cd web && pnpm install --frozen-lockfile && pnpm run build)
bash deploy/pack-release.sh "$work/release" "$work/target/$TARGET/release/scribe-be"
cp deploy/activate-incoming.sh "$work/activate-incoming.sh"

id="$(date -u +%Y%m%d-%H%M%S)-${sha:0:12}"
echo "== upload $id"
tar -czf - -C "$work" release activate-incoming.sh |
	bash "$SSH" "set -euo pipefail
		up=\$(mktemp -d); trap 'rm -rf \"\$up\"' EXIT
		tar xzf - --no-same-owner -C \"\$up\"
		install -m 755 -o root -g root \"\$up/activate-incoming.sh\" /usr/local/sbin/scribe-activate-incoming
		mkdir -p /opt/scribe-be/ci-incoming
		mv \"\$up/release\" /opt/scribe-be/ci-incoming/$id"

echo "== activate (the same step CI runs)"
bash "$SSH" "/usr/local/sbin/scribe-activate-incoming $id" </dev/null

echo "== smoke"
served="$(curl -fsS "https://scribe.hoandat.com/RELEASE?v=$id" | tr -d '\r\n')"
[ "$served" = "$id" ] || { echo "origin serves '$served', expected $id" >&2; exit 1; }
curl -fsS https://scribe.hoandat.com/api/health
echo
echo "deployed $id"
