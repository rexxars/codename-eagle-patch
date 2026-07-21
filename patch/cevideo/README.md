# cevideo

`smackw32.dll` is a drop-in replacement for Codename Eagle's Smacker video
library (RAD's `SMACKW32.DLL`) that lets cutscenes play from a modern, open
**WebM** video (AV1 video + Vorbis audio) in addition to the game's original
`.smk` files. It is a small 32-bit Rust DLL that ce-patch bundles with the
**full game** and the patched `ce.exe` loads in place of the stock library.

Its main purpose is to **allow** WebM cutscenes — for new or re-encoded
cutscenes a mod/author drops in — without disturbing anything that ships stock.
It is not required, and it never converts or replaces the game's own videos on
its own: with no `.webm` files present it behaves exactly like the original
library.

## Overriding / replacing a cutscene with WebM

For any cutscene the game plays from `cutscn\<NAME>.smk`, put a
`cutscn\<NAME>.webm` next to it and the game plays the WebM instead. If no
matching `.webm` exists, the call is forwarded to the original Smacker library
(shipped alongside as `smackw32_orig.dll`), so the `.smk` plays exactly as
before. So:

- **Add a new cutscene in WebM** — ship `cutscn\<NAME>.webm` (no `.smk` needed
  if the game only ever references `<NAME>.smk`; the shim resolves the sidecar
  from the requested `.smk` path).
- **Replace an existing cutscene** — drop a `cutscn\<NAME>.webm` beside the
  stock `<NAME>.smk`; the WebM wins, the `.smk` stays as the fallback.
- **Leave it stock** — ship no `.webm`; the original `.smk` plays via
  `smackw32_orig.dll`.

**Encode as AV1 video + Vorbis audio in a WebM container, preserving the source
clip's exact frame count and frame rate 1:1.** Codename Eagle renders subtitles
at runtime by frame index (from `DIALOGUE.DAT`), so a re-timed clip would drift
out of sync. `scripts/transcode-cutscenes.js` does a parity-guarded
`.smk → .webm` conversion (it refuses any output whose frame count or fps
differs from the input) and is the easy way to re-encode an existing cutscene.

## Requirements

- **`smackw32_orig.dll`** — the stock Smacker DLL, renamed. The shim
  `LoadLibrary`s it to play `.smk` files and to forward any Smacker call it does
  not handle. Without it, only `.webm` cutscenes play and `.smk` cutscenes are
  skipped (the engine's normal missing-file behaviour). ce-patch ships it
  automatically for full-game installs.
- Full game only. The shim is bundled into `game/full/` (see
  `scripts/build-full-payload.js`) and installed only when the patcher detects a
  full-game install — never on the multiplayer demo.

## How it works

The engine drives the Smacker playback loop itself, reading fields out of the
Smacker context struct and blitting decoded frames into its own DirectDraw
surface (subtitles are drawn on top by the engine). The shim re-exports the nine
Smacker functions the engine imports and, for a `.webm` cutscene, presents an
ABI-compatible fake context whose frames it decodes itself; every other handle
is forwarded to `smackw32_orig.dll`. The engine's subtitle rendering, skip and
boot-reel behaviour are untouched, and `ce.exe` is not patched.

Decoding is pure Rust — [`rav1d`](https://crates.io/crates/rav1d) (AV1, a
maintained dav1d port), `matroska-demuxer` (WebM) and `symphonia` (Vorbis) — so
the DLL is self-contained with no C codec or ffmpeg dependency. Audio plays on
the shim's own output stream via [rodio](https://crates.io/crates/rodio) (the
same approach as `cemusic`). AV1 decode runs on a dedicated large-stack worker
thread: `ce.exe`/`game.exe` is 32-bit with a ~1 MiB main-thread stack, which
inline AV1 decode overflows.

## Building

The DLL loads into the 32-bit `ce.exe`, so it **must be built 32-bit**, for the
`i686-pc-windows-msvc` target, with the MSVC C runtime statically linked (see
`.cargo/config.toml`) so it loads on a clean Windows install. Cross-compile from
any host with [cargo-xwin](https://github.com/rust-cross/cargo-xwin):

```sh
rustup target add i686-pc-windows-msvc
cargo install cargo-xwin
XWIN_ARCH=x86 cargo xwin build --release --target i686-pc-windows-msvc
```

`XWIN_ARCH=x86` matters (cargo-xwin defaults to x86_64/aarch64 SDK libraries).
The artifact lands at `target/i686-pc-windows-msvc/release/smackw32.dll` and is
bundled into the full-game payload by `scripts/build-full-payload.js`.
