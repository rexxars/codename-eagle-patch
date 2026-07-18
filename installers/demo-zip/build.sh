#!/usr/bin/env bash
# Package the Codename Eagle multiplayer demo as a plain zip: the same game
# files as the demo installer, with none of the setup niceties (no firewall
# rules, no cneagle:// registration, no shortcut, no uninstaller). For players
# who prefer extract-and-play, and for mirrors that want a plain archive.
#
# usage: ./build.sh [version] [out.zip]
#   version   version in the zip name and bundled note   (default: 1.50)
#   out.zip   output path (default: out/codename-eagle-mp-demo-<version>.zip)
#
# The payload is staged by the demo installer's own script
# (../demo/build.sh --stage-only), so the payload definition - game/common/
# overlaid with game/demo/ - and its checks (git-lfs stubs, case-duplicate
# paths, mac cruft) live in exactly one place.
#
# Requires zip.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"

version="${1:-1.50}"
out="${2:-$here/out/codename-eagle-mp-demo-$version.zip}"

command -v zip >/dev/null || {
  echo "error: zip not found" >&2
  exit 1
}

# Stage via the demo installer's script; it prints "staged: <dir>" and keeps
# the dir for the caller to consume (and clean up).
stage="$("$here/../demo/build.sh" --stage-only "$version" | sed -n 's/^staged: //p')"
[[ -n "$stage" && -d "$stage/payload" && -d "$stage/dgvoodoo" ]] || {
  echo "error: staging via ../demo/build.sh --stage-only failed" >&2
  exit 1
}
trap 'rm -rf "$stage"' EXIT

# dgVoodoo is an optional component in the installer, so the demo staging keeps
# its six files in a separate dir. The zip has no opt-out - merge them back into
# the payload so extract-and-play always includes dgVoodoo.
cp -R "$stage/dgvoodoo/." "$stage/payload/"

# Zip with a top-level folder so extraction never splats files into cwd.
root="codename-eagle-mp-demo"
mv "$stage/payload" "$stage/$root"

# The zip counterpart of the things the installer does beyond copying files.
cat > "$stage/$root/README-zip.txt" <<EOF
Codename Eagle multiplayer demo $version (community patch)
https://codenameeagle.net/

Extract this folder anywhere and run ce.exe - no installation needed.

Because this is a plain zip, a few things the installer normally sets up
are manual:

- Windows Firewall: the first time you refresh the server list or HOST
  a game, Windows shows its firewall prompt. In fullscreen this can
  minimize the game - press Alt+Tab, click Allow, and switch back (or
  allow inbound rules for ce.exe, lobby.exe and iplist.exe beforehand).
- cneagle:// one-click-join links start working for your Windows user
  after the first time you run the game.
- There is no uninstaller: to remove the game, delete this folder.

See readme150.txt for what the 1.50 community patch changes.
EOF

mkdir -p "$(dirname "$out")"
out="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
rm -f "$out"
(cd "$stage" && zip -q -r -X "$out" "$root")
echo "built: $out"
