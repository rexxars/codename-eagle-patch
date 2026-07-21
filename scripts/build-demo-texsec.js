#!/usr/bin/env node
// Build game/demo/24bits/texsec.dat: a texture archive carrying the full-game
// texture overrides that apply to demo installs too - the 32-bit smooth
// sniper-scope overlay (SNIPEMOD.tga) and the centered 32x32 aiming crosshair
// (Target.tga).
//
// The demo has no texsec.dat of its own (it is a full-game addition the demo
// repack never carried), but the engine unconditionally probes for it and
// searches it BEFORE textures.dat in the by-name texture lookup - so this tiny
// archive overrides the stock 24-bit SNIPEMOD / 8x8 Target inside the demo's
// textures.dat without shipping a modified copy of that 134 MB file. Full-game
// installs get the same overrides patched into their textures.dat by textool
// at install time.
//
// The TGAs are the authored artifacts in game/full-overrides/ (provenance and
// regeneration: game/full-overrides/README.md).
import fs from 'node:fs'
import path from 'node:path'

import {buildTextureArchive} from 'cnetool'

const REPO = path.join(import.meta.dirname, '..')
const ovr = (f) => fs.readFileSync(path.join(REPO, 'game/full-overrides/24bits', f))
const dest = path.join(REPO, 'game/demo/24bits/texsec.dat')
fs.mkdirSync(path.dirname(dest), {recursive: true})
fs.writeFileSync(
  dest,
  buildTextureArchive([
    {name: 'SNIPEMOD.tga', data: ovr('snipemod32.tga')},
    {name: 'Target.tga', data: ovr('target32.tga')},
  ]),
)
console.log(`wrote ${dest} (2 entries: SNIPEMOD.tga, Target.tga)`)
