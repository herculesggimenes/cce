#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${CCE_APP_NAME:-CCE.app}"
APP_PATH="${CCE_APP_PATH:-$HOME/Applications/$APP_NAME}"
APP_DIR="$(dirname "$APP_PATH")"
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="$SCRIPT_DIR/apps/cce.applescript"
INFO_PLIST_SOURCE="$SCRIPT_DIR/apps/cce-info.plist"
BUNDLE_FRAGMENT="$SCRIPT_DIR/apps/cce-document-types.plist"
BUNDLE_ID="${CCE_APP_BUNDLE_ID:-dev.hercules.cce}"

plist_set() {
  local key="$1"
  local type="$2"
  local value="$3"
  if /usr/libexec/PlistBuddy -c "Print :$key" "$PLIST" >/dev/null 2>&1; then
    /usr/libexec/PlistBuddy -c "Set :$key $value" "$PLIST"
  else
    /usr/libexec/PlistBuddy -c "Add :$key $type $value" "$PLIST"
  fi
}

plist_delete_if_exists() {
  local key="$1"
  if /usr/libexec/PlistBuddy -c "Print :$key" "$PLIST" >/dev/null 2>&1; then
    /usr/libexec/PlistBuddy -c "Delete :$key" "$PLIST"
  fi
}

mkdir -p "$APP_DIR"
rm -rf "$APP_PATH"
osacompile -o "$APP_PATH" "$SOURCE"

PLIST="$APP_PATH/Contents/Info.plist"
python3 - "$PLIST" "$INFO_PLIST_SOURCE" <<'PY'
import plistlib
from pathlib import Path
import sys

dest_path = Path(sys.argv[1])
source_path = Path(sys.argv[2])

with dest_path.open("rb") as handle:
    dest = plistlib.load(handle)

with source_path.open("rb") as handle:
    source = plistlib.load(handle)

for key, value in source.items():
    if key == "CFBundleExecutable":
        continue
    dest[key] = value

with dest_path.open("wb") as handle:
    plistlib.dump(dest, handle)
PY
plist_delete_if_exists "CFBundleDocumentTypes"
/usr/libexec/PlistBuddy -c "Merge $BUNDLE_FRAGMENT :" "$PLIST"
plist_set "CFBundleIdentifier" string "$BUNDLE_ID"
plist_set "CFBundleName" string "CCE"
plist_set "LSUIElement" bool true

/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP_PATH" >/dev/null 2>&1 || true
mdimport "$APP_PATH" >/dev/null 2>&1 || true

echo "$APP_PATH"
