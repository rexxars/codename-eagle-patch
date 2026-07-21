# Full-game authored overrides

Files here are **not** derivable from a pristine install. They are our own edits to
shipped full-game assets. `level6/red.scr` is applied by `scripts/build-full-payload.js`
**last**, on top of the pristine-derived `game/full/`, so it survives a payload
regeneration. The three texture overrides are **not** baked into the payload: the full
installer ships them alongside a pristine `texsec.dat` and runs
[`textool`](../../patch/textool/) at **install time** to patch the player's archives.
(Everything else in `game/full/` is exactly "pristine 1.43 minus what a base install
already has"; these are the exception.)

Layout mirrors the payload:

| Override | What / why |
| --- | --- |
| `level6/red.scr` | LEVEL6 ("Demolition Man") player script, recompiled to grant a **visible gas-mask inventory item** when the professor's rescuer hands the mask over. Stock only calls `REFGasMask` (attaches the worn model) + a sound, so the mask never appeared in the inventory (a spoken-only prop). |
| `24bits/INTERFC1.tga` | The full 256×256×32 `INTERFC1` inventory-icon atlas with the gas-mask HUD icon painted into **cell 39** (col 7, row 4). Cell 39 is the one atlas cell no item, weapon, vehicle, or health icon uses; every other pixel is byte-identical to the pristine 1.43 atlas. `textool` replaces `INTERFC1.tga` inside the player's `texsec.dat` at install time. |
| `24bits/snipemod32.tga` | The sniper-scope overlay regenerated as a **32-bit texture with real antialiased alpha** (stock is 24-bit, so its transparency is the engine's binary black color-key - hard staircase edges when upscaled). `textool` replaces `SNIPEMOD.tga` directly inside the player's `textures.dat` at install time. Demo installs get the same TGA via `game/demo/24bits/texsec.dat`, built by `scripts/build-demo-texsec.js`. |
| `24bits/target32.tga` | The aiming crosshair as a **centered 32×32** texture (original orange), replacing `Target.tga`. Stock `Target.tga` is 8×8 with its cross content in a 7×7 block, so its centre sits ~0.5 texel off; this replaces it with a crisp, exactly-centred 32×32. Pairs with the crosshair-scaling patch, which draws it at a resolution-relative size. `textool` replaces it in the player's `textures.dat` at install time; demo installs get it via `game/demo/24bits/texsec.dat`. |
| `src/red.scr.txt` | The cnetool script source `level6/red.scr` was compiled from (provenance). |
| `src/interfc1.png` | The full mask atlas (256×256) `INTERFC1.tga` encodes, in human orientation (provenance/QC). |
| `src/snipemod32.png` | The human-oriented QC image `snipemod32.tga` was encoded from (provenance). |
| `src/target32.png` | The 32×32 crosshair `target32.tga` was encoded from. Regenerate the TGA with cnetool's `pngToTga(png, {topDown: true})`. |

## How the mask shows up in-game

`red.scr`'s `DelayedGasMask()` (reached via `oldman.PlayS3OFCOUR` → `red.AddGasMask` →
2 s delay) keeps the original `REFGasMask(MYSELF, 1)` + `REFPlayFX`, and now also:

```
REFGetProject(g0, "gasmask", 0);   // the gasmask project already exists in objects.dat
REFSetItemTextureNr(g0, 39);       // point its inventory icon at atlas cell 39
REFSetProjectVars(g0, ITEM, ON);   // mark it a carryable item (idcard-style, no WEAPON_TYPE)
REFAddItem(MYSELF, g0);            // hand it to the player (MYSELF = the player in red.scr)
```

No `WEAPON_TYPE`, so it is a passive, non-usable "you got the mask" token, exactly like the
ID card. The engine's HUD atlas handle is loaded once per process and cached, so the icon
edit is necessarily **global** (`texsec.dat`, which wins the archive-precedence search over
`textures.dat`), but cell 39 is drawn only when something carries an item whose icon index
is 39, which happens only here in LEVEL6.

## Regenerating the override artifacts (needs cnetool)

These are prebuilt and committed (like the bundled `menudll.dll`/`cemusic.dll`). They are
regenerated with [cnetool](https://www.npmjs.com/package/cnetool), which is not a runtime
dependency of this repo:

**`level6/red.scr`**: decompile pristine `LEVEL6/red.scr`, insert the four calls above into
`DelayedGasMask()`, recompile with `compileScript`. The recompile is not byte-identical to
pristine (the compiler lowers multiple `return`s to a single exit and normalizes handler-name
case), but it is behaviorally identical: handler lookup is case-insensitive (`ce.exe`
`0x49ff30` uppercases both sides) and the control-flow lowering is equivalent.

**`24bits/INTERFC1.tga`**: extracted (8-byte constant TGA prefix + stored blob) from the
fixed `texsec.dat` this repo previously committed, which was pristine 1.43 plus a 32×32
cell-39 splice of the gas-mask icon cut from `src/interfc1.png` (the splice source,
`interfc1-cell39.bgra`, is in git history). Verified: byte-identical to the pristine 1.43
`INTERFC1.tga` outside cell 39 (x 224–255, y 128–159). The TGA is in **engine row order**
(the blob stores rows top-down behind a bottom-origin descriptor, the same orientation as
`src/interfc1.png`, so no flip).

**`24bits/snipemod32.tga`**: generated by `scripts/make-snipemod32.js` from a stock
`textures.dat`:

```
node scripts/make-snipemod32.js <stock-textures.dat> src/snipemod32.png 24bits/snipemod32.tga
```

The script recovers the lens shape from the stock 24-bit texture (the arc is a fitted
ellipse, the artist's 320×240 screen-space circle squeezed by the 4:3 aspect; ~0.23px mean
residual), measures the reticle bars/ticks from the stock pixels, and re-renders everything
with supersampled coverage AA, except the four graduation ticks, which stay stock-exact and
hard-edged on purpose. The TGA is in **engine row order** (cnetool `pngToTga({topDown: true})`),
ready to insert verbatim; the PNG is the same image in human orientation for QC.
