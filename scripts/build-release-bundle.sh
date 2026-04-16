#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 2 ]; then
  echo "usage: $0 <version> <arch> [binary-path]" >&2
  exit 1
fi

VERSION="$1"
ARCH="$2"
BINARY_PATH="${3:-target/release/cce}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$REPO_ROOT/dist"
STAGE_DIR="$DIST_DIR/cce-${VERSION}-macos-${ARCH}"
ARCHIVE_PATH="$DIST_DIR/cce-${VERSION}-macos-${ARCH}.tar.gz"

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/bin" "$STAGE_DIR/apps" "$STAGE_DIR/scripts"

install -m 755 "$BINARY_PATH" "$STAGE_DIR/bin/cce"
install -m 755 "$REPO_ROOT/bin/dev-editor" "$STAGE_DIR/bin/dev-editor"
install -m 755 "$REPO_ROOT/bin/dev-editor-open" "$STAGE_DIR/bin/dev-editor-open"
install -m 755 "$REPO_ROOT/bin/zed" "$STAGE_DIR/bin/zed"

cp "$REPO_ROOT"/apps/* "$STAGE_DIR/apps/"
cp "$REPO_ROOT"/scripts/install-cce-app.sh "$STAGE_DIR/scripts/"
cp "$REPO_ROOT"/scripts/install-zed-shim-app.sh "$STAGE_DIR/scripts/"
cp "$REPO_ROOT"/scripts/set-cce-associations.sh "$STAGE_DIR/scripts/"
install -m 644 "$REPO_ROOT/README.md" "$STAGE_DIR/README.md"
install -m 644 "$REPO_ROOT/LICENSE" "$STAGE_DIR/LICENSE"

cat > "$STAGE_DIR/install.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"

mkdir -p "$HOME/.local/bin"
install -m 755 "$ROOT/bin/cce" "$HOME/.local/bin/cce"
install -m 755 "$ROOT/bin/dev-editor" "$HOME/.local/bin/dev-editor"
install -m 755 "$ROOT/bin/dev-editor-open" "$HOME/.local/bin/dev-editor-open"
install -m 755 "$ROOT/bin/zed" "$HOME/.local/bin/zed"

echo "Installed CCE binaries into ~/.local/bin"
echo "Installing CCE.app and Zed.app shims..."
"$HOME/.local/bin/cce" install-macos

cat <<'INSTRUCTIONS'

Next steps:
  1. Add ~/.local/bin to PATH if it is not already present.
  2. Run: eval "$(cce shell-init zsh)"
  3. Restart your terminal.

Note:
  This release is unsigned. macOS may show a Gatekeeper warning the first time
  you open the app or run the binary.
INSTRUCTIONS
EOF
chmod +x "$STAGE_DIR/install.sh"

rm -f "$ARCHIVE_PATH"
tar -C "$DIST_DIR" -czf "$ARCHIVE_PATH" "$(basename "$STAGE_DIR")"
shasum -a 256 "$ARCHIVE_PATH" > "$ARCHIVE_PATH.sha256"

echo "$ARCHIVE_PATH"
