#!/usr/bin/env node
// Reproduce the patched MENUDLL.DLL (map-name display + 15-char map column +
// 25-char server-name column + base-relocation fix) from a stock 1.41
// MENUDLL.DLL, then bake in the menu.log and savegame (saves\sg%d.dat)
// redirects, producing the menudll.dll that ce-patch bundles/installs.
//
// The DISPLAY_EDITS were recovered by diffing the hand-patched menudll against
// stock (26 runs). The original patches were made by hand; the "why" is from
// commit fc7973c ("real map names ..."). What each group does:
//
//   - map-name display code (.text): the browser's populate path is duplicated
//     across ~5 functions; each gets the same ~57-byte edit so the row shows the
//     server's real map-name string. Plus two small helper edits (0x3aa8e/0x3aac8)
//     and a guard tweak (0x1b8b).
//   - 15-char map column (.rdata, ~0xa36cc / file 669356): a format-string edit -
//     iplist pads the map to MAP_WIDTH=15 and the menudll row format's post-map gap
//     is shrunk 12 -> 4 spaces (15+4 = the 19-char slot Players/Type/IP align to),
//     so columns line up for any name length.
//   - base-relocation fix (.reloc): 15 entries whose type nibble is changed 3 -> 0
//     (HIGHLOW -> ABSOLUTE) so the loader does NOT relocate addresses inside the
//     patched bytes - otherwise it corrupts the patch at load (the EIP=0xFF crash).
//
// To extend the menudll patch: edit/add entries here (and update the notes above),
// then regenerate ./menudll.dll (this directory's reference build).
//
// Usage: make-menudll-patch.js STOCK_MENUDLL.DLL -o menudll.dll
import assert from 'node:assert'
import fs from 'node:fs'
import {parseArgs} from 'node:util'

// (file_offset, orig_hex, new_hex)
const DISPLAY_EDITS = [
  [0x1b8b, 'cccccccccccccccc', '544553544d415000'], // map-name display code
  [
    0x15146,
    '742b68d4390a10e80e40030083c4048b9518fbffff5268c8390a1068183a0a10e8f53f030083c40ce9dd0500008b859cfaffff898530faffff',
    '0f85040600008b859cfaffff898530faffff8b851cfbffff83c00489859cfaffff89c283c21839d07308803822740340ebf4c6000090909090',
  ], // map-name display code
  [
    0x1bba6,
    '742b68d4390a10e8aed5020083c4048b9518fbffff5268c8390a1068183a0a10e895d5020083c40ce9dd0500008b859cfaffff898530faffff',
    '0f85040600008b859cfaffff898530faffff8b851cfbffff83c00489859cfaffff89c283c21839d07308803822740340ebf4c6000090909090',
  ], // map-name display code
  [
    0x224b6,
    '742b68d4390a10e89e6c020083c4048b9518fbffff5268c8390a1068183a0a10e8856c020083c40ce9dd0500008b859cfaffff898530faffff',
    '0f85040600008b859cfaffff898530faffff8b851cfbffff83c00489859cfaffff89c283c21839d07308803822740340ebf4c6000090909090',
  ], // map-name display code
  [
    0x28dc6,
    '742b68d4390a10e88e03020083c4048b9518fbffff5268c8390a1068183a0a10e87503020083c40ce9dd0500008b859cfaffff898530faffff',
    '0f85040600008b859cfaffff898530faffff8b851cfbffff83c00489859cfaffff89c283c21839d07308803822740340ebf4c6000090909090',
  ], // map-name display code
  [
    0x2f6d6,
    '742b68d4390a10e87e9a010083c4048b9518fbffff5268c8390a1068183a0a10e8659a010083c40ce9dd0500008b859cfaffff898530faffff',
    '0f85040600008b859cfaffff898530faffff8b851cfbffff83c00489859cfaffff89c283c21839d07308803822740340ebf4c6000090909090',
  ], // map-name display code
  [0x3aa8e, 'cccccccccccccccccccccccccccccccccccccc', '8b45088b48048b91c40000008b12e92f000000'], // map-name display code
  [0x3aac8, '8b45088b088b510c', 'e9c1ffffff909090'], // map-name display code
  // Row format string (rebuilt whole from stock in one edit). Two changes vs
  // stock "%s<20sp>%s %6d<16sp>%s<8sp>%s<8sp>%d.%d.%d.%d":
  //   (1) the map id "%s %6d" becomes a single map-name "%s" (the map-name
  //       display code above feeds it the real map string), and
  //   (2) the name->map gap shrinks 20->10 spaces so the server-name column can
  //       grow 15->25 chars (see the name-cap edits appended below) WITHOUT
  //       moving map/players/type/IP - there were 20 blank cols after the name.
  // New string is 55 bytes; the region is 75, so it's NUL-padded to fit.
  [
    0xa36ac,
    '2573202020202020202020202020202020202020202025732025366420202020202020202020202020202020257320202020202020202573202020202020202025642e25642e25642e2564',
    '257320202020202020202020257320202020257320202020257320202020202020202573202020202020202025642e25642e25642e25640000000000000000000000000000000000000000',
  ], // rdata: row format (map -> %s, name column 25-wide)
  [0xbeaa7, '31', '01'], // base-relocation fix
  [0xbeaa9, '31', '01'], // base-relocation fix
  [0xbeaab, '31', '01'], // base-relocation fix
  [0xbf7a3, '3b', '0b'], // base-relocation fix
  [0xbf7a5, '3b', '0b'], // base-relocation fix
  [0xbf7a7, '3b', '0b'], // base-relocation fix
  [0xc04a1, '34', '04'], // base-relocation fix
  [0xc04a3, '34', '04'], // base-relocation fix
  [0xc04a5, '34', '04'], // base-relocation fix
  [0xc1193, '3d', '0d'], // base-relocation fix
  [0xc1195, '3d', '0d'], // base-relocation fix
  [0xc1197, '3d', '0d'], // base-relocation fix
  [0xc1e8d, '36', '06'], // base-relocation fix
  [0xc1e8f, '36', '06'], // base-relocation fix
  [0xc1e91, '36', '06'], // base-relocation fix
]

// Server-name display column: raise the 15-char cap to 25 at all 5 duplicated
// browser-populate sites. Each site space-pads a 30-byte stack buffer at
// [ebp-0x550] then caps the name at 15 via three instructions:
//   (a) length clamp   `cmp dword [ebp-4], 0xf`        (837dfc0f)
//   (b) clamp value    `mov dword [ebp-0x64c], 0xf`    (c785b4f9ffff0f000000)
//   (c) NUL terminator `mov byte [ebp-0x541], 0`       (c685bffaffff00) -> buf+15
// Bump the clamp to 25 (0x19) and move the NUL to buf+25 = [ebp-0x537]
// (c685c9faffff00). The buffer is already memset to 30 spaces, so a 25-char
// space-padded field + NUL fits with no stack-frame change, and the format edit
// above keeps every later column in place. NB: the length-clamp byte pattern
// 837dfc0f also occurs once at 0x412a9 (an unrelated `cmp [ebp-4],0xf`) - that
// offset is deliberately excluded.
const NAME_CLAMP_CMP = [0x15085, 0x1bae5, 0x223f5, 0x28d05, 0x2f615]
const NAME_CLAMP_VAL = [0x15096, 0x1baf6, 0x22406, 0x28d16, 0x2f626]
const NAME_NUL_TERM = [0x150d6, 0x1bb36, 0x22446, 0x28d56, 0x2f666]
for (const o of NAME_CLAMP_CMP) DISPLAY_EDITS.push([o, '837dfc0f', '837dfc19'])
for (const o of NAME_CLAMP_VAL)
  DISPLAY_EDITS.push([o, 'c785b4f9ffff0f000000', 'c785b4f9ffff19000000'])
for (const o of NAME_NUL_TERM) DISPLAY_EDITS.push([o, 'c685bffaffff00', 'c685c9faffff00'])

// menu.log redirect (same as ce.exe's logs): c:\menu.log -> logs\menu.log
const MENULOG_STR_OFF = 0xb5d00 // .data slack (verify zeros)
const MENULOG_STR = Buffer.from('logs\\menu.log\x00', 'latin1').toString('hex')
const MENULOG_PTR_OFF = 0x3aef8 // mov dword[ebp-0x24], imm
const MENULOG_PTR_ORIG = 'e4560a10' // 0x100a56e4 (c:\menu.log)
const MENULOG_PTR_NEW = '005d0b10' // 0x100b5d00 (logs\menu.log)

// savegame redirect (matches ce-patch fix #11 in ce.exe, same string value):
// sg%d.dat -> saves\sg%d.dat. The save/load slot menu builds the filename
// itself via sprintf in 5 duplicated populate/action paths, all `push imm32`
// of the .rdata "sg%d.dat" (0x100a3dec); repoint them at a new string in the
// .data slack right after logs\menu.log. All 5 sites have HIGHLOW relocs that
// stay valid (the new value is still an in-module address), and no reloc
// targets the slack (verified against stock .reloc).
const SAVES_STR_OFF = 0xb5d10 // .data slack (verify zeros)
const SAVES_STR = Buffer.from('saves\\sg%d.dat\x00', 'latin1').toString('hex')
const SAVES_PTR_OFFS = [0x180b6, 0x1e9c6, 0x252d6, 0x2bbe6, 0x324f6]
const SAVES_PTR_ORIG = 'ec3d0a10' // 0x100a3dec (sg%d.dat)
const SAVES_PTR_NEW = '105d0b10' // 0x100b5d10 (saves\sg%d.dat)

function fromHex(h) {
  return Buffer.from(h, 'hex')
}

function allZero(buf) {
  for (const b of buf) if (b !== 0) return false
  return true
}

function main() {
  const {values, positionals} = parseArgs({
    options: {out: {type: 'string', short: 'o'}},
    allowPositionals: true,
  })
  const stock = positionals[0]
  const outPath = values.out
  if (!stock) throw new Error('need STOCK_MENUDLL.DLL')
  if (!outPath) throw new Error('need -o OUT.dll')

  const d = fs.readFileSync(stock)

  for (const [off, o, n] of DISPLAY_EDITS) {
    const ob = fromHex(o)
    const nb = fromHex(n)
    const cur = d.subarray(off, off + ob.length)
    if (cur.equals(nb)) continue
    assert(cur.equals(ob), `unexpected bytes at ${hex(off)}: ${cur.toString('hex')} (not stock)`)
    nb.copy(d, off)
  }

  // menu.log: string into slack + repoint
  let s = fromHex(MENULOG_STR)
  {
    const slot = d.subarray(MENULOG_STR_OFF, MENULOG_STR_OFF + s.length)
    assert(allZero(slot) || slot.equals(s))
    s.copy(d, MENULOG_STR_OFF)
    const po = fromHex(MENULOG_PTR_ORIG)
    const pn = fromHex(MENULOG_PTR_NEW)
    const cur = d.subarray(MENULOG_PTR_OFF, MENULOG_PTR_OFF + 4)
    assert(cur.equals(po) || cur.equals(pn), `menu.log ptr unexpected: ${cur.toString('hex')}`)
    pn.copy(d, MENULOG_PTR_OFF)
  }

  // savegames: string into slack + repoint the 5 sprintf sites
  s = fromHex(SAVES_STR)
  {
    const slot = d.subarray(SAVES_STR_OFF, SAVES_STR_OFF + s.length)
    assert(allZero(slot) || slot.equals(s))
    s.copy(d, SAVES_STR_OFF)
    const po = fromHex(SAVES_PTR_ORIG)
    const pn = fromHex(SAVES_PTR_NEW)
    for (const off of SAVES_PTR_OFFS) {
      const cur = d.subarray(off, off + 4)
      assert(
        cur.equals(po) || cur.equals(pn),
        `sg%d.dat ptr unexpected at ${hex(off)}: ${cur.toString('hex')}`,
      )
      pn.copy(d, off)
    }
  }

  fs.writeFileSync(outPath, d)
  console.log(
    `wrote ${outPath} (${d.length} bytes, ${DISPLAY_EDITS.length} display edits + menu.log + saves)`,
  )
}

function hex(v) {
  return '0x' + (v >>> 0).toString(16)
}

main()
