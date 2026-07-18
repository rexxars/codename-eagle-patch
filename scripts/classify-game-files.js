#!/usr/bin/env node
// ONE-SHOT migration tool: classified the pre-split game/ layout; kept for provenance.
// Classify game/ files into common/ (shipped by demo installer AND full-game
// patcher) vs demo/ (demo installer only). Rules from the 2026-07-13 design doc:
//   - unchanged 1.0 -> 1.43 -> repo: full game already has it -> demo/
//   - level128..level134 + levels.nfo variants: level dirs wholesale -> common/
//   - configs (default.cfg, keyconf.dat, menuinfo.dat) -> common/ (patcher
//     applies write-if-absent + stock-refresh logic)
//   - known demo variants (Game.exe, 24bits/textures.dat, menu/**, levels.nfo)
//     -> demo/
//   - everything else that differs from pristine 1.43 or is repo-only is a
//     1.50 fix -> common/
// Usage: node scripts/classify-game-files.js [--check]
//   default: prints "<class>\t<path>" lines; --check: only prints a summary.
import {createHash} from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'

const REPO = path.join(import.meta.dirname, '..')
const GAME = path.join(REPO, 'game')
const PRISTINE = process.env.CE_PRISTINE
if (!PRISTINE) {
  console.error('Set CE_PRISTINE to a directory containing pristine 1.0/ and 1.43/ installs')
  process.exit(1)
}

if (fs.existsSync(path.join(GAME, 'common'))) {
  console.error(
    'game/ already split — this script classified the pre-split layout (see commit 9c0aa96)',
  )
  process.exit(1)
}
for (const version of ['1.0', '1.43']) {
  if (!fs.existsSync(path.join(PRISTINE, version))) {
    console.error(`Pristine reference install not found: ${path.join(PRISTINE, version)}`)
    process.exit(1)
  }
}

const JUNK =
  /(^|\/)((a|o|s|scr|tex|fac|w)cache\.(bin|dat)|hiscores\.dat|diacache\.dat|lobby\.log|sg0\.dat|.*\.bak|\.ds_store)$/i
const CONFIGS = new Set(['default.cfg', 'keyconf.dat', 'menuinfo.dat'])
const DEMO_VARIANTS = [/^game\.exe$/i, /^24bits\/textures\.dat$/i, /^menu\//i, /^levels\.nfo$/i]
const LEVEL_DIR = /^level(12[89]|13[0-4])\//i

function walk(dir, base = dir, out = new Map()) {
  for (const e of fs.readdirSync(dir, {withFileTypes: true})) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) walk(p, base, out)
    else if (e.isFile()) {
      const rel = path.relative(base, p)
      out.set(rel.toLowerCase(), p)
    }
  }
  return out
}
const md5 = (p) => createHash('md5').update(fs.readFileSync(p)).digest('hex')

const repoFiles = walk(GAME)
const p10 = walk(path.join(PRISTINE, '1.0'))
const p143 = walk(path.join(PRISTINE, '1.43'))

const result = new Map()
for (const [rel, actual] of [...repoFiles.entries()].sort()) {
  if (JUNK.test(rel)) {
    result.set(rel, {cls: 'JUNK-IN-REPO', actual})
    continue
  }
  let cls
  if (CONFIGS.has(rel)) cls = 'common'
  else if (LEVEL_DIR.test(rel)) cls = 'common'
  else if (DEMO_VARIANTS.some((re) => re.test(rel))) cls = 'demo'
  else {
    const hRepo = md5(actual)
    const h10 = p10.has(rel) ? md5(p10.get(rel)) : null
    const h143 = p143.has(rel) ? md5(p143.get(rel)) : null
    cls = h10 && h10 === h143 && h143 === hRepo ? 'demo' : 'common'
  }
  result.set(rel, {cls, actual})
}

const counts = {}
for (const {cls} of result.values()) counts[cls] = (counts[cls] ?? 0) + 1
if (process.argv.includes('--check')) {
  console.log(counts)
} else {
  for (const [, {cls, actual}] of result) console.log(`${cls}\t${path.relative(REPO, actual)}`)
}
if (counts['JUNK-IN-REPO']) {
  console.error(
    `WARNING: ${counts['JUNK-IN-REPO']} junk files inside game/ - investigate before splitting`,
  )
  process.exitCode = 1
}
