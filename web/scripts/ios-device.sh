#!/usr/bin/env bash
# Build the iOS app and install it on the iPhone plugged into this Mac.
# Also the fix when the free provisioning profile expires (every 7 days):
# rebuilding with -allowProvisioningUpdates signs it again.
#
#   npm run ios:device            # first connected iPhone
#   IOS_DEVICE=<udid> npm run ios:device
#
# Needs: Xcode (any path; DEVELOPER_DIR defaults to /Applications/Xcode.app),
# the Apple ID of team AM5C55N339 signed in to Xcode, web/.env, the phone
# unlocked and "trusted" (Settings → General → VPN & Device Management).
set -euo pipefail
cd "$(dirname "$0")/.."

export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
TEAM="${IOS_TEAM:-AM5C55N339}"
BUNDLE="vn.inet.onescribe"
DERIVED="${IOS_DERIVED_DATA:-${TMPDIR:-/tmp}/onescribe-ios-build}"

DEVICE="${IOS_DEVICE:-}"
if [ -z "$DEVICE" ]; then
	DEVICE=$(xcrun devicectl list devices 2>/dev/null \
		| awk '/available \(paired\)/ && /iPhone/ { for (i = 1; i <= NF; i++) if ($i ~ /^[0-9A-F]{8}-[0-9A-F]{16}$/) { print $i; exit } }')
fi
if [ -z "$DEVICE" ]; then
	echo "Không thấy iPhone nào đang cắm và đã ghép đôi (xcrun devicectl list devices)." >&2
	exit 1
fi
echo "→ iPhone $DEVICE"

npm run build
npx cap sync ios

xcodebuild -project ios/App/App.xcodeproj -scheme App -configuration Release \
	-destination "id=$DEVICE" -derivedDataPath "$DERIVED" \
	-allowProvisioningUpdates DEVELOPMENT_TEAM="$TEAM" CODE_SIGN_STYLE=Automatic \
	build | grep -E "error:|warning:|BUILD (SUCCEEDED|FAILED)" || true

APP="$DERIVED/Build/Products/Release-iphoneos/App.app"
[ -d "$APP" ] || { echo "Build không tạo ra App.app — xem lỗi xcodebuild ở trên." >&2; exit 1; }

xcrun devicectl device install app --device "$DEVICE" "$APP" >/dev/null
echo "→ Đã cài. Mở app (iPhone phải đang mở khoá)…"
xcrun devicectl device process launch --device "$DEVICE" --terminate-existing "$BUNDLE" >/dev/null \
	&& echo "→ Xong." \
	|| echo "Không mở được app: mở khoá iPhone rồi bấm vào biểu tượng OneScribe."
