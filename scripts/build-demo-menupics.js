#!/usr/bin/env node
// Rebuild game/demo/menu/menupics.dat: the demo's menu-screen texture archive
// with the full-game menu textures the demo repack dropped added back in.
//
// Dafoosa's 1.43 MP demo repack shipped a trimmed menupics.dat (74 entries) that
// is missing a handful of textures the menu code still references, so those slots
// render blank in the demo. This adds the missing full-game textures back:
//
//   c_chn16, c_dr3df, c_gfno, c_gfmid, kc_invX, jg_tmA
//
// menupics.dat is a plain named-blob archive (parseArchive/buildArchive), and
// each entry's stored blob is the engine's internal texture format - a standard
// TGA with its constant first 8 header bytes stripped. The source TGAs in
// game/demo-additions/menu/ are standalone (viewable) files extracted verbatim
// from the full game's MENU/menupics.dat; we strip that 8-byte prefix here to
// recover the stored blob and append it. See game/demo-additions/README.md.
//
// The additions are pristine full-game content, not authored edits, so they are
// NOT in game/full-overrides/ (which is authored overrides only).
//
// Idempotent: it strips any existing copy of the six names before re-adding them,
// so it converges to the same bytes whether run on the trimmed original or on an
// already-augmented file.
import fs from 'node:fs'
import path from 'node:path'

import {parseArchive, extractFile, buildArchive} from 'cnetool'

// The constant first 8 bytes cnetool strips from a stored blob to make a valid
// standalone TGA (imagetype 2, empty id/colormap). We re-strip it to go back.
const TGA_PREFIX = Uint8Array.from([0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00])

// Names to add, in the order they should be appended (matches the tested build).
const ADDITIONS = ['c_chn16', 'c_dr3df', 'c_gfno', 'c_gfmid', 'kc_invX', 'jg_tmA'].map(
  (n) => `${n}.tga`,
)

const REPO = path.join(import.meta.dirname, '..')
const dest = path.join(REPO, 'game/demo/menu/menupics.dat')
const srcDir = path.join(REPO, 'game/demo-additions/menu')

// Recover the stored blob from a standalone source TGA by stripping the 8-byte prefix.
function storedBlob(name) {
  const tga = new Uint8Array(fs.readFileSync(path.join(srcDir, name)))
  if (!TGA_PREFIX.every((b, i) => tga[i] === b)) {
    throw new Error(`${name}: unexpected TGA header prefix, refusing to strip`)
  }
  return tga.subarray(TGA_PREFIX.length)
}

const base = new Uint8Array(fs.readFileSync(dest))
const archive = parseArchive(base)

const additionSet = new Set(ADDITIONS.map((n) => n.toLowerCase()))
const entries = archive.entries
  // Drop any existing copy of the additions so re-runs stay idempotent.
  .filter((e) => !additionSet.has(e.name.toLowerCase()))
  .map((e) => ({name: e.name, data: extractFile(base, e)}))

for (const name of ADDITIONS) {
  entries.push({name, data: storedBlob(name)})
}

const out = buildArchive(entries)
fs.writeFileSync(dest, out)
console.log(
  `wrote ${dest} (${entries.length} entries, +${ADDITIONS.length}: ${ADDITIONS.join(', ')})`,
)
