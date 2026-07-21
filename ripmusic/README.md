# ripmusic

Optional companion tool: rips the **Codename Eagle CD's audio tracks** to
`music\` for use with the in-game music patch (`cemusic.dll`). For people who
have the original CD and want the soundtrack in-game without keeping the disc
mounted.

## Usage

```
ripmusic [OUT_DIR] [--drive X | --image FILE]
```

Writes each ripped track to `OUT_DIR\music\<title>.ogg`, using the track's own
title (e.g. `music\The Village Fool.ogg` for CD track 2), or `music\trackNN.ogg`
for any track past the title list. `OUT_DIR` defaults to the current directory;
point it at your game folder to drop the files in place:
`ripmusic "C:\Games\Codename Eagle"`. Then launch the patched game - `cemusic.dll`
looks up single-player campaign tracks (2-13) by this exact title, so the
ripped files just work with no renaming. (If double-clicked, the window pauses
at the end so you can read the result.)

**From a physical CD:** insert the disc and run with no source flag - it
auto-detects the CD-ROM drive holding Codename Eagle (by `codename.ico` / `cutscn\`).
`--drive X` forces a drive letter.

**From a disc image** (no CD drive needed): `--image` a `.cue` (or a `.img`/`.bin`
sitting next to a `.cue`):

```
ripmusic "C:\Games\Codename Eagle" --image D:\rips\CodenameEagle.cue
```

> **Not the `.iso`.** An `.iso` is the _data track only_ and contains no CD audio -
> the ripper will say "no audio tracks". Use the `.cue`/`.img` from the same rip
> (e.g. a CloneCD `.ccd`/`.img`/`.sub` set, or a `.cue`/`.bin`), which hold the
> Redbook audio tracks. You don't need to mount it - point `--image` straight at the
> file.

## What it does

1. Finds the CD drive by marker file/folder.
2. Reads the table of contents (`IOCTL_CDROM_READ_TOC`) and skips the data track.
3. Reads each audio track as raw CD-DA (`IOCTL_CDROM_RAW_READ`, 44.1 kHz/16-bit
   stereo PCM).
4. **Loudness-normalizes** to -14 LUFS with a -1.5 dBTP look-ahead limiter (the CE
   masters are quiet; this matches the hand-made `.ogg` tracks - measured via the
   pure-Rust `ebur128`).
5. Encodes Ogg Vorbis (`vorbis_rs`, VBR ~q6) to `music\<level name>.ogg`, or
   `music\trackNN.ogg` as a fallback (see Usage above).

## Building

Windows-only binary, **32-bit** (no relation to where it runs vs the game - it just
reads the CD and writes files; 32-bit keeps it consistent with the rest of the
toolset). `vorbis_rs` builds bundled C (aoTuV libvorbis), so it needs a C compiler.

**On GitHub CI / native Windows (intended):** trivial - the MSVC toolchain has the
C compiler and archiver.

```
rustup target add i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

**Cross from macOS (for local testing):** needs LLVM (for `clang-cl` + `llvm-lib`)
and a CFLAG to downgrade a clang-strict warning in aoTuV's SSE code that `cl.exe`
ignores:

```sh
brew install llvm cargo-xwin
rustup target add i686-pc-windows-msvc
XW="$HOME/Library/Caches/cargo-xwin/xwin"
PATH="$(brew --prefix llvm)/bin:$PATH" XWIN_ARCH=x86 \
  env "CFLAGS_i686-pc-windows-msvc=--target=i686-pc-windows-msvc -fuse-ld=lld-link -Wno-error=incompatible-pointer-types /imsvc $XW/crt/include /imsvc $XW/sdk/include/ucrt /imsvc $XW/sdk/include/um /imsvc $XW/sdk/include/shared /imsvc $XW/sdk/include/winrt" \
  cargo xwin build --release --target i686-pc-windows-msvc
```

The loudness pipeline is host-testable: `cargo test`, and
`cargo run --example normcheck -- track02.wav` prints input/output LUFS + peak.
