# cemusic

File-based background music for Codename Eagle. `cemusic.dll` plays patent-free
**Ogg Vorbis** tracks from `<gamedir>\music\` instead of the CD Redbook audio the
game originally used, so music works with no CD in the drive - or no CD drive at
all. It is a small 32-bit Rust DLL that ce-patch bundles and installs alongside
`iplist.exe` and `menudll.dll`, and that the patched `ce.exe` loads at startup.

## What it does

- When the game asks for CD track `NN`, the DLL loops `music\trackNN.ogg`
  (e.g. `music\track02.ogg`) instead.
- The in-game **music volume slider works, and controls only music**: playback
  runs on the DLL's own output stream with volume applied as a software gain on
  the music samples, so it never touches the game's DirectSound sound effects or
  the system mixer. (This independent volume is the reason music lives in a DLL
  at all - the engine has no usable per-stream volume for anything it could play
  itself.)
- Everything degrades gracefully: a missing or undecodable track file just means
  silence (logged to `logs\cemusic.log`, never a crash), and if `cemusic.dll`
  itself is absent the game falls back to CD music exactly as before. The DLL is
  purely additive.

## How it works

The engine plays music through MCI `cdaudio`. The ce-patch startup routine loads
`cemusic.dll` and detours the engine's play, stop, and music-volume functions to
the DLL's three exports (falling through to the original CD code if the DLL or
an export is missing). The exports are plain `extern "C"` (cdecl), undecorated:

```
cemusic_play(track: u32)    // loop music\trackNN.ogg; NN = track, zero-padded to 2
cemusic_stop()              // stop and release the current track
cemusic_volume(vol: f32)    // 0.0..1.0 software gain; persists across track changes
```

A few behaviors worth knowing:

- **`cemusic_play` is idempotent per track.** The engine calls its play function
  every frame and normally relies on an internal "already playing" guard that
  the detour bypasses, so the DLL dedupes on the track number itself. A new
  track number stops the old track and starts the new one.
- All exports are non-blocking and safe to call from the game thread repeatedly.
  Panics are caught at the FFI boundary and never unwind into `ce.exe`.

Internally the DLL uses [rodio](https://crates.io/crates/rodio) (cpal for WASAPI
output, a pure-Rust Vorbis decoder - no C codecs, no patented formats). rodio's
`OutputStream` is `!Send`, so a dedicated audio thread owns all the rodio
objects and the exports just post commands to it over a channel. The thread
starts lazily on the first call; `DllMain` does nothing. If no audio output
device exists, commands are drained silently.

The ce.exe-side details (which functions are detoured and how) live in
[`../docs/technical-details.md`](../docs/technical-details.md).

## Building

The DLL loads into the 32-bit `ce.exe`, so it **must be built 32-bit**, for the
`i686-pc-windows-msvc` target. The MSVC C runtime is statically linked (see
`.cargo/config.toml`) so the DLL loads on a clean Windows install with no VC++
redistributable.

On any host (Linux, macOS - including CI), cross-compile with
[cargo-xwin](https://github.com/rust-cross/cargo-xwin), which fetches the MS
CRT/SDK (accepting the Microsoft EULA on your behalf) and links with the
bundled `rust-lld` - no Windows machine and no system LLVM needed:

```sh
rustup target add i686-pc-windows-msvc
cargo install cargo-xwin
XWIN_ARCH=x86 cargo xwin build --release --target i686-pc-windows-msvc
```

`XWIN_ARCH=x86` matters: cargo-xwin fetches x86_64/aarch64 SDK libraries by
default. Build from this directory so cargo picks up the crate's
`.cargo/config.toml`. On an actual Windows machine, plain
`cargo build --release --target i686-pc-windows-msvc` works too.

(Why not `i686-pc-windows-gnu`: commonly packaged i686 mingw-w64 toolchains use
the SjLj exception model while Rust's prebuilt std expects Dwarf-2, which fails
at link time. The MSVC target sidesteps that entirely.)

The artifact lands at `target/i686-pc-windows-msvc/release/cemusic.dll`; it is
bundled by ce-patch and copied into the full-game payload by
`scripts/build-full-payload.js`.
