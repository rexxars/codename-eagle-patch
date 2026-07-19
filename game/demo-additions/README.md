# Demo additions (pristine full-game content)

Files here are **pristine full-game assets** that Dafoosa's 1.43 MP-demo repack
dropped, re-added to the demo payload by a build script. Unlike
[`full-overrides/`](../full-overrides/) (our own authored edits), these are
verbatim copies of shipped full-game content - so they live here, not there.

| Source | Consumed by | What / why |
| --- | --- | --- |
| `menu/*.tga` | `scripts/build-demo-menupics.js` | Six menu-screen textures the demo's trimmed `menu/menupics.dat` is missing but the menu code still references, so their slots render blank in the demo: `c_chn16`, `c_dr3df`, `c_gfno`, `c_gfmid`, `kc_invX`, `jg_tmA`. Extracted verbatim from the full game's `MENU/menupics.dat` and appended back into the demo archive. |

The `.tga` files are standalone (viewable) textures - the engine's stored blob is
a TGA with its constant first 8 header bytes stripped, so the build re-strips that
prefix before appending. Regenerate the demo archive with:

```bash
node scripts/build-demo-menupics.js
```

The script is idempotent (it strips any existing copy of the six names before
re-adding them) and rewrites `game/demo/menu/menupics.dat` in place.

## Provenance

The six TGAs were extracted from a pristine full-game `MENU/menupics.dat` (1.41)
with [cnetool](https://www.npmjs.com/package/cnetool):

```bash
cnetool extract MENU/menupics.dat   # then copy the six c_*/kc_*/jg_* TGAs here
```
