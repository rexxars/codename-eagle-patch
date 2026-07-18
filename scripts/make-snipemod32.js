// Regenerate the sniper-scope overlay (SNIPEMOD.tga) as a 32-bit texture with a
// smooth antialiased lens edge and an antialiased reticle, from the shipped
// 24-bit original. The stock texture's transparency is the engine's binary
// black color-key, so the lens edge and reticle steps are hard 1-bit cutouts;
// a 32-bit texture renders from its real alpha (ARGB4444 at runtime, 16
// levels), which bilinear filtering upscales smoothly at modern resolutions.
//
// Geometry is recovered from the original, not redrawn: the lens arc is an
// axis-aligned ellipse (the artist drew a circle in 320x240 screen space and
// the 4:3 aspect squeeze lands it in the square texture; fitted to ~0.2px mean
// residual), and the reticle bars/ticks are measured from the stock pixels
// (3-texel bars stepping to a 1-texel hairline at y~173 / x~192, tick marks at
// y=186/198/210/222). Deliberate changes: coverage-AA on the lens edge and
// bars, the width step becomes a short taper, and the hairline thins toward
// the seam (1.0 to 0.6 texels) so the mirrored center cross reads thinner.
// The four tick marks stay stock-exact and hard-edged (2 texels from the seam,
// full alpha, no AA); they are graduation marks and should read crisp.
//
// Usage:
//   node scripts/make-snipemod32.js <stock-textures.dat> <out.png> <out.tga>
//
// out.png is the human-oriented QC image; out.tga is the engine-order 32-bit
// TGA (pngToTga {topDown}) ready to inject into a texture archive.
import {readFile, writeFile} from 'node:fs/promises'

import {encodePng, extractTexture, parseArchive, pngToTga} from 'cnetool'

const [texturesPath, outPng, outTga] = process.argv.slice(2)
if (!texturesPath || !outPng || !outTga) {
  process.stderr.write('Usage: make-snipemod32.js <stock-textures.dat> <out.png> <out.tga>\n')
  process.exit(1)
}

const clamp01 = (v) => Math.min(1, Math.max(0, v))
const lerp = (a, b, t) => a + (b - a) * clamp01(t)

// The engine's color-key rule: everything that quantizes to RGB565 black.
const keyed = (r, g, b) => r < 8 && g < 4 && b < 8

// Solve a small linear system in place (Gauss-Jordan); returns the solution vector.
function solve(matrix, rhs) {
  const n = rhs.length
  for (let i = 0; i < n; i++) {
    const pivot = matrix[i][i]
    for (let j = 0; j < n; j++) matrix[i][j] = matrix[i][j] / pivot
    rhs[i] = rhs[i] / pivot
    for (let k = 0; k < n; k++) {
      if (k === i) continue
      const factor = matrix[k][i]
      for (let j = 0; j < n; j++) matrix[k][j] = matrix[k][j] - factor * matrix[i][j]
      rhs[k] = rhs[k] - factor * rhs[i]
    }
  }
  return rhs
}

const archive = new Uint8Array(await readFile(texturesPath))
const entry = parseArchive(archive).entries.find((e) => e.name.toLowerCase() === 'snipemod.tga')
if (!entry) throw new Error(`no SNIPEMOD.tga in ${texturesPath}`)
const tga = extractTexture(archive, entry)

const width = tga[12] | (tga[13] << 8)
const height = tga[14] | (tga[15] << 8)
if (tga[16] !== 24) throw new Error(`expected the stock 24-bit SNIPEMOD, got ${tga[16]}bpp`)

// Archive blobs store rows top-down (the descriptor lies); read them verbatim.
const hole = []
for (let y = 0; y < height; y++) {
  const row = []
  for (let x = 0; x < width; x++) {
    const o = 18 + (y * width + x) * 3
    row.push(keyed(tga[o + 2], tga[o + 1], tga[o]))
  }
  hole.push(row)
}

// --- fit the lens ellipse on clean arc points (reticle/rim area excluded) ---
const RIM = 250
const edge = []
for (let y = 1; y < RIM; y++) {
  for (let x = 1; x < RIM; x++) {
    if (hole[y][x] && (!hole[y][x - 1] || !hole[y - 1][x])) edge.push([x, y])
  }
}
// axis-aligned ellipse through x^2 + C y^2 + D x + E y + F = 0, least squares
const m = Array.from({length: 4}, () => [0, 0, 0, 0])
const v = [0, 0, 0, 0]
for (const [x, y] of edge) {
  const row = [y * y, x, y, 1]
  for (let i = 0; i < 4; i++) {
    for (let j = 0; j < 4; j++) m[i][j] = m[i][j] + row[i] * row[j]
    v[i] = v[i] + row[i] * -(x * x)
  }
}
const [coefC, coefD, coefE, coefF] = solve(m, v)
const cx = -coefD / 2
const cy = -coefE / (2 * coefC)
const rhsVal = cx * cx + coefC * cy * cy - coefF
const semiX = Math.sqrt(rhsVal)
const semiY = Math.sqrt(rhsVal / coefC)
// signed distance to the ellipse (implicit value over gradient magnitude)
const dist = (x, y) => {
  const f = ((x - cx) / semiX) ** 2 + ((y - cy) / semiY) ** 2
  const g = Math.hypot((2 * (x - cx)) / (semiX * semiX), (2 * (y - cy)) / (semiY * semiY))
  return (f - 1) / g
}
let residualSum = 0
for (const [x, y] of edge) residualSum += Math.abs(dist(x, y))
const meanResidual = residualSum / edge.length
process.stdout.write(
  `lens ellipse (${cx.toFixed(2)}, ${cy.toFixed(2)}) semi-axes ${semiX.toFixed(2)}x${semiY.toFixed(2)}, ` +
    `mean residual ${meanResidual.toFixed(3)}px over ${edge.length} arc points\n`,
)
if (meanResidual > 0.5) throw new Error('ellipse fit too loose - texture shape changed?')

// --- reticle model (widths in texels, measured from the seam edge) ---
const vWidth = (y) =>
  y < 170 ? 3 : y < 176 ? lerp(3, 1, (y - 170) / 6) : lerp(1, 0.6, (y - 176) / 80)
const hWidth = (x) =>
  x < 189 ? 3 : x < 195 ? lerp(3, 1, (x - 189) / 6) : lerp(1, 0.6, (x - 195) / 61)
// Tick marks: stock-exact texel rows, rendered hard-edged below (no AA).
const TICK_ROWS = [186, 198, 210, 222]
const TICK_WIDTH = 2 // texels from the seam, as shipped
const inReticle = (fx, fy) => {
  if (fx >= width - vWidth(fy)) return true
  if (fy >= height - hWidth(fx)) return true
  return false
}

// --- render: alpha = union(outside-ellipse solid, reticle), 4x4 supersampled ---
const SS = 4
const FEATHER = 1.5
const SURROUND = 6 // the stock near-black scope color, one 565 step above the key
const out = new Uint8Array(width * height * 4)
for (let y = 0; y < height; y++) {
  for (let x = 0; x < width; x++) {
    let coverage = 0
    for (let sy = 0; sy < SS; sy++) {
      for (let sx = 0; sx < SS; sx++) {
        const fx = x + (sx + 0.5) / SS
        const fy = y + (sy + 0.5) / SS
        const solid = clamp01(0.5 + dist(fx, fy) / FEATHER)
        coverage += inReticle(fx, fy) ? 1 : solid
      }
    }
    const o = (y * width + x) * 4
    out[o] = SURROUND
    out[o + 1] = SURROUND
    out[o + 2] = SURROUND
    // ticks are graduation marks: stock-exact texels at full alpha, no AA
    const tick = TICK_ROWS.includes(y) && x >= width - TICK_WIDTH
    out[o + 3] = tick ? 255 : Math.round((coverage / (SS * SS)) * 255)
  }
}

const png = encodePng({width, height, channels: 4, data: out})
await writeFile(outPng, png)
await writeFile(outTga, pngToTga(png, {topDown: true}))
process.stdout.write(`wrote ${outPng} (QC) and ${outTga} (engine-order 32-bit TGA)\n`)
