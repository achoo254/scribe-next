#!/usr/bin/env bash
# Build the Android app (debug) and install it on the phone or emulator adb sees.
# For trying on a device only; Play builds come from CI (.github/workflows/android-release.yml).
#
#   pnpm run android:device              # first device in `adb devices`
#   ANDROID_SERIAL=<serial> pnpm run android:device
#
# Needs: JDK 21, the Android SDK (ANDROID_HOME, or sdk.dir in android/local.properties), adb on PATH,
# web/.env, USB debugging allowed on the phone. Heavy (Gradle): do not run it next to another build or
# test suite on the shared 16 GB machine.
set -euo pipefail
cd "$(dirname "$0")/.."

command -v adb >/dev/null || { echo "Không thấy adb (Android SDK platform-tools) trên PATH." >&2; exit 1; }
DEVICE="${ANDROID_SERIAL:-$(adb devices | awk 'NR > 1 && $2 == "device" { print $1; exit }')}"
[ -n "$DEVICE" ] || { echo "Không thấy máy Android nào (adb devices): cắm máy và cho phép gỡ lỗi USB." >&2; exit 1; }
echo "→ Android $DEVICE"

# Backend origin of the app (BuildConfig.SCRIBE_BACKEND_ORIGIN), from the same source as the web build: web/.env.
ORIGIN=$(grep -E '^VITE_NATIVE_API_ORIGIN=' .env | tail -1 | cut -d= -f2-)
[ -n "$ORIGIN" ] || { echo "Thiếu VITE_NATIVE_API_ORIGIN trong web/.env (xem web/.env.example)." >&2; exit 1; }

pnpm run build
pnpm exec cap sync android

(cd android && SCRIBE_BACKEND_ORIGIN="$ORIGIN" ANDROID_SERIAL="$DEVICE" ./gradlew --no-daemon installDebug)

adb -s "$DEVICE" shell am start -n com.hoandat.scribe/.MainActivity >/dev/null \
	&& echo "→ Xong." \
	|| echo "Không mở được app: mở khoá máy rồi bấm vào biểu tượng Scribe."
