#!/usr/bin/env node
// Transcode Codename Eagle's Smacker cutscenes (.smk) to modern AV1 + Vorbis
// WebM, for the cevideo shim (patch/cevideo, the drop-in smackw32.dll).
//
//   node scripts/transcode-cutscenes.js <input-dir> <output-dir>
//   node scripts/transcode-cutscenes.js --check <input-dir> [output-dir]
//
// Every *.smk in <input-dir> (case-insensitive) becomes <output-dir>/<stem>.webm.
// The shim overlays subtitles by frame index, so the transcode MUST preserve the
// clip's frame rate AND exact frame count 1:1 - the encoder is not allowed to
// resample, drop or duplicate frames. After each encode (and in --check) the
// input and output are re-probed and the run FAILS if fps or frame count differ.
//
// --check: verify parity without writing. With an output dir it re-probes the
// already-transcoded outputs against their inputs (CI guard); with only an input
// dir it is a dry run that just probes the inputs and reports fps + frame count.
//
// Requires ffmpeg + ffprobe on PATH (Homebrew: /opt/homebrew/bin).
import {spawnSync} from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'

const DEFAULT_CRF = 30

function usage() {
  console.error(
    [
      'Usage:',
      '  node scripts/transcode-cutscenes.js [--crf N] <input-dir> <output-dir>',
      '  node scripts/transcode-cutscenes.js --check <input-dir> [output-dir]',
      '',
      'Transcodes every *.smk in <input-dir> to <output-dir>/<stem>.webm (AV1 + Vorbis),',
      'preserving fps and exact frame count. --check verifies parity without writing.',
    ].join('\n'),
  )
}

// Locate a required external tool, failing with a clear message if it is missing.
function requireTool(name) {
  const probe = spawnSync(name, ['-version'], {encoding: 'utf8'})
  if (probe.error) {
    console.error(
      `${name} not found on PATH - install ffmpeg (Homebrew: brew install ffmpeg) and retry`,
    )
    process.exit(1)
  }
  return name
}

// Pick the best available AV1 encoder: libsvtav1 (fast) first, then libaom-av1.
function pickAv1Encoder(ffmpeg) {
  const res = spawnSync(ffmpeg, ['-hide_banner', '-encoders'], {encoding: 'utf8'})
  const list = `${res.stdout ?? ''}${res.stderr ?? ''}`
  for (const enc of ['libsvtav1', 'libaom-av1']) {
    if (list.includes(enc)) return enc
  }
  console.error('No AV1 encoder found in ffmpeg (need libsvtav1 or libaom-av1)')
  process.exit(1)
}

// Parse ffprobe's "num/den" rate string into a Number, or null if unusable.
function parseRate(rate) {
  if (!rate || rate === '0/0') return null
  const [num, den] = rate.split('/')
  const n = Number(num)
  const d = den === undefined ? 1 : Number(den)
  if (!Number.isFinite(n) || !Number.isFinite(d) || d === 0) return null
  return n / d
}

// Probe a media file for its video fps, exact frame count and whether it carries
// an audio stream. Counts frames the reliable way (-count_frames) because neither
// Smacker nor AV1 store nb_frames in the container.
function probe(ffprobe, file) {
  const v = spawnSync(
    ffprobe,
    [
      '-v',
      'error',
      '-select_streams',
      'v:0',
      '-count_frames',
      '-show_entries',
      'stream=r_frame_rate,nb_read_frames',
      '-of',
      'default=nokey=1:noprint_wrappers=1',
      file,
    ],
    {encoding: 'utf8'},
  )
  if (v.status !== 0) {
    throw new Error(`ffprobe failed on ${file}: ${(v.stderr ?? '').trim()}`)
  }
  const [rateLine, framesLine] = v.stdout.trim().split('\n')
  const fps = parseRate(rateLine)
  const frameCount = Number(framesLine)
  if (fps === null || !Number.isInteger(frameCount)) {
    throw new Error(`could not read fps/frame count from ${file} (got "${v.stdout.trim()}")`)
  }
  const a = spawnSync(
    ffprobe,
    [
      '-v',
      'error',
      '-select_streams',
      'a',
      '-show_entries',
      'stream=index',
      '-of',
      'csv=p=0',
      file,
    ],
    {encoding: 'utf8'},
  )
  const hasAudio = a.status === 0 && a.stdout.trim() !== ''
  return {fps, frameCount, hasAudio}
}

// Compare an input probe against an output probe. Returns null on parity, or a
// human-readable reason string on mismatch. fps is compared with a small epsilon
// (rational rates re-expressed by the muxer can differ in the last ulp).
function parityMismatch(input, output) {
  if (output.frameCount !== input.frameCount) {
    return `frame count ${output.frameCount} != input ${input.frameCount}`
  }
  if (Math.abs(output.fps - input.fps) > 1e-6) {
    return `fps ${output.fps} != input ${input.fps}`
  }
  return null
}

// List *.smk files (case-insensitive) in a directory, sorted for stable output.
function listSmk(dir) {
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    console.error(`Input directory not found: ${dir}`)
    process.exit(1)
  }
  return fs
    .readdirSync(dir)
    .filter((name) => name.toLowerCase().endsWith('.smk'))
    .sort((a, b) => a.localeCompare(b))
}

function transcode(ffmpeg, encoder, crf, input, output, inProbe) {
  const args = ['-y', '-i', input, '-r', String(inProbe.fps), '-c:v', encoder, '-crf', String(crf)]
  if (encoder === 'libsvtav1') args.push('-preset', '6')
  args.push('-pix_fmt', 'yuv420p')
  if (inProbe.hasAudio) args.push('-c:a', 'libvorbis', '-q:a', '4')
  else args.push('-an')
  args.push(output)
  const res = spawnSync(ffmpeg, args, {encoding: 'utf8'})
  if (res.status !== 0) {
    throw new Error(
      `ffmpeg failed on ${input}: ${(res.stderr ?? '').trim().split('\n').slice(-5).join('\n')}`,
    )
  }
}

function main() {
  const argv = process.argv.slice(2)
  if (argv.includes('--help') || argv.includes('-h')) {
    usage()
    process.exit(0)
  }
  const check = argv.includes('--check')
  let crf = DEFAULT_CRF
  const positional = []
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === '--check') continue
    if (arg === '--crf') {
      crf = Number(argv[++i])
      if (!Number.isFinite(crf)) {
        console.error('--crf requires a number')
        process.exit(1)
      }
      continue
    }
    if (arg.startsWith('--crf=')) {
      crf = Number(arg.slice('--crf='.length))
      continue
    }
    if (arg.startsWith('-')) {
      console.error(`Unknown option: ${arg}`)
      usage()
      process.exit(1)
    }
    positional.push(arg)
  }

  const [inputDir, outputDir] = positional
  if (!inputDir) {
    usage()
    process.exit(1)
  }
  // A plain transcode needs an output dir; --check may run as an input-only dry run.
  if (!check && !outputDir) {
    usage()
    process.exit(1)
  }

  const ffprobe = requireTool('ffprobe')
  const smks = listSmk(inputDir)
  if (smks.length === 0) {
    console.error(`No .smk files found in ${inputDir}`)
    process.exit(1)
  }

  let failures = 0

  if (check && !outputDir) {
    // Dry run: probe inputs only, report fps + frame count.
    for (const name of smks) {
      const input = path.join(inputDir, name)
      try {
        const p = probe(ffprobe, input)
        console.log(`${name}: ${p.frameCount} frames @ ${p.fps} fps${p.hasAudio ? ' +audio' : ''}`)
      } catch (error) {
        console.error(`FAIL ${name}: ${error.message}`)
        failures++
      }
    }
    if (failures > 0) process.exit(1)
    return
  }

  if (check) {
    // Verify already-transcoded outputs against their inputs.
    for (const name of smks) {
      const input = path.join(inputDir, name)
      const output = path.join(outputDir, `${path.parse(name).name}.webm`)
      try {
        if (!fs.existsSync(output)) throw new Error(`missing output ${output}`)
        const inP = probe(ffprobe, input)
        const outP = probe(ffprobe, output)
        const mismatch = parityMismatch(inP, outP)
        if (mismatch) throw new Error(mismatch)
        console.log(
          `OK ${name} -> ${path.basename(output)} (${inP.frameCount} frames @ ${inP.fps} fps)`,
        )
      } catch (error) {
        console.error(`FAIL ${name}: ${error.message}`)
        failures++
      }
    }
    if (failures > 0) {
      console.error(`${failures} of ${smks.length} cutscene(s) failed the parity check`)
      process.exit(1)
    }
    console.log(`All ${smks.length} cutscene(s) pass the parity check`)
    return
  }

  // Transcode mode.
  const ffmpeg = requireTool('ffmpeg')
  const encoder = pickAv1Encoder(ffmpeg)
  console.log(`Transcoding ${smks.length} cutscene(s) with ${encoder} (crf ${crf}) + libvorbis`)
  fs.mkdirSync(outputDir, {recursive: true})

  for (const name of smks) {
    const input = path.join(inputDir, name)
    const output = path.join(outputDir, `${path.parse(name).name}.webm`)
    try {
      const inP = probe(ffprobe, input)
      transcode(ffmpeg, encoder, crf, input, output, inP)
      const outP = probe(ffprobe, output)
      const mismatch = parityMismatch(inP, outP)
      if (mismatch) {
        fs.rmSync(output, {force: true})
        throw new Error(`parity check failed (${mismatch})`)
      }
      console.log(
        `  ${name} -> ${path.basename(output)}: ${outP.frameCount} frames @ ${outP.fps} fps`,
      )
    } catch (error) {
      console.error(`FAIL ${name}: ${error.message}`)
      failures++
    }
  }

  if (failures > 0) {
    console.error(`${failures} of ${smks.length} cutscene(s) failed`)
    process.exit(1)
  }
  console.log(`Done: ${smks.length} cutscene(s) transcoded, all frame/fps parity checks passed`)
}

main()
