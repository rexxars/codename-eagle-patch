#!/usr/bin/env node
// Assemble game/full/: the files the full-game patcher must ship that are not
// already in game/common/. Sources from pristine 1.43 (official patch output).
// Skips: junk, music oggs, demo-variant files, community-repack extras, and
// level dirs (level128+ ship wholesale from game/common).
// Also crafts levels.nfo (Fever valley renumbered 248->134 for the 1.50 MP
// level table) and copies cemusic.dll from the patch/cemusic crate's release
// build (override the artifact path with CE_CEMUSIC_DLL).
import {createHash} from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'

import {buildTextureArchive, extractEntries, parseArchive} from 'cnetool'

const REPO = path.join(import.meta.dirname, '..')
const PRISTINE = process.env.CE_PRISTINE
if (!PRISTINE) {
  console.error('Set CE_PRISTINE to a directory containing pristine 1.0/ and 1.43/ installs')
  process.exit(1)
}
const FULL = path.join(REPO, 'game/full')

for (const version of ['1.0', '1.43']) {
  if (!fs.existsSync(path.join(PRISTINE, version))) {
    console.error(
      `Pristine reference install not found: ${path.join(PRISTINE, version)} - set CE_PRISTINE to a directory containing pristine 1.0/ and 1.43/ installs`,
    )
    process.exit(1)
  }
}

const JUNK =
  /(^|\/)((a|o|s|scr|tex|fac|w)cache\.(bin|dat)|hiscores\.dat|diacache\.dat|(lobby|error|chat)\.log|sg0\.dat|.*\.bak|\.ds_store)$/i

// Never part of an official patch (community repack additions) or superseded:
// url shortcuts (repo ships its own), uninstaller art, binaries and data
// whose canonical 1.50 versions ship from game/common, level dirs 128+
// (wholesale from game/common), levels.nfo (crafted below).
// The single-file entries deliberately overlap the common.has() check further
// down - preventing a file accidentally removed from game/common/ from silently
// leak into game/full/.
const SKIP = [
  /^music\//i,
  /\.url$/i,
  /^uninst/i,
  /^ce\.exe$/i,
  /^ce\.ico$/i,
  /^cespy\.ico$/i,
  /^iplist\.exe$/i,
  /^lobby\.exe$/i,
  /^data3\.bin$/i,
  /^data4\.bin$/i,
  /^mdata[34]\.bin$/i,
  /^objects2?\.dat$/i,
  /^menuinfo\.dat$/i,
  /^default\.cfg$/i,
  /^keyconf\.dat$/i,
  /^readme142\.txt$/i,
  /^level(12[89]|13[0-3]|248)\//i,
  /^levels\.nfo$/i,
]

function walk(dir, base = dir, out = new Map()) {
  for (const e of fs.readdirSync(dir, {withFileTypes: true})) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) walk(p, base, out)
    else if (e.isFile()) out.set(path.relative(base, p).toLowerCase(), p)
  }
  return out
}
const sha1 = (p) => createHash('sha1').update(fs.readFileSync(p)).digest('hex')

const p10 = walk(path.join(PRISTINE, '1.0'))
const p143 = walk(path.join(PRISTINE, '1.43'))
const common = walk(path.join(REPO, 'game/common'))

// cemusic.dll is not sourced from the pristine installs - it is built from
// the patch/cemusic crate. Without it the payload is incomplete, so refuse to
// run (before deleting anything) if the build artifact is missing.
const cemusicSrc =
  process.env.CE_CEMUSIC_DLL ??
  path.join(REPO, 'patch/cemusic/target/i686-pc-windows-msvc/release/cemusic.dll')
if (!fs.existsSync(cemusicSrc)) {
  console.error(
    `cemusic.dll build artifact not found: ${cemusicSrc} - build it first (from patch/cemusic/: XWIN_ARCH=x86 cargo xwin build --release --target i686-pc-windows-msvc) or point CE_CEMUSIC_DLL at a built cemusic.dll; regenerating without it would produce an incomplete payload`,
  )
  process.exit(1)
}
// Start clean so the output is exactly the computed set (no stale leftovers).
fs.rmSync(FULL, {recursive: true, force: true})
fs.mkdirSync(FULL, {recursive: true})
fs.copyFileSync(cemusicSrc, path.join(FULL, 'cemusic.dll'))
console.log('cemusic.dll (from patch/cemusic build)')

let copied = 0
for (const [rel, src] of [...p143.entries()].sort()) {
  if (JUNK.test(rel) || SKIP.some((re) => re.test(rel))) continue
  const in10 = p10.get(rel)
  if (in10 && sha1(in10) === sha1(src)) continue // untouched by any patch
  if (common.has(rel)) continue // 1.50 canonical version ships from common
  const dest = path.join(FULL, rel) // rel is already lowercased
  fs.mkdirSync(path.dirname(dest), {recursive: true})
  fs.copyFileSync(src, dest)
  copied++
  console.log(rel)
}

// Craft the full-game levels.nfo: SP 1-12 + MP 128-134. Same as pristine 1.43
// except Fever valley moves from its pre-1.50 slot 248 to 134 (the demo's MP
// table already uses 134). CRLF line endings, matching the original.
const nfoSrc = p143.get('levels.nfo')
if (!nfoSrc) {
  console.error(
    `levels.nfo not found in pristine 1.43 (${path.join(PRISTINE, '1.43')}) - cannot craft game/full/levels.nfo`,
  )
  process.exit(1)
}
const allLines = fs
  .readFileSync(nfoSrc, 'latin1')
  .split('\r\n')
  .filter((line) => line !== '')
const lines = allLines.filter((line) => !/Val:248$/.test(line))
const dropped = allLines.length - lines.length
if (dropped !== 1) {
  console.error(
    `expected exactly one 'Val:248' line in pristine 1.43 levels.nfo, found ${dropped} - refusing to craft a plausible-but-wrong file`,
  )
  process.exit(1)
}
lines.push('Name:Fever valley Val:134')
fs.writeFileSync(path.join(FULL, 'levels.nfo'), lines.join('\r\n') + '\r\n', 'latin1')
console.log('levels.nfo (crafted)')

// Authored overrides (game/full-overrides/): files that are NOT derivable from a
// pristine install - our own edits to shipped full-game assets. Applied last, on
// top of the pristine-derived payload. See game/full-overrides/README.md.
//  * level6/red.scr - the LEVEL6 player script, recompiled (via cnetool) to grant a
//    visible gas-mask inventory item when the professor's rescuer hands it over
//    (fix: "Demolition Man" mask was a spoken-only prop). red.scr is byte-identical
//    across 1.0-1.43, so a stock install already has the original; this overwrites it.
//  * 24bits/texsec.dat INTERFC1 cell 39 <- interfc1-cell39.bgra: the gas-mask HUD
//    icon, spliced into the one atlas cell that no item/weapon/vehicle/health icon
//    uses. Only cell 39's 32x32 pixels change; every other byte stays pristine.
//  * 24bits/texsec.dat SNIPEMOD.tga <- snipemod32.tga: the 32-bit smooth sniper
//    scope overlay, appended as a NEW entry. texsec.dat wins the engine's
//    archive-precedence search over textures.dat, so this overrides the stock
//    24-bit color-keyed SNIPEMOD without touching the user's textures.dat.
const OVR = path.join(REPO, 'game/full-overrides')
let overrides = 0

// 1. drop-in file overrides (mirror the payload layout)
const redSrc = path.join(OVR, 'level6/red.scr')
const redDest = path.join(FULL, 'level6/red.scr')
fs.mkdirSync(path.dirname(redDest), {recursive: true})
fs.copyFileSync(redSrc, redDest)
overrides++
console.log('override: level6/red.scr')

// 2. texsec INTERFC1 cell-39 pixel splice. Locate INTERFC1 via cnetool's
// parseArchive, verify its 10-byte blob header is the expected 256x256x32 before
// writing, then overwrite the 32x32 BGRA pixels of cell 39 (col 7, row 4) in place.
const CELL = {grid: 8, size: 32, col: 7, row: 4} // cell 39 = row*grid+col
const cell39 = fs.readFileSync(path.join(OVR, '24bits/interfc1-cell39.bgra'))
if (cell39.length !== CELL.size * CELL.size * 4) {
  console.error(
    `interfc1-cell39.bgra is ${cell39.length} bytes; expected ${CELL.size * CELL.size * 4}`,
  )
  process.exit(1)
}
const texsecPath = path.join(FULL, '24bits/texsec.dat')
const texsec = fs.readFileSync(texsecPath)
const tv = new DataView(texsec.buffer, texsec.byteOffset, texsec.byteLength)
const interfc1 = parseArchive(texsec).entries.find((e) =>
  e.name.toUpperCase().startsWith('INTERFC1'),
)
if (!interfc1) {
  console.error('INTERFC1 not found in texsec.dat TOC - refusing to splice the gas-mask icon')
  process.exit(1)
}
const blobOff = interfc1.dataOffset
// blob header: [u16 x, u16 y, u16 w, u16 h, u8 depth, u8 descriptor], then BGRA pixels
const w = tv.getUint16(blobOff + 4, true)
const h = tv.getUint16(blobOff + 6, true)
const depth = texsec[blobOff + 8]
if (w !== 256 || h !== 256 || depth !== 32) {
  console.error(`INTERFC1 blob is ${w}x${h}x${depth}, expected 256x256x32 - refusing to splice`)
  process.exit(1)
}
const pixels = blobOff + 10
const stride = w * 4
const x0 = CELL.col * CELL.size
const y0 = CELL.row * CELL.size
for (let dy = 0; dy < CELL.size; dy++) {
  const dst = pixels + (y0 + dy) * stride + x0 * 4
  cell39.copy(texsec, dst, dy * CELL.size * 4, (dy + 1) * CELL.size * 4)
}
overrides++
console.log('override: 24bits/texsec.dat (INTERFC1 cell 39 = gas-mask icon)')

// 3. texsec SNIPEMOD.tga + Target.tga appends. SNIPEMOD is the 32-bit
// smooth-alpha sniper-scope overlay (see game/full-overrides/README.md;
// regenerated by scripts/make-snipemod32.js). Target is the aiming crosshair as
// a centered 32x32 texture (stock Target.tga is 8x8 with off-center 7x7
// content); it pairs with the crosshair-scaling patch (fix #16), which draws it
// at a resolution-relative size. Both are new entries, so texsec wins the
// engine's archive-precedence search over the stock versions in textures.dat.
const snipemod = fs.readFileSync(path.join(OVR, '24bits/snipemod32.tga'))
const target = fs.readFileSync(path.join(OVR, '24bits/target32.tga'))

// Rebuild the archive from its current entries plus the two overrides.
// extractEntries yields each stored texture as a full TGA and buildTextureArchive
// re-strips the constant 8-byte prefix, so every existing entry (including the
// cell-39 splice above) round-trips byte-for-byte and the two new textures are
// appended in order.
const entries = extractEntries(texsec).map((e) => ({name: e.name, data: e.data}))
entries.push({name: 'SNIPEMOD.tga', data: snipemod})
entries.push({name: 'Target.tga', data: target})
fs.writeFileSync(texsecPath, buildTextureArchive(entries))
overrides++
console.log('override: 24bits/texsec.dat (SNIPEMOD.tga scope + Target.tga crosshair)')

console.error(
  `copied ${copied} files + crafted levels.nfo + ${overrides} overrides into game/full/`,
)
