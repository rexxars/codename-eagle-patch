#!/usr/bin/env bash
# Build the Codename Eagle multiplayer-demo installer (a Windows setup exe)
# from macOS or Linux.
#
# usage: ./build.sh [--stage-only] [version] [out.exe]
#   --stage-only  stage and verify the payload, print the staging dir and exit
#                 without running makensis; the staging dir is kept for
#                 inspection (clean it up yourself)
#   version       display version for Add/Remove Programs     (default: 1.50)
#   out.exe       output path (default: out/codename-eagle-mp-demo-<version>-setup.exe)
#
# The payload is the repo's game/common/ overlaid with game/demo/, shipped
# as-is: the binaries in it are already patched (ce.exe with the master-server
# redirect and cneagle:// handler, menudll.dll, iplist.exe, ...), so there is no
# patch step here or at install time. dgVoodoo is added from the repo's
# dgvoodoo/ dir as an optional component.
#
# Requires makensis (brew install makensis).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(dirname "$(dirname "$here")")"
game_dir="$repo/game"
dgvoodoo_src="$repo/dgvoodoo"
# The setup/uninstall exe's own icon. Not shipped in the payload (the game's
# icon is embedded in ce.exe); this is the source .ico for the NSIS wizard.
wizard_icon="$repo/patch/assets/ce.ico"

stage_only=0
if [[ "${1:-}" == "--stage-only" ]]; then
  stage_only=1
  shift
fi

if [[ "${1:-}" == -* ]]; then
  echo "error: unknown flag: $1" >&2
  echo "usage: $0 [--stage-only] [version] [out.exe]" >&2
  exit 2
fi

version="${1:-1.50}"
out="${2:-$here/out/codename-eagle-mp-demo-$version-setup.exe}"

# NSIS VIProductVersion must be strictly numeric X.X.X.X. Derive it from the
# display version: fold a -beta.M suffix into a numeric field (1.50.0-beta.1 ->
# 1.50.0.1) and pad/truncate to exactly four fields (1.50 -> 1.50.0.0).
numeric="${version//-beta./.}"
IFS='.' read -ra viparts <<< "$numeric"
while [[ ${#viparts[@]} -lt 4 ]]; do viparts+=(0); done
viversion="${viparts[0]}.${viparts[1]}.${viparts[2]}.${viparts[3]}"

if [[ $stage_only -eq 0 ]]; then
  command -v makensis >/dev/null || {
    echo "error: makensis not found (brew install makensis)" >&2
    exit 1
  }
fi
for f in ce.exe lobby.exe; do
  [[ -f "$game_dir/common/$f" ]] || {
    echo "error: $game_dir/common/$f missing" >&2
    exit 1
  }
done
[[ -f "$wizard_icon" ]] || {
  echo "error: $wizard_icon missing" >&2
  exit 1
}

stage="$(mktemp -d)"
if [[ $stage_only -eq 0 ]]; then
  trap 'rm -rf "$stage"' EXIT
fi
payload="$stage/payload"
dgvoodoo="$stage/dgvoodoo"

# The demo payload is common/ with demo/ overlaid on top ("src/." makes both
# BSD and GNU cp merge the contents into the existing payload dir).
mkdir "$payload" "$dgvoodoo"
cp -R "$game_dir/common/." "$payload/"
cp -R "$game_dir/demo/." "$payload/"

# dgVoodoo lives in the repo's dgvoodoo/ dir and ships as an optional component
# in the installer (checked by default), staged on its own. The installer's main
# File /r copies the payload, and a dedicated NSIS section installs the dgVoodoo
# files, so unchecking the component leaves them out with no double install. The
# demo zip pulls this dir back in, so extract-and-play always ships dgVoodoo.
for f in dgVoodoo.conf dgVoodoo.txt dgVoodooCpl.exe D3D8.dll D3D9.dll D3DImm.dll DDraw.dll; do
  cp "$dgvoodoo_src/$f" "$dgvoodoo/$f"
done

# In stage-only mode, say where the kept dir is up front so a failed check
# below still tells you what to inspect.
if [[ $stage_only -eq 1 ]]; then
  echo "staged: $stage"
fi

# Refuse to ship git-lfs pointer stubs instead of the real data files.
stubs="$(cd "$payload" && grep -rl --include='*.dat' --include='*.bin' \
  'https://git-lfs.github.com/spec' . || true)"
if [[ -n "$stubs" ]]; then
  echo "error: git-lfs pointer files in payload - run 'git lfs pull':" >&2
  echo "$stubs" >&2
  exit 1
fi

# Strip mac cruft and stray backups so they don't ship to every player.
find "$stage" \( -name '.DS_Store' -o -name '*.bak' \) -delete

# Case-duplicate paths (foo.dat + FOO.DAT as two files, possible on a
# case-sensitive filesystem) would extract in nondeterministic order on Windows.
dupes="$(cd "$payload" && find . | sort -f | uniq -di)"
if [[ -n "$dupes" ]]; then
  echo "error: case-duplicate payload paths:" >&2
  echo "$dupes" >&2
  exit 1
fi

if [[ $stage_only -eq 1 ]]; then
  exit 0
fi

mkdir -p "$(dirname "$out")"
makensis -DPAYLOAD_DIR="$payload" -DDGVOODOO_DIR="$dgvoodoo" \
  -DWIZARD_ICON="$wizard_icon" \
  -DOUTFILE="$out" -DVERSION="$version" -DVIVERSION="$viversion" \
  "$here/installer.nsi"
echo "built: $out"
