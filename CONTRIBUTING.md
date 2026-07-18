# Contributing

This repository holds the tooling and source that produce the Codename Eagle 1.50 releases. Players do not need any of it: they download a build from the [releases page](https://github.com/rexxars/codename-eagle-patch/releases). This document is for building the releases yourself, working on the patch, and keeping the documentation in sync.

## Repository layout

- [`game/`](game/) is the payload that the releases ship. It splits into `common/` (shipped by everything), `demo/` (multiplayer demo builds and the Docker image), and `full/` (the full-game patcher only). See [`game/README.md`](game/README.md) for the details of the split.
- [`patch/`](patch/) is the development tool that produces the pre-patched binaries in `game/common/`. It is never run on user machines. It also houses the `iplist/`, `menudll/` and `cemusic/` subprojects, which only work together with its byte patches. See [`patch/README.md`](patch/README.md).
- [`ripmusic/`](ripmusic/) is the CD-to-Ogg soundtrack ripper. See [`ripmusic/README.md`](ripmusic/README.md).
- [`installers/`](installers/) holds the NSIS setup wizards (`demo/` and `patch/`) and the extract-and-play `demo-zip/`.
- [`dgvoodoo/`](dgvoodoo/) holds the bundled dgVoodoo graphics wrapper (third-party), kept on its own so it can be updated as a drop-in.
- [`docker-dedicated/`](docker-dedicated/) is the dedicated server image, which runs the game's server under Wine.
- [`scripts/`](scripts/) holds the payload maintenance scripts and the documentation generator.
- [`docs/`](docs/) holds `fixes.js`, the single source of truth for the change descriptions (see below).

## Tooling and conventions

- **Node.js for all scripting, not Python.** Scripts are plain ESM `.js` files (the repository is `"type": "module"`). Target Node.js 22.19 or newer: use `node:` import prefixes, the global `crypto`, and `import.meta.dirname`. There is no Python anywhere in the build.
- **Cross-platform by default.** The scripts and the release build are meant to run on macOS, Linux and Windows. The GitHub Actions release builds everything on Linux, so that is the reference environment. A couple of steps still expect specific external tools (`makensis` for the installers, `rasm2` for regenerating the binary patch), noted where they apply.
- **Formatting** is handled by [oxfmt](https://www.npmjs.com/package/oxfmt). Run `npm run format` before committing. It formats everything except the `game/` payload (game markdown files are still formatted).
- **Codename Eagle file formats** are handled by the [`cnetool`](https://www.npmjs.com/package/cnetool) package rather than hand-rolled code. Use its API for reading and writing the game's archives, textures and other formats.

## Prerequisites

- [Node.js](https://nodejs.org) 22.19 or newer, then `npm install`.
- [Git LFS](https://git-lfs.com): run `git lfs pull` after cloning, since the large game assets live in Git LFS.
- `makensis` (from [NSIS](https://nsis.sourceforge.io)) to build the installers. On macOS: `brew install makensis`. On Debian and Ubuntu: `apt install nsis`.
- A Rust toolchain with the Windows cross targets for building the tools (see each crate's `.cargo/config.toml`). The release build uses [cargo-xwin](https://github.com/rust-cross/cargo-xwin) for the 32-bit Windows targets.
- `rasm2` (from [radare2](https://radare.org)) only if you regenerate the `ce.exe`/`lobby.exe` byte patch, which assembles a small routine at build time.

## Building

The large assets live in Git LFS, so run `git lfs pull` first.

**Installers** need `makensis`. Build them with `installers/demo/build.sh` and `installers/patch/build.sh`. Pass `--stage-only` to stage and verify the payload without running `makensis`.

**Demo zip** needs only `zip`: `installers/demo-zip/build.sh`.

**Docker image** (build context is the repository root):

```bash
docker build --platform linux/amd64 -f docker-dedicated/Dockerfile -t ce-dedicated:1.50 .
```

**Rust crates** are deliberately not a Cargo workspace: they target different Windows triples and each carries its own `.cargo/config.toml` with the target and linker setup. Run each build from that crate's directory, since `.cargo/config.toml` is read relative to the invocation directory. The exact commands and toolchain notes are in each crate's `.cargo/config.toml` comments and README.

## Documenting changes

The user-facing description of every change lives once, in [`docs/changes.js`](docs/changes.js). A generator writes it into the places it needs to appear:

- the change list and before/after gallery in the root [`README.md`](README.md),
- the "What's new" section of [`game/common/readme150.txt`](game/common/readme150.txt),
- the detailed list in [`patch/README.md`](patch/README.md).

To add or edit a described change:

1. Edit the entry in `docs/changes.js`. Each entry has an `id`, a `title`, a `category`, a `scope` (`all`, `mp`, or `full`), a short `summary` (used for the readme150 list and the root README change list), a fuller `body` (used for the detailed patch README list), and an optional `images` before/after pair.
2. Run the generator:

   ```bash
   node scripts/generate-docs.js
   ```

3. Commit `docs/changes.js` together with the regenerated documents.

The generated regions sit between `<!-- GENERATED:...:start -->` and `<!-- GENERATED:...:end -->` markers in the markdown files, and between the section headings in `readme150.txt`. Do not edit inside those regions by hand; edit `docs/changes.js` and regenerate. Run `node scripts/generate-docs.js --check` to confirm the documents are up to date without writing (it exits non-zero if they are stale).

The byte-level engineering write-up in [`patch/docs/technical-details.md`](patch/docs/technical-details.md) is maintained separately by hand.

## Releases

The [release workflow](.github/workflows/release.yaml) builds every distributable on a Linux runner and publishes a GitHub release with the assets attached: the multiplayer demo installer, the demo zip, and the full-game patch installer. You do not need a local toolchain to cut a release.

Releases are versioned `1.50.NNNN`, where `NNNN` is a zero-padded build number (for example `1.50.0001`). The `1.50` base is the in-game version.

To cut a release, run the workflow manually from the Actions tab (`workflow_dispatch`). It computes the next build number from the latest existing tag, or you can pass a specific build number to override it. It then tags `v1.50.NNNN`, builds everything, and publishes the release.

Two checks cannot run in CI and should be run locally before a release, because they need pristine game installs and a Windows machine:

- the provenance tests (below),
- the Windows install checklist in [`installers/patch/README.md`](installers/patch/README.md).

## Provenance tests

The provenance tests prove that the pre-patched binaries in `game/common/` are exactly stock 1.43 plus this repo's edits, and nothing else. They are ignored by default because they need a pristine 1.43 install. Run them from the `patch/` directory:

```bash
cd patch
CE_PRISTINE_143=/path/to/pristine/1.43 \
CE_GAME_COMMON="$PWD/../game/common" \
cargo test -- --ignored provenance
```

See [`patch/README.md`](patch/README.md) for what each test covers.
