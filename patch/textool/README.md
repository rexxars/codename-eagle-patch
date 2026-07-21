# textool

`textool.exe` adds or replaces **TGA textures** inside Codename Eagle's texture
archives (`24bits/textures.dat`, `24bits/texsec.dat`). The patch installer runs
it at install time, so the shipped archives stay untouched in the repo and only
the player's installed copies are edited.

It is an **install-time tool**, not a game file: it never lands in the game
folder and no game binary loads it. Like `menuinfo-nick.exe`, it is compiled
fresh at release time and nothing is committed.

## Usage

```
textool set <archive.dat> <texture.tga>...   add or replace TGAs (entry name = basename)
textool list <archive.dat>                   NAME  WxHxDEPTH  <blob bytes>
```

Exit codes: 0 ok, 1 runtime error (the archive is never touched), 2 usage
error. `set` validates and upserts every staged TGA in memory first, then
rewrites the archive with one **atomic** write (temp sibling file + fsync +
rename) - on any error nothing is written, so a failed or interrupted run can
never leave a half-written archive.

## The archive format

Both archives share one container, all integers little-endian:

- offset 0: u32 entry count (used slots);
- offset 4: a **fixed 2048-slot TOC**, regardless of count, of 17-byte
  records: a 13-byte name field (NUL-terminated Latin-1, max 12 name bytes;
  bytes after the NUL are don't-care, canonical fill `0xCC`) followed by a u32
  **absolute** blob offset - no length field: a blob runs to the next entry's
  offset, the last one to EOF;
- offset 34820 (= 4 + 2048×17): the blobs, canonically concatenated in TOC
  order.

A blob is a standard uncompressed true-color TGA minus its first 8 constant
bytes (`00 00 02 00 00 00 00 00`). The reader is lenient (shipped originals
carry garbage padding and non-canonical blob placement; offsets are
bounds-checked and must be monotonic); the writer emits the canonical layout,
mirroring cnetool's `buildTextureArchive`.

## Input TGAs: validation, orientation, names

Inputs must be uncompressed true-color (type 2), 24- or 32-bit, square,
power-of-two sized (1–1024), with no image ID, color map, RLE or trailing
bytes (e.g. a TGA v2 footer) - anything else is refused rather than risk
corrupting the archive.

Blobs are stored **verbatim** (only the 8-byte prefix is stripped): textool
never reorients pixels. The archives store pixel rows **top-down behind a
bottom-origin descriptor** (the engine reads rows in file order and ignores
the origin bit), so inputs must already be in engine row order - cnetool's
`pngToTga({topDown: true})` convention, used for the authored TGAs in
`game/full-overrides/`.

The entry name is the staged file's **basename**, required to be plain ASCII
and at most 12 bytes (the stored names are Latin-1, and every real texture
name is ASCII). Replacement matches existing entries case-insensitively and
**preserves the stored name**: stock archives store e.g. `TARGET.tga` while we
stage `Target.tga`. When nothing matches, the entry is appended (refused if
all 2048 TOC slots are used).

## Building

Cross-compiled to 64-bit Windows from macOS/Linux via mingw-w64, the same
toolchain as [`../iplist`](../iplist):

```sh
rustup target add x86_64-pc-windows-gnu
brew install mingw-w64          # or: apt-get install gcc-mingw-w64-x86-64
cargo build --release --target x86_64-pc-windows-gnu
```

## Testing

```sh
cargo test
```

Tests run natively (no cross-compile): unit tests for the format code and CLI
integration tests against synthetic archives.

The **pristine tests** are ignored by default because they need a real
pristine 1.43 install (and the textures.dat one copies a ~151 MB file). They
prove that a `set` run like the installer's replaces exactly the requested
entries and leaves every other entry byte-identical:

```sh
CE_PRISTINE_143=/path/to/pristine/1.43 \
cargo test -- --ignored --test-threads=1
```
