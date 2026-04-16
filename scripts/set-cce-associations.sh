#!/usr/bin/env bash
set -euo pipefail

BUNDLE_ID="${CCE_APP_BUNDLE_ID:-dev.hercules.cce}"
PLIST_PATH="$HOME/Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist"

python3 - "$PLIST_PATH" "$BUNDLE_ID" <<'PY'
import plistlib
import tempfile
import time
from pathlib import Path
import sys

plist_path = Path(sys.argv[1]).expanduser()
bundle_id = sys.argv[2]
old_bundle_ids = {"dev.editorbridge.config", "com.hgimenes.deveditor"}

managed_content_types = {
    "public.source-code",
    "public.script",
    "public.shell-script",
    "public.bash-script",
    "public.csh-script",
    "public.ksh-script",
    "public.tcsh-script",
    "public.zsh-script",
    "public.json",
    "public.xml",
    "com.apple.xml-property-list",
    "com.apple.property-list",
    "com.netscape.javascript-source",
    "net.daringfireball.markdown",
    "public.python-script",
    "public.c-source",
    "public.c-header",
    "public.c-source.preprocessed",
    "public.c-plus-plus-source",
    "public.c-plus-plus-header",
    "public.c-plus-plus-inline-header",
    "public.objective-c-source",
    "public.objective-c-plus-plus-source",
    "public.swift-source",
    "com.sun.java-source",
    "public.ruby-script",
    "public.php-script",
    "public.perl-script",
    "public.ada-source",
    "public.assembly-source",
    "public.dylan-source",
    "public.fortran-source",
    "public.fortran-77-source",
    "public.fortran-90-source",
    "public.fortran-95-source",
    "public.lex-source",
    "public.yacc-source",
    "public.make-source",
    "public.yaml",
    "public.geojson",
    "public.css",
    "public.xhtml",
    "org.khronos.gltf",
}

managed_extensions = {
    "env",
    "gitignore",
    "gitattributes",
    "editorconfig",
    "npmrc",
    "nvmrc",
    "zshrc",
    "zprofile",
    "bashrc",
    "bash_profile",
    "bazelrc",
    "bazelversion",
    "bazelignore",
    "py",
    "ts",
    "tsx",
    "rs",
    "json",
    "md",
    "js",
    "jsx",
    "sh",
    "yaml",
    "yml",
    "toml",
}

timestamp = int(time.time())

if plist_path.exists():
    with plist_path.open("rb") as handle:
        data = plistlib.load(handle)
else:
    data = {}

handlers = data.get("LSHandlers", [])
filtered = []
for item in handlers:
    roles = {
        item.get("LSHandlerRoleAll"),
        item.get("LSHandlerRoleEditor"),
        item.get("LSHandlerRoleViewer"),
        item.get("LSHandlerRoleShell"),
    }
    content_type = item.get("LSHandlerContentType")
    content_tag = item.get("LSHandlerContentTag")
    content_tag_class = item.get("LSHandlerContentTagClass")

    if roles & old_bundle_ids:
        continue
    if bundle_id in roles and content_type in managed_content_types:
        continue
    if (
        bundle_id in roles
        and content_tag_class == "public.filename-extension"
        and content_tag in managed_extensions
    ):
        continue
    filtered.append(item)

for content_type in sorted(managed_content_types):
    filtered.append(
        {
            "LSHandlerContentType": content_type,
            "LSHandlerRoleAll": bundle_id,
            "LSHandlerRoleEditor": bundle_id,
            "LSHandlerRoleViewer": bundle_id,
            "LSHandlerPreferredVersions": {"LSHandlerRoleAll": "-"},
            "LSHandlerModificationDate": timestamp,
        }
    )

for extension in sorted(managed_extensions):
    filtered.append(
        {
            "LSHandlerContentTag": extension,
            "LSHandlerContentTagClass": "public.filename-extension",
            "LSHandlerRoleAll": bundle_id,
            "LSHandlerRoleEditor": bundle_id,
            "LSHandlerRoleViewer": bundle_id,
            "LSHandlerPreferredVersions": {"LSHandlerRoleAll": "-"},
            "LSHandlerModificationDate": timestamp,
        }
    )

data["LSHandlers"] = filtered
plist_path.parent.mkdir(parents=True, exist_ok=True)
with tempfile.NamedTemporaryFile("wb", delete=False, dir=str(plist_path.parent)) as handle:
    plistlib.dump(data, handle, fmt=plistlib.FMT_BINARY)
    tmp_path = Path(handle.name)
tmp_path.replace(plist_path)
PY

killall cfprefsd >/dev/null 2>&1 || true
