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
# already patched, so there is no binary-patch step here or at install time.
# The texture fixes are the one exception: the installer patches the player's
# own 24bits archives in place with textool.exe and the delta TGAs from
# game/full-overrides/24bits/, instead of shipping whole multi-MB archives. The
# three config files (default.cfg, keyconf.dat, menuinfo.dat), the two
# levels.nfo variants, the stock 24bits/texsec.dat (written only when the
# target install has none), and the demo menu/menupics.dat are staged
# separately because the installer writes them conditionally, not via the
# blanket File /r.
# (menupics.dat is written to demo installs only, to add back the menu textures
# the demo repack trimmed; a full-game install keeps its own complete copy.)
# The dgVoodoo files come from the repo's dgvoodoo/ dir and are staged
# separately too: they are an optional
# component (checked by default) the user can uncheck, so a dedicated section
# installs them instead of the blanket copy.
#
# RIPMUSIC_EXE (env) overrides the bundled soundtrack ripper; the default is
# the ripmusic crate's cargo-xwin release build. TEXTOOL_EXE (env) overrides
# the bundled texture-archive patcher; the default is the textool crate's
# mingw-w64 release build.
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

# NSIS VIProductVersion must be strictly numeric X.X.X.X. Derive it from the
# display version: fold a -beta.M suffix into a numeric field (1.50.0-beta.1 ->
# 1.50.0.1) and pad/truncate to exactly four fields (1.50 -> 1.50.0.0).
numeric="${version//-beta./.}"
IFS='.' read -ra viparts <<< "$numeric"
while [[ ${#viparts[@]} -lt 4 ]]; do viparts+=(0); done
viversion="${viparts[0]}.${viparts[1]}.${viparts[2]}.${viparts[3]}"
ripmusic_exe="${RIPMUSIC_EXE:-$repo/ripmusic/target/i686-pc-windows-msvc/release/ripmusic.exe}"
textool_exe="${TEXTOOL_EXE:-$repo/patch/textool/target/x86_64-pc-windows-gnu/release/textool.exe}"

if [[ $stage_only -eq 0 ]]; then
  command -v makensis >/dev/null || {
    echo "error: makensis not found (brew install makensis)" >&2
    exit 1
  }
fi
for f in common/ce.exe common/lobby.exe common/default.cfg \
  common/keyconf.dat common/menuinfo.dat \
  full/levels.nfo full/24bits/texsec.dat demo/levels.nfo demo/menu/menupics.dat \
  full-overrides/24bits/INTERFC1.tga full-overrides/24bits/snipemod32.tga \
  full-overrides/24bits/target32.tga; do
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
if [[ ! -f "$textool_exe" ]]; then
  echo "error: $textool_exe missing - build it with:" >&2
  echo '  cd patch/textool && rustup target add x86_64-pc-windows-gnu \' >&2
  echo '    && cargo build --release --target x86_64-pc-windows-gnu' >&2
  echo "(see patch/textool/README.md; needs mingw-w64)" >&2
  echo "or point TEXTOOL_EXE at an existing build" >&2
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
# texsec.dat is written conditionally too (only when the target install has
# none - stock 1.0 shipped without it), so pull it out of the blanket File /r
# and stage it on its own. An existing texsec.dat is the player's - textool
# patches it in place at install time, so texture mods survive.
mv "$full/24bits/texsec.dat" "$stage/texsec-stock.dat"
rmdir "$full/24bits" 2>/dev/null || true # drop the dir if that emptied it

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
  -DMENUPICS_DEMO="$game_dir/demo/menu/menupics.dat" \
  -DSTOCK_DIR="$here/stock" \
  -DLOWERCASE_PS1="$here/lowercase.ps1" \
  -DRIPMUSIC_EXE="$ripmusic_exe" \
  -DTEXTOOL_EXE="$textool_exe" \
  -DTEXSEC_STOCK="$stage/texsec-stock.dat" \
  -DTEX_INTERFC1="$game_dir/full-overrides/24bits/INTERFC1.tga" \
  -DTEX_SNIPEMOD="$game_dir/full-overrides/24bits/snipemod32.tga" \
  -DTEX_TARGET="$game_dir/full-overrides/24bits/target32.tga" \
  -DOUTFILE="$out" \
  -DVERSION="$version" \
  -DVIVERSION="$viversion" \
  "$here/installer.nsi"
echo "built: $out"
