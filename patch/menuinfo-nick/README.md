# menuinfo-nick

`menuinfo-nick.exe` sets the **multiplayer player name** stored inside Codename
Eagle's `menuinfo.dat`. The demo installer runs it once, post-copy, with the
name the player typed on the setup wizard's "Multiplayer name" page — so a fresh
demo install shows the player's own name in-game instead of the shipped
`CEDemo` default.

It is an **install-time tool**, not a game file: it never lands in the game
folder and no game binary loads it. Like `ripmusic.exe`, it is compiled fresh at
release time and nothing is committed — see the release workflow.

## Usage

```
menuinfo-nick.exe <path-to-menuinfo.dat> <nickname>
```

The name is normalized: printable ASCII only, `"` removed, trimmed to 10
characters (the length the host broadcasts into multiplayer), falling back to
`CEDemo` when empty. The file is rewritten in place via a temp file + rename, so
a crash mid-write can't leave a corrupt profile. Exit status is non-zero (with a
message on stderr) on any failure; the installer treats that as non-fatal and
keeps the default name.

## The file format

`menuinfo.dat` is three layers deep:

1. an 8-byte header: `u32 uncompressedSize`, `u32 compressedSize` (both LE), then
   the body;
2. the body is a zlib stream wrapped by a per-byte additive cipher (**KEY1**,
   128 bytes, cyclically indexed and applied mod 256);
3. the inflated bytes are the plaintext wrapped by a second additive cipher
   (**KEY2**, 70 bytes).

Decode subtracts (KEY1 outer, then KEY2 inner); encode adds (KEY2 inner, then
KEY1 outer). The plaintext is exactly three 272-byte blocks — `PlayInfo`,
`LevelsDone`, `OptionsMenu` — each a 16-byte NUL-padded tag plus a 256-byte
struct. The player name is the 20-byte field at `PlayInfo + 0x42`; the 40-byte
host-name field at `PlayInfo + 0x1a` is left untouched.

The cipher moduli are the real key lengths (128 / 70) — earlier notes citing
"126 / 69" are wrong and make inflation fail. See `src/lib.rs`.

## Building

Cross-compiled to 64-bit Windows from macOS/Linux via mingw-w64, the same
toolchain as [`../iplist`](../iplist):

```sh
rustup target add x86_64-pc-windows-gnu
brew install mingw-w64          # or: apt-get install gcc-mingw-w64-x86-64
cargo build --release --target x86_64-pc-windows-gnu
```

The demo installer's `build.sh` picks up
`target/x86_64-pc-windows-gnu/release/menuinfo-nick.exe`, or a path from the
`MENUINFO_NICK_EXE` environment variable.

## Testing

```sh
cargo test
```

Tests run natively (no cross-compile) against a committed copy of a real
`menuinfo.dat` in `tests/fixtures/`: a decode/encode round-trip that must
preserve the plaintext byte-for-byte, a surgical-patch check that only the name
field changes, name normalization, and malformed-input rejection.
