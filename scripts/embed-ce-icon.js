// Embed a Windows application icon into a patched ce.exe (and any other CE
// executable), cross-platform and dependency-light.
//
// Stock ce.exe ships with no resource (.rsrc) section at all, so Windows shows
// the generic executable icon everywhere (Explorer, taskbar, alt-tab). This
// step appends a proper icon resource. Because it is append-only it never
// touches the stock .text/.rdata/.data bytes the provenance tests check.
//
// The icon we embed is a *hybrid*: the small frames are classic BMP/DIB (which
// Windows XP can decode), plus a PNG-compressed 256x256 frame for crisp hi-DPI
// rendering on Windows 10/11. A modern all-PNG .ico renders as a blank icon on
// XP because XP has no PNG decoder for icon frames; an all-BMP .ico works
// everywhere but is large and has no 256 frame. The hybrid gets both right.
//
// Pure ESM, no native binaries and no Wine, so it runs the same on macOS, the
// Linux release runner and Windows. Uses `resedit` (a pure-JS PE editor) for
// the PE surgery; the .ico assembly is plain Buffer work.

import fs from 'node:fs'
import * as ResEdit from 'resedit'

const ICONDIR_SIZE = 6
const ICONDIRENTRY_SIZE = 16
const PNG_MAGIC = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])

// Parse an .ico into its frames. Each frame keeps its raw image payload (either
// a BMP/DIB blob or a PNG blob) plus the directory metadata.
function parseIco(buf) {
  const reserved = buf.readUInt16LE(0)
  const type = buf.readUInt16LE(2)
  const count = buf.readUInt16LE(4)
  if (reserved !== 0 || type !== 1) {
    throw new Error('Not a valid .ico file (bad ICONDIR header)')
  }
  const frames = []
  for (let i = 0; i < count; i++) {
    const o = ICONDIR_SIZE + i * ICONDIRENTRY_SIZE
    const width = buf.readUInt8(o) || 256
    const height = buf.readUInt8(o + 1) || 256
    const colors = buf.readUInt8(o + 2)
    const planes = buf.readUInt16LE(o + 4)
    const bitCount = buf.readUInt16LE(o + 6)
    const bytes = buf.readUInt32LE(o + 8)
    const offset = buf.readUInt32LE(o + 12)
    const data = buf.subarray(offset, offset + bytes)
    const isPng = data.subarray(0, 8).equals(PNG_MAGIC)
    frames.push({width, height, colors, planes, bitCount, isPng, data})
  }
  return frames
}

// Reassemble frames into a single .ico buffer.
function assembleIco(frames) {
  const header = Buffer.alloc(ICONDIR_SIZE + frames.length * ICONDIRENTRY_SIZE)
  header.writeUInt16LE(0, 0) // reserved
  header.writeUInt16LE(1, 2) // type: icon
  header.writeUInt16LE(frames.length, 4)

  let offset = header.length
  const bodies = []
  frames.forEach((f, i) => {
    const o = ICONDIR_SIZE + i * ICONDIRENTRY_SIZE
    header.writeUInt8(f.width >= 256 ? 0 : f.width, o)
    header.writeUInt8(f.height >= 256 ? 0 : f.height, o + 1)
    header.writeUInt8(f.colors, o + 2)
    header.writeUInt8(0, o + 3) // reserved
    header.writeUInt16LE(f.planes, o + 4)
    header.writeUInt16LE(f.bitCount, o + 6)
    header.writeUInt32LE(f.data.length, o + 8)
    header.writeUInt32LE(offset, o + 12)
    bodies.push(f.data)
    offset += f.data.length
  })
  return Buffer.concat([header, ...bodies])
}

// Build the XP-safe hybrid: every sub-256 frame from the BMP-based `legacy`
// icon, plus the 256x256 frame(s) from the PNG-based `modern` icon.
export function buildHybridIcon(legacyIco, modernIco) {
  const legacy = parseIco(legacyIco)
  const modern = parseIco(modernIco)
  const small = legacy.filter((f) => f.width < 256 && f.height < 256)
  const large = modern.filter((f) => f.width >= 256 || f.height >= 256)
  if (small.length === 0) throw new Error('legacy icon has no sub-256 frames')
  if (large.length === 0) throw new Error('modern icon has no 256x256 frame')
  return assembleIco([...small, ...large])
}

// Embed one or more .ico into a PE executable, returning the new exe bytes.
// The icons become icon groups 1..n: group 1 is the numerically lowest id, so
// Windows uses it as the default application icon (Explorer, taskbar, alt-tab);
// the rest are selectable alternates, e.g. via a shortcut's "Change Icon"
// dialog. The resource-directory timestamp is pinned to 0 so output is
// reproducible (byte-identical across runs), which the provenance tests rely on.
export function embedIcons(exeBuf, icoBufs, {lang = 1033} = {}) {
  const exe = ResEdit.NtExecutable.from(exeBuf, {ignoreCert: true})
  const res = ResEdit.NtExecutableResource.from(exe)
  icoBufs.forEach((icoBuf, i) => {
    const iconFile = ResEdit.Data.IconFile.from(icoBuf)
    ResEdit.Resource.IconGroupEntry.replaceIconsForResource(
      res.entries,
      i + 1,
      lang,
      iconFile.icons.map((item) => item.data),
    )
  })
  for (const entry of res.entries) entry.lang = lang
  res.outputResource(exe)
  return Buffer.from(exe.generate())
}

// Convenience wrapper for the single-icon case.
export function embedIcon(exeBuf, icoBuf, opts = {}) {
  return embedIcons(exeBuf, [icoBuf], opts)
}

const USAGE = `Usage:
  node scripts/embed-ce-icon.js <target.exe> <icon.ico> [alt.ico ...] [-o out.exe]
      Embed one or more icons into <target.exe> (in place unless -o is given).
      The first icon is the default application icon; each additional icon is a
      selectable alternate (e.g. via a shortcut's "Change Icon" dialog).

  node scripts/embed-ce-icon.js merge <legacy.ico> <modern.ico> <out.ico>
      Author the hybrid icon: sub-256 BMP frames from <legacy.ico> (XP-safe)
      plus the 256x256 PNG frame from <modern.ico>. This is a one-time step;
      its output is the committed patch/assets/ce.ico.`

function main(argv) {
  const args = argv.slice(2)
  if (args[0] === 'merge') {
    const [, legacyPath, modernPath, outPath] = args
    if (!legacyPath || !modernPath || !outPath) {
      console.error(USAGE)
      process.exit(1)
    }
    const hybrid = buildHybridIcon(fs.readFileSync(legacyPath), fs.readFileSync(modernPath))
    fs.writeFileSync(outPath, hybrid)
    console.log(`Wrote hybrid icon ${outPath} (${hybrid.length} bytes)`)
    return
  }
  let outPath = null
  const oi = args.indexOf('-o')
  if (oi !== -1) {
    outPath = args[oi + 1]
    args.splice(oi, 2)
  }
  const [exePath, ...icoPaths] = args
  if (!exePath || icoPaths.length === 0) {
    console.error(USAGE)
    process.exit(1)
  }
  outPath ??= exePath
  const out = embedIcons(
    fs.readFileSync(exePath),
    icoPaths.map((p) => fs.readFileSync(p)),
  )
  fs.writeFileSync(outPath, out)
  console.log(`Embedded ${icoPaths.length} icon(s) into ${outPath} (${out.length} bytes)`)
}

if (import.meta.filename === process.argv[1]) main(process.argv)
