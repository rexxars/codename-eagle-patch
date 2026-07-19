# Codename Eagle 1.50 full-game patch installer

A classic Windows setup wizard that upgrades an **existing** Codename Eagle
installation - any version from 1.0 to 1.43, or the old multiplayer demo - to
1.50 in one hop, built entirely from macOS/Linux with
[NSIS](https://nsis.sourceforge.io/) (`brew install makensis`).

It never ships the game itself: the payload is this repo's `game/common/`
folder (both variants) plus `game/full/` (written only when the target is a
full-game install) - roughly 250 files, all pre-patched. See `game/README.md`
for the payload split semantics.

Naming note: the repo also has a top-level `patch/` directory. That one is the
**development tool** that produced the patched binaries in `game/` - it is
never run on user machines. `installers/patch/` (this directory) is the
shippable exe that delivers those binaries.

## Build

```bash
./build.sh                # -> out/codename-eagle-patch-1.50.exe
./build.sh 1.50 out.exe   # explicit version + output path
```

The script stages `../../game/common` (minus the three config files and the six
dgVoodoo files, which the installer writes conditionally) and `../../game/full`
(minus `levels.nfo`, which the installer picks per variant), strips
`.DS_Store`/`*.bak`, refuses to ship git-lfs pointer stubs (run `git lfs pull`
first) or case-duplicate paths, and compiles `installer.nsi`. The six dgVoodoo
files are staged into their own dir so the installer can offer them as an
optional component; the main file copy skips them. Pass `--stage-only` as the
first argument to stage and verify the payload without running makensis (the
staging dir is printed and kept for inspection).

The bundled soundtrack ripper defaults to the ripmusic crate's release build
(`../../ripmusic/target/i686-pc-windows-msvc/release/ripmusic.exe`); set the
`RIPMUSIC_EXE` environment variable to use a different build. If it is missing,
the script prints the exact `cargo xwin` command to produce it.

## What the patch exe does

1. Refuses to proceed until the chosen folder contains a Codename Eagle
   installation (`Game.exe` + `dialogue.dat`) - the Next button stays disabled
   otherwise, and the same check is re-run at the start of the install step,
   so a silent install (`/S /D=dir`, which skips all wizard pages) against a
   non-game folder aborts before touching anything. This patch does not
   contain the game; there is nothing it can do with an empty folder. If the
   MP demo installer's registry entry exists, its install location is used as
   the default folder.
2. Detects the variant: `level1\` present means a full-game install; absent
   means the old multiplayer demo (which then skips the single-player payload
   and gets an MP-only `levels.nfo`, so no phantom level entries appear, plus
   the fixed `menu/menupics.dat` that adds back the menu textures the demo
   repack trimmed - a full-game install keeps its own complete copy).
3. Lowercases ALL_CAPS file and directory names left behind by old installers
   (`LEVEL1`, `GAME.EXE`, ...) via a bundled PowerShell script
   (`lowercase.ps1`). Cosmetic on Windows; failures only produce a warning.
   On re-runs (or over a demo install) this also lowercases previously
   installed mixed-case payload names (`D3D8.dll` -> `d3d8.dll`,
   `dgVoodoo.conf` -> `dgvoodoo.conf`, ...) - harmless on Windows's
   case-insensitive filesystems, and accepted.
4. Deletes stale regeneratable junk: `*cache.bin`/`*cache.dat` in every stock
   level folder, `diacache.dat`, `lobby.log`, `player*.txt`, `*.bak`, the three
   `level128` scripts 1.50 removed from No Mans Land (`cactus1.scr`,
   `cactuss.scr`, `switch1.scr`), and the whole `level248\` folder (Fever
   valley is `level134` as of 1.50). It never touches user data:
   `hiscores.dat`, savegames, screenshots.
5. Overwrites with the 1.50 payload (`common`, plus `full` and the full-game
   `levels.nfo` on full installs).
6. Handles the config files separately: `keyconf.dat` and `default.cfg` are
   written if absent, refreshed if still byte-identical to a known factory-stock
   version (so a 1.0 install gets current keybinds), and **left alone if
   customized**. `menuinfo.dat` (the saved profile - it holds single-player
   campaign progress, so refreshing even a stock one could reset a returning
   player) is written only if absent - an existing one is always preserved.
7. Drops `ripmusic.exe` in the game folder so the soundtrack rip can be run
   later at any time.
8. Adds the three Windows Firewall allow-rules and the machine-wide `cneagle://`
   protocol registration - the same rules/keys as the MP demo installer (it is
   the same game; this is why the patcher requests elevation).

It writes **no uninstaller and no Add/Remove Programs entry** - it patches an
installation it does not own.

### dgVoodoo component

The components page offers the **dgVoodoo graphics wrapper (recommended)** as a
separate component, checked by default. It bundles the six dgVoodoo files
(`dgVoodooCpl.exe`, `D3D8.dll`, `D3D9.dll`, `D3DImm.dll`, `DDraw.dll` and
`dgVoodoo.conf`), which fix rendering problems on modern Windows and make
options like anti-aliasing easy to turn on. Unchecking it installs none of the
six. When it is checked, `dgVoodoo.conf` is written only if it is absent, so a
config you tuned earlier is preserved. It also installs a `dgVoodoo.txt` notice
that explains what dgVoodoo is, where it comes from, and how to remove it.

### Optional CD steps

Two unchecked components need the Codename Eagle CD; both are skippable and
have manual equivalents documented in `readme150.txt`:

- **Rip CD soundtrack**: runs `ripmusic.exe <gamedir>`, producing
  `music\trackNN.ogg` files that `cemusic.dll` plays instead of CD audio.
  Manual equivalent: run `ripmusic.exe` in the game folder later.
- **Copy cutscenes (~215 MB)**: scans CD drives for a `cutscn\` folder with
  the game's `.smk` videos and copies it into the game folder. Manual
  equivalent: copy the CD's `cutscn\` folder yourself.

## Testing

Real Windows only - the interesting behavior (fc-based config comparison,
PowerShell renames, netsh, CD scanning) means nothing under Wine. Point it at
copies of pristine installs; a patched-from-1.0 tree should match a
patched-from-1.43 tree file-for-file (except preserved user configs).

### Manual test checklist (Windows)

Run against **copies** of pristine installs, never the originals:

1. **Pristine 1.0**: run the patcher, then compare the resulting tree
   file-for-file against a patched-from-1.43 copy - they must match except for
   the preserved config files. Boot single-player level 1; save a game and
   load it back (this exercises the new `saves\` redirect).
2. **Pristine 1.43 with a customized `keyconf.dat`**: run the patcher - the
   keybinds must survive, the `*cache.bin`/`*cache.dat` files must be gone,
   `level248\` must be gone, the three stray `level128` scripts
   (`cactus1.scr`, `cactuss.scr`, `switch1.scr`) must be gone, and Fever
   valley must play as `level134`. The ALL-CAPS names must be lowercase -
   directories too (`LEVEL1` -> `level1`, `ANM`, `GLOBAL`, `MENU`, `SOUNDS`),
   with no `*.lc-tmp` leftovers.
3. **Old MP demo install**: run the patcher - no single-player levels may
   appear, and `levels.nfo` must stay MP-only (levels 128-134).
4. **Re-run on an already-patched install**: must be idempotent - customized
   configs and `dgVoodoo.conf` survive a second pass.
5. **Optional CD steps** (with a CD or mounted ISO): the soundtrack rip must
   produce `music\trackNN.ogg` files and in-game music must play without the
   CD; the cutscene copy must produce `cutscn\` and the intro must play.
6. **Silent mode**: `/S /D=C:\some\empty\dir` must abort without touching the
   directory (the non-game-folder gate).
7. **Browse button**: on the directory page, Browse to a game folder whose name
   is not "Codename Eagle" (e.g. `C:\Games\CE`) - the picked folder must be
   used verbatim (no `\Codename Eagle` appended) and Next must enable.
8. **Fire-cooldown fix ("8 trick")**: in a hosted game, fire the bazooka and
   immediately re-select it (key 8) - the next shot must only be possible ~2 s
   after the previous one (unpatched: ~1.2 s). Bazooka -> pistol still fires the
   moment it is up; pistol shot -> bazooka waits ~2 s from the pistol shot. Two
   patched clients in a lobby with one attempting the trick must stay in sync
   (no ghost rockets, no `Error:` line in `error.log`).
9. **Demo installer smoke test**: a fresh MP demo install boots to the
   multiplayer menu and the server browser lists internet servers.

## Caveat

The patch exe is unsigned, so SmartScreen shows "Windows protected your PC" on
first download ("More info" -> "Run anyway"). Code signing is a separate
project.
