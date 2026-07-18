// Regenerates the user-facing change descriptions from docs/changes.js into the
// documents that show them: the root README change list and screenshot gallery,
// game/common/readme150.txt, and patch/README.md. Run it after editing
// docs/changes.js so every document stays in sync:
//
//   node scripts/generate-docs.js
//
// Pass --check to verify the documents are up to date without writing (exits
// non-zero if anything would change), which is useful in CI or a pre-commit
// hook.

import {execFileSync} from 'node:child_process'
import {mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs'
import {join} from 'node:path'
import {categories, changes} from '../docs/changes.js'

const repoRoot = join(import.meta.dirname, '..')
const check = process.argv.includes('--check')

// Format a markdown string the way `npm run format` (oxfmt) would, so generated
// output stays byte-identical after the formatter runs (oxfmt aligns tables).
// The temp file lives inside the repo so oxfmt picks up the repo's config.
function formatMarkdown(content) {
  const dir = mkdtempSync(join(repoRoot, '.generate-docs-'))
  try {
    const file = join(dir, 'doc.md')
    writeFileSync(file, content)
    execFileSync(join(repoRoot, 'node_modules/.bin/oxfmt'), [file])
    return readFileSync(file, 'utf8')
  } finally {
    rmSync(dir, {recursive: true, force: true})
  }
}

function changesByCategory(key) {
  return changes.filter((change) => change.category === key)
}

// A change scoped to the full game gets a "(full game)" note. When every change
// in a category is full-game, the note goes on the category heading instead of
// repeating on each line.
function categoryIsFullOnly(entries) {
  return entries.length > 0 && entries.every((change) => change.scope === 'full')
}

function titleWithScope(change, categoryFullOnly) {
  if (categoryFullOnly || change.scope !== 'full') return change.title
  return `${change.title} (full game)`
}

// Plain-text "What's new" body for readme150.txt.
function renderReadme150() {
  const blocks = []
  for (const category of categories) {
    const entries = changesByCategory(category.key)
    if (entries.length === 0) continue
    const fullOnly = categoryIsFullOnly(entries)
    const heading = fullOnly ? `${category.title} (full game)` : category.title
    const lines = [heading, '']
    for (const change of entries) {
      lines.push(`* ${titleWithScope(change, fullOnly)}`)
      lines.push(change.summary)
      lines.push('')
    }
    blocks.push(lines.join('\n').trimEnd())
  }
  return blocks.join('\n\n')
}

// Markdown list of every change, grouped by category. `field` picks the prose:
// 'summary' for the concise root README list, 'body' for the fuller
// patch/README.md list.
function renderCategorizedMarkdown(field) {
  const blocks = []
  for (const category of categories) {
    const entries = changesByCategory(category.key)
    if (entries.length === 0) continue
    const fullOnly = categoryIsFullOnly(entries)
    const heading = fullOnly ? `${category.title} (full game)` : category.title
    const lines = [`### ${heading}`, '']
    for (const change of entries) {
      lines.push(`- **${titleWithScope(change, fullOnly)}.** ${change[field]}`)
    }
    blocks.push(lines.join('\n'))
  }
  return blocks.join('\n\n')
}

// Markdown before/after gallery for the root README, one table per change that
// carries a screenshot pair.
function renderScreenshots() {
  const blocks = []
  for (const change of changes) {
    if (!change.images) continue
    const {before, after, alt} = change.images
    blocks.push(
      [
        `**${change.title}**`,
        '',
        '| Before | After |',
        '| --- | --- |',
        `| ![${alt} before](${before}) | ![${alt} after](${after}) |`,
      ].join('\n'),
    )
  }
  return blocks.join('\n\n')
}

// Replace the text between a pair of HTML comment markers, keeping the markers.
function replaceMarked(source, name, body) {
  const start = `<!-- GENERATED:${name}:start -->`
  const end = `<!-- GENERATED:${name}:end -->`
  const startAt = source.indexOf(start)
  const endAt = source.indexOf(end)
  if (startAt === -1 || endAt === -1) {
    throw new Error(`Missing ${start} / ${end} markers`)
  }
  const before = source.slice(0, startAt + start.length)
  const after = source.slice(endAt)
  return `${before}\n\n${body}\n\n${after}`
}

// Replace the body of readme150.txt section 3. The section heading is anchored
// on the separator line that precedes it, so it does not match the same text in
// the table of contents. Works on LF-normalized input.
function replaceReadme150Section(source, body) {
  const pattern = /(={20,}\n3\.What's new in v1\.50 \?\n)([\s\S]*?)(\n={20,}\n4\.Compatibility)/
  if (!pattern.test(source)) {
    throw new Error('Could not locate the "What\'s new" section in readme150.txt')
  }
  return source.replace(pattern, `$1\n${body}\n$3`)
}

const targets = [
  {
    path: 'README.md',
    format: true,
    transform: (source) => {
      let next = replaceMarked(source, 'changes', renderCategorizedMarkdown('summary'))
      next = replaceMarked(next, 'screenshots', renderScreenshots())
      return next
    },
  },
  {
    path: 'patch/README.md',
    format: true,
    transform: (source) => replaceMarked(source, 'changes', renderCategorizedMarkdown('body')),
  },
  {
    path: 'game/common/readme150.txt',
    transform: (source) => {
      // readme150.txt uses CRLF line endings. Work in LF, then restore them.
      const isCrlf = source.includes('\r\n')
      const lf = source.replace(/\r\n/g, '\n')
      const replaced = replaceReadme150Section(lf, renderReadme150())
      return isCrlf ? replaced.replace(/\n/g, '\r\n') : replaced
    },
  },
]

let changed = false
for (const target of targets) {
  const fullPath = join(repoRoot, target.path)
  const source = readFileSync(fullPath, 'utf8')
  let next = target.transform(source)
  if (target.format) next = formatMarkdown(next)
  if (next === source) continue
  changed = true
  if (check) {
    console.error(`Out of date: ${target.path}`)
  } else {
    writeFileSync(fullPath, next)
    console.log(`Updated ${target.path}`)
  }
}

if (check && changed) {
  console.error('Run "node scripts/generate-docs.js" to update the generated sections.')
  process.exit(1)
}

if (!changed) {
  console.log('Documents already up to date.')
}
