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
  /^smackw32\.dll$/i,
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

// cevideo's smackw32.dll is the drop-in Smacker shim (plays transcoded AV1 WebM
// cutscenes; see patch/cevideo). Like cemusic.dll it is built from its crate, not
// sourced from a pristine install, so refuse to run (before deleting anything) if
// the build artifact is missing. Override the artifact path with CE_CEVIDEO_DLL.
const cevideoSrc =
  process.env.CE_CEVIDEO_DLL ??
  path.join(REPO, 'patch/cevideo/target/i686-pc-windows-msvc/release/smackw32.dll')
if (!fs.existsSync(cevideoSrc)) {
  console.error(
    `smackw32.dll (cevideo) build artifact not found: ${cevideoSrc} - build it first (from patch/cevideo/: XWIN_ARCH=x86 cargo xwin build --release --target i686-pc-windows-msvc) or point CE_CEVIDEO_DLL at a built smackw32.dll; regenerating without it would produce an incomplete payload`,
  )
  process.exit(1)
}

// The cevideo shim forwards any un-shimmed Smacker calls to the stock DLL, which
// it loads as smackw32_orig.dll. The stock SMACKW32.DLL is byte-identical across
// 1.0-1.43, so we ship the copy already committed at game/demo/smackw32.dll
// rather than depending on a pristine install for it. (The main copy loop below
// skips smackw32.dll, so the shim written above is never clobbered.)
const stockSmackw32 = path.join(REPO, 'game/demo/smackw32.dll')
if (!fs.existsSync(stockSmackw32)) {
  console.error(
    `stock smackw32.dll missing at ${stockSmackw32} - needed as smackw32_orig.dll for the cevideo shim to forward to`,
  )
  process.exit(1)
}

// Start clean so the output is exactly the computed set (no stale leftovers).
fs.rmSync(FULL, {recursive: true, force: true})
fs.mkdirSync(FULL, {recursive: true})
fs.copyFileSync(cemusicSrc, path.join(FULL, 'cemusic.dll'))
console.log('cemusic.dll (from patch/cemusic build)')
fs.copyFileSync(cevideoSrc, path.join(FULL, 'smackw32.dll'))
console.log('smackw32.dll (cevideo shim, from patch/cevideo build)')
fs.copyFileSync(stockSmackw32, path.join(FULL, 'smackw32_orig.dll'))
console.log('smackw32_orig.dll (stock Smacker DLL, from game/demo)')

// Transcoded cutscenes: cutscn/*.webm (AV1 + Vorbis), produced by
// scripts/transcode-cutscenes.js from the CD's CUTSCN folder. These are
// copyrighted game content transcoded at build time, never committed and not
// present in a pristine install, so they are opt-in: point CE_CUTSCN_WEBM at the
// transcode output dir to bundle them. Without it the payload still builds (the
// shim just has no videos to play, same as a stock install with no cutscenes
// copied in). The shim's subtitle sync relies on transcode-cutscenes.js having
// preserved each clip's frame count and fps 1:1.
const cutscnSrc = process.env.CE_CUTSCN_WEBM
if (cutscnSrc) {
  if (!fs.existsSync(cutscnSrc) || !fs.statSync(cutscnSrc).isDirectory()) {
    console.error(`CE_CUTSCN_WEBM is set but is not a directory: ${cutscnSrc}`)
    process.exit(1)
  }
  const webms = fs
    .readdirSync(cutscnSrc)
    .filter((name) => name.toLowerCase().endsWith('.webm'))
    .sort()
  if (webms.length === 0) {
    console.error(`CE_CUTSCN_WEBM directory has no .webm files: ${cutscnSrc}`)
    process.exit(1)
  }
  const cutscnDest = path.join(FULL, 'cutscn')
  fs.mkdirSync(cutscnDest, {recursive: true})
  for (const name of webms) {
    fs.copyFileSync(path.join(cutscnSrc, name), path.join(cutscnDest, name.toLowerCase()))
  }
  console.log(`cutscn/ (${webms.length} transcoded .webm cutscene(s) from CE_CUTSCN_WEBM)`)
} else {
  console.log('cutscn/ skipped (set CE_CUTSCN_WEBM to a transcode output dir to bundle cutscenes)')
}

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
// The texture overrides (INTERFC1.tga, snipemod32.tga, target32.tga) are NOT baked
// into the payload: texsec.dat ships pristine, and the installer runs patch/textool
// at install time to patch the player's texsec.dat/textures.dat with them.
const OVR = path.join(REPO, 'game/full-overrides')
let overrides = 0

// drop-in file overrides (mirror the payload layout)
const redSrc = path.join(OVR, 'level6/red.scr')
const redDest = path.join(FULL, 'level6/red.scr')
fs.mkdirSync(path.dirname(redDest), {recursive: true})
fs.copyFileSync(redSrc, redDest)
overrides++
console.log('override: level6/red.scr')

console.error(
  `copied ${copied} files + crafted levels.nfo + ${overrides} overrides into game/full/`,
)
