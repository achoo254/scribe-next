#!/usr/bin/env python3
"""Checks the merged AndroidManifest.xml of a Gradle build of web/android (CI, after assemble/bundle).

- targetSdkVersion is the one variables.gradle pins (Google Play's requirement, docs/android-app.md);
- the app asks for no permission outside the list the Play declarations cover (Data safety, foreground
  service of type microphone), so a dependency cannot add one silently.

Usage: android-check-manifest.py <debug|release>   (run from web/android)
"""

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ANDROID = "{http://schemas.android.com/apk/res/android}"
ALLOWED = {
    "android.permission.INTERNET",
    "android.permission.ACCESS_NETWORK_STATE",
    "android.permission.RECORD_AUDIO",
    "android.permission.FOREGROUND_SERVICE",
    "android.permission.FOREGROUND_SERVICE_MICROPHONE",
    "android.permission.POST_NOTIFICATIONS",
    # androidx.core: signature permission protecting the app's own non-exported dynamic receivers.
    "com.hoandat.scribe.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION",
}


def main() -> int:
    variant = sys.argv[1] if len(sys.argv) > 1 else "debug"
    candidates = sorted(
        path
        for path in Path("app/build/intermediates").glob("merged_manifest*/**/AndroidManifest.xml")
        if variant in str(path).lower()
    )
    if not candidates:
        print(f"::error::No merged manifest for {variant} under app/build/intermediates", file=sys.stderr)
        return 1
    manifest = candidates[0]
    variables = Path("variables.gradle").read_text(encoding="utf-8")
    match = re.search(r"targetSdkVersion\s*=\s*(\d+)", variables)
    if match is None:
        print("::error::targetSdkVersion not found in variables.gradle", file=sys.stderr)
        return 1
    expected_target = match.group(1)

    root = ET.parse(manifest).getroot()
    sdk = root.find("uses-sdk")
    target = sdk.get(f"{ANDROID}targetSdkVersion") if sdk is not None else None
    permissions = {element.get(f"{ANDROID}name") for element in root.iter("uses-permission")}

    failed = False
    if target != expected_target:
        print(f"::error::targetSdkVersion is {target}, expected {expected_target} ({manifest})", file=sys.stderr)
        failed = True
    extra = sorted(name for name in permissions if name not in ALLOWED)
    if extra:
        print(f"::error::Permissions outside the declared list: {', '.join(extra)}", file=sys.stderr)
        failed = True
    print(f"{manifest}: targetSdkVersion={target}; permissions: {', '.join(sorted(p for p in permissions if p))}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
