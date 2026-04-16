#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${ZED_SHIM_APP_NAME:-Zed.app}"
APP_PATH="${ZED_SHIM_APP_PATH:-$HOME/Applications/$APP_NAME}"
APP_DIR="$(dirname "$APP_PATH")"
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="$SCRIPT_DIR/apps/zed.applescript"
BUNDLE_FRAGMENT="$SCRIPT_DIR/apps/zed-shim-info.plist"
BUNDLE_ID="${ZED_SHIM_BUNDLE_ID:-dev.zed.Zed}"

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

mkdir -p "$APP_DIR"
rm -rf "$APP_PATH"
osacompile -o "$APP_PATH" "$SOURCE"

PLIST="$APP_PATH/Contents/Info.plist"
if /usr/libexec/PlistBuddy -c "Print :CFBundleURLTypes" "$PLIST" >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy -c "Delete :CFBundleURLTypes" "$PLIST"
fi
/usr/libexec/PlistBuddy -c "Merge $BUNDLE_FRAGMENT :" "$PLIST"
plist_set "CFBundleIdentifier" string "$BUNDLE_ID"
plist_set "CFBundleName" string "Zed"
plist_set "LSUIElement" bool true

/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP_PATH" >/dev/null 2>&1 || true
mdimport "$APP_PATH" >/dev/null 2>&1 || true

echo "$APP_PATH"
