#!/usr/bin/env node
// Patch game/demo/menu/menupics.dat: the multiplayer demo's menu-screen texture
// archive.
//
// The official Codename Eagle multiplayer demo (going back to the 1.33 MP demo)
// ships a trimmed menupics.dat - 74 entries, missing six menu textures the menu
// code still references, so those slots render blank. Dafoosa's 1.43 repack
// (the demo this patch builds on) inherited that same trimmed archive; the
// trimming is the official demo's, not his. This adds the missing full-game
// textures back:
//
//   c_chn16, c_dr3df, c_gfno, c_gfmid, kc_invX, jg_tmA
//
// and replaces the menu font (menufont) with the one carrying the extra
// name-punctuation glyphs, so player and server names with punctuation display
// correctly in the demo's menus.
//
// Unlike the full game - where the ~120 MB menupics.dat is patched on the
// player's machine at install time - the demo SHIPS its menupics.dat, so it is
// patched here in the repo and shipped as-is by the demo installer and zip.
//
// This uses the repo's own Rust texture tool (patch/textool), the same tool the
// full installer runs, rather than cnetool. `--allow-any` is required because
// menu bitmaps are not square / power-of-two like the in-game 24bits textures.
// textool upserts (replace-if-present, else append) in one atomic write, so this
// script is idempotent: re-running converges to the same archive whether it
// starts from the pristine trimmed demo or an already-patched copy.
//
// Needs a Rust toolchain (the tool is built and run for the host via `cargo
// run`). Usage: node scripts/build-demo-menupics.js
import {execFileSync} from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'

const REPO = path.join(import.meta.dirname, '..')
const dest = path.join(REPO, 'game/demo/menu/menupics.dat')

// Full-game menu textures the demo archive is missing (blank slots in-game),
// staged as standalone TGAs extracted verbatim from the full game.
const additions = ['c_chn16', 'c_dr3df', 'c_gfno', 'c_gfmid', 'kc_invX', 'jg_tmA'].map((n) =>
  path.join(REPO, 'game/demo-additions/menu', `${n}.tga`),
)
// The menu font with the extra name-punctuation glyphs (the same authored TGA
// the full installer patches in). See game/full-overrides/README.md.
const menufont = path.join(REPO, 'game/full-overrides/menu/menufont.tga')

const tgas = [...additions, menufont]
for (const f of [dest, ...tgas]) {
  if (!fs.existsSync(f)) throw new Error(`missing: ${f}`)
}

// Build + run textool for the host via cargo (its .cargo/config only sets the
// Windows cross-linker, so a plain `cargo run` targets the host and executes
// here). --manifest-path so cargo resolves the crate regardless of cwd.
const manifest = path.join(REPO, 'patch/textool/Cargo.toml')
console.log(`textool set --allow-any ${path.relative(REPO, dest)}  (+${tgas.length} TGAs)`)
const out = execFileSync(
  'cargo',
  ['run', '--quiet', '--manifest-path', manifest, '--', 'set', '--allow-any', dest, ...tgas],
  {encoding: 'utf8'},
)
process.stdout.write(out)
