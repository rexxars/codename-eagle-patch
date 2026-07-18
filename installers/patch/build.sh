#!/usr/bin/env bash
# Build the Codename Eagle 1.50 full-game patch installer (a Windows exe that
# upgrades an existing install to 1.50) from macOS or Linux.
#
# usage: ./build.sh [--stage-only] [version] [out.exe]
#   --stage-only  stage and verify the payload, print the staging dir and exit
#                 without running makensis; the staging dir is kept for
#                 inspection (clean it up yourself)
#   version       display version                              (default: 1.50)
#   out.exe       output path (default: out/codename-eagle-patch-<version>.exe)
#
# The payload is game/common/ (both variants) plus game/full/ (written only
# when the target is a full-game install), shipped as-is: the binaries are
# already patched, so there is no patch step here or at install time. The
# three config files (default.cfg, keyconf.dat, menuinfo.dat) and the two
# levels.nfo variants are staged separately because the installer writes them
# conditionally, not via the blanket File /r. The dgVoodoo files come from the
# repo's dgvoodoo/ dir and are staged separately too: they are an optional
# component (checked by default) the user can uncheck, so a dedicated section
# installs them instead of the blanket copy.
#
# RIPMUSIC_EXE (env) overrides the bundled soundtrack ripper; the default is
# the ripmusic crate's cargo-xwin release build.
#
# Requires makensis (brew install makensis).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(dirname "$(dirname "$here")")"
game_dir="$repo/game"
dgvoodoo_src="$repo/dgvoodoo"

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
out="${2:-$here/out/codename-eagle-patch-$version.exe}"
ripmusic_exe="${RIPMUSIC_EXE:-$repo/ripmusic/target/i686-pc-windows-msvc/release/ripmusic.exe}"

if [[ $stage_only -eq 0 ]]; then
  command -v makensis >/dev/null || {
    echo "error: makensis not found (brew install makensis)" >&2
    exit 1
  }
fi
for f in common/ce.exe common/lobby.exe common/default.cfg \
  common/keyconf.dat common/menuinfo.dat \
  full/levels.nfo demo/levels.nfo; do
  [[ -f "$game_dir/$f" ]] || {
    echo "error: $game_dir/$f missing" >&2
    exit 1
  }
done
# The patch exe's own icon. Not shipped in the payload (the game's icon is
# embedded in ce.exe); this is the source .ico for the NSIS wizard.
wizard_icon="$repo/patch/assets/ce.ico"
[[ -f "$wizard_icon" ]] || {
  echo "error: $wizard_icon missing" >&2
  exit 1
}
if [[ ! -f "$ripmusic_exe" ]]; then
  echo "error: $ripmusic_exe missing - build it with:" >&2
  echo '  cd ripmusic && PATH="$(brew --prefix llvm)/bin:$PATH" XWIN_ARCH=x86 \' >&2
  echo '    CL="-Wno-error=incompatible-pointer-types -Wno-incompatible-pointer-types" \' >&2
  echo '    cargo xwin build --release --target i686-pc-windows-msvc' >&2
  echo "(see ripmusic/.cargo/config.toml + ripmusic/README.md for details)" >&2
  echo "or point RIPMUSIC_EXE at an existing build" >&2
  exit 1
fi

stage="$(mktemp -d)"
if [[ $stage_only -eq 0 ]]; then
  trap 'rm -rf "$stage"' EXIT
fi
base="$stage/base"
full="$stage/full"
configs="$stage/configs"
dgvoodoo="$stage/dgvoodoo"

# base = game/common minus the three conditionally-written configs and the six
# optional dgVoodoo files;
# full = game/full minus the conditionally-picked levels.nfo ("src/." makes
# both BSD and GNU cp merge the contents into the existing dir).
mkdir "$base" "$full" "$configs" "$dgvoodoo"
cp -R "$game_dir/common/." "$base/"
cp -R "$game_dir/full/." "$full/"
for f in default.cfg keyconf.dat menuinfo.dat; do
  mv "$base/$f" "$configs/$f"
done
# dgVoodoo lives in the repo's dgvoodoo/ dir and ships as an optional component
# (checked by default): a dedicated section installs it and the user can uncheck
# it. base comes from game/common, which no longer holds dgVoodoo, so the main
# File /r over base never touches these files and there is no double install.
for f in dgVoodoo.conf dgVoodoo.txt dgVoodooCpl.exe D3D8.dll D3D9.dll D3DImm.dll DDraw.dll; do
  cp "$dgvoodoo_src/$f" "$dgvoodoo/$f"
done
rm "$full/levels.nfo"

# In stage-only mode, say where the kept dir is up front so a failed check
# below still tells you what to inspect.
if [[ $stage_only -eq 1 ]]; then
  echo "staged: $stage"
fi

# Refuse to ship git-lfs pointer stubs instead of the real data files.
stubs="$(cd "$stage" && grep -rl --include='*.dat' --include='*.bin' \
  'https://git-lfs.github.com/spec' . || true)"
if [[ -n "$stubs" ]]; then
  echo "error: git-lfs pointer files in payload - run 'git lfs pull':" >&2
  echo "$stubs" >&2
  exit 1
fi

# Strip mac cruft and stray backups so they don't ship to every player.
find "$stage" \( -name '.DS_Store' -o -name '*.bak' \) -delete

# Case-duplicate paths (foo.dat + FOO.DAT as two files, possible on a
# case-sensitive filesystem) would extract in nondeterministic order on
# Windows. base and full extract into the same target dir, so check their
# union; sort -u first drops the directories they legitimately share.
dupes="$({
  (cd "$base" && find .)
  (cd "$full" && find .)
} | sort -u | sort -f | uniq -di)"
if [[ -n "$dupes" ]]; then
  echo "error: case-duplicate payload paths:" >&2
  echo "$dupes" >&2
  exit 1
fi

if [[ $stage_only -eq 1 ]]; then
  exit 0
fi

mkdir -p "$(dirname "$out")"
makensis \
  -DPAYLOAD_BASE="$base" \
  -DPAYLOAD_FULL="$full" \
  -DWIZARD_ICON="$wizard_icon" \
  -DCONFIGS_DIR="$configs" \
  -DDGVOODOO_DIR="$dgvoodoo" \
  -DLEVELS_NFO_FULL="$game_dir/full/levels.nfo" \
  -DLEVELS_NFO_DEMO="$game_dir/demo/levels.nfo" \
  -DSTOCK_DIR="$here/stock" \
  -DLOWERCASE_PS1="$here/lowercase.ps1" \
  -DRIPMUSIC_EXE="$ripmusic_exe" \
  -DOUTFILE="$out" \
  -DVERSION="$version" \
  "$here/installer.nsi"
echo "built: $out"
