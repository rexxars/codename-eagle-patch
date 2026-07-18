# game/ payload split

The three directories here are named by **which deliverable ships them**, not
by variant lineage:

- `common/`: shipped by **both** the MP-demo deliverables (installer, zip,
  docker image) and the full-game patcher.
- `demo/`: shipped by the MP-demo deliverables (installer, zip, docker
  image) **only**.
- `full/`: shipped by the full-game patcher **only**.

So a file's directory tells you where it goes, not where it came from. A file
that originated in the demo repack can live in `common/` if the full game
needs it too.

## Provenance

`common/` is the 1.50 work: pre-patched binaries produced by the `patch/`
tool (a development tool that is never run on user machines; users receive
the already-patched files), the MP levels `level128`–`level134`, and patch-added
files both variants share. It also carries `psapi.dll`, Microsoft's Win9x-era
PSAPI redistributable, absent from official installs, inherited from Dafoosa's
demo repack, and kept for compatibility on old systems. Three config files live
here too, but are special-cased by the patcher: `default.cfg` and `keyconf.dat`
are written only if they are absent or still match a known factory-stock
version; `menuinfo.dat` (the saved profile, which carries single-player campaign
progress, so refreshing even a stock one could reset a returning player) is
written only if absent, so customized configs and any existing profile survive
patching. The bundled `menuinfo.dat` is a clean default profile: name "CEDemo",
1024×768, and **no campaign progress** (`LevelsDone` all zero), so a fresh
install starts the single-player campaign from the beginning.

The dgVoodoo graphics wrapper ships with every deliverable too, but lives in the
top-level [`dgvoodoo/`](../dgvoodoo/) directory, not here, so it can be updated
as a drop-in. Its `dgVoodoo.conf` is likewise written only if absent.

`demo/` holds the files inherited from Dafoosa's 1.43 MP demo repack that the
full game either already has or has its own variant of: the demo-trimmed
`24bits/textures.dat` (the full 151 MB original was never touched by any
patch, so the patcher never ships textures), the demo `Game.exe`, `menu/`
assets, the demo `levels.nfo` (MP levels 128–134 only), and the stock
animations, sounds and dialogue the demo carries unchanged from 1.0. One
exception is authored, not inherited: `24bits/texsec.dat`, a small texture
archive carrying the full-game texture overrides that apply to the demo too -
the 32-bit smooth sniper-scope overlay and the centered 32×32 crosshair (the
demo never had a texsec.dat, but the engine probes for it and searches it
before `textures.dat`, so it overrides the stock scope and crosshair without a
modified `textures.dat`). Regenerate it with `node scripts/build-demo-texsec.js`;
the textures themselves live in [`full-overrides/`](full-overrides/).

One cache file ships on purpose, the single exception to the rule that caches
are runtime junk the game rebuilds by itself: `common/level133/wcache.bin`,
the stock water/land pairing cache that Refraction built in August 2000
(byte-identical in pristine 1.42 and 1.43). Fortress cannot rebuild it: its
stock terrain has 19 spots where two land faces or two water faces overlap,
and `InitWater` treats those as a fatal error while building the cache, so a
fresh install without the file exits on load with `two land faces or two sea
faces, nErrors=19`. Every stock install shipped this cache, which is why the
bad terrain never surfaced. It only describes the terrain mesh (`oldbf`),
which 1.50 leaves untouched, so the stock file is still correct. All other
caches (`*cache.bin`, `diacache.dat`) rebuild cleanly and must not ship.

`full/` is what a full-game install needs on top of `common/`: the official
1.0→1.43 patch delta sourced from a pristine 1.43 install (single-player
level fixes for `level1`–`level12`, `24bits/texsec.dat`, the 1.43 `game.exe`
launcher, patch-added sounds the demo lacks), plus two crafted
files: the full-game `levels.nfo`, which renumbers Fever valley from its
pre-1.50 slot 248 to 134 so the MP level table matches across variants, and
`cemusic.dll`, the file-based music playback used by full-game installs.

A handful of `full/` files are **authored overrides**, not pristine-derived: our
own edits to shipped assets, kept in [`full-overrides/`](full-overrides/) and
applied last by the build script (see below). Currently the LEVEL6 gas-mask
inventory item, a recompiled `level6/red.scr` plus a one-cell splice into
`24bits/texsec.dat`'s `INTERFC1` HUD atlas. See
[`full-overrides/README.md`](full-overrides/README.md).

## What is deliberately nobody's payload

Music (`music/*.ogg`) and cutscenes (`cutscn/`) never ship in any deliverable:
the oggs are CD rips that were never official patch content, and we do not
distribute them. Users rip music from their own CD with `ripmusic.exe` and
copy the cutscene folder themselves.

## Regeneration

`game/full/` can be regenerated with `node scripts/build-full-payload.js`
(requires pristine 1.0 and 1.43 installs; set `CE_PRISTINE` to a directory
containing `1.0/` and `1.43/` subdirectories). The script also re-crafts
`levels.nfo`, preserves the committed `cemusic.dll`, and applies the authored
overrides in [`full-overrides/`](full-overrides/) last (drop-in files plus the
`texsec.dat` gas-mask-icon splice).

`scripts/classify-game-files.js` is the one-shot tool that produced the
`common/`/`demo/` split from the pre-split layout. It is kept for historical
provenance only. Do not re-run it.
