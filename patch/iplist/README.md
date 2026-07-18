# iplist

`iplist.exe` is a drop-in replacement for the server prober that Codename Eagle's menu (`MENUDLL.DLL`) spawns to populate the in-game **server browser**. The stock prober speaks a legacy query protocol that no modern server answers, which is why the stock browser stays empty. The replacement finds both **internet servers** (from the live community master list) and **LAN servers**, and fills every row with the server's real name, map, player counts, game type, and measured ping.

## What it does

The menu fires a `255.255.255.255` "LAN sweep" invocation each time the player clicks **Refresh list**. (Opening the browser does _not_ auto-query: it shows the previous results, or an empty list on first open, until Refresh is clicked.) The replacement uses that one invocation to do all the discovery, concurrently:

- fetch the community server list over HTTPS (the CEServers.net master list, served from Sanity), and
- listen ~1.5 s on UDP port 210 for the beacons modern CE servers broadcast about once a second on the LAN.

Every discovered host - internet or LAN - is then resolved the same way: a GameSpy `\status\` query on UDP port 4711, which returns the hostname, map name, game type, and player counts. Ping is the measured round-trip of that query. Hosts resolve concurrently, one thread each.

Manual `iplist.txt` entries still work: the menu probes each listed address itself with a one-per-IP spawn of this same binary (a unicast invocation makes a single `\status\` query and emits one row), and the sweep dedupes its central + LAN results against the manual list so nothing shows up twice.

Everything degrades gracefully. Offline (or DNS/TLS failure, or a timeout) just means no internet rows - LAN and manual servers still show. A LAN host that beacons but doesn't answer `\status\` still gets a row built from its beacon (name and player count, blank map). A malformed invocation emits nothing rather than feeding the menu garbage.

## How it works

**Invocation.** The menu spawns `iplist.exe` with six integers: `writeFd mode o1 o2 o3 o4` - a C-runtime pipe fd to write results into, a mode (the menu always sends 1), and the target IP as four octets. `255.255.255.255` selects the LAN sweep; any other address is a single-server probe.

**Output.** One text line per server, in the exact format the menu parses:

```
Name:"<name>" Ping:<ms> Map:255"<map, padded to 15>" Players:<n> MaxPlayers:<n> Spectators:0 MaxSpectators:0 Type:<TYPE> IP:<a>.<b>.<c>.<d> IPXAdress:0.0.0.0.0.0
```

The `Map:255"<name>"` framing is what the patched `menudll.dll` expects when extracting the quoted map name, and the fixed 15-character map column keeps the Players/Type/IP columns aligned for any name length. Names are sanitized (quotes/backslashes stripped) so they can't break the menu's parse.

**The row sink is not stdout.** `writeFd` is a Microsoft C-runtime file descriptor for the write end of a pipe the menu created and passed down via the `STARTUPINFO.lpReserved2` fd-inheritance block. On startup the prober reads that block (`[count][per-fd flag bytes][handle table]`, deriving the handle width from the block size), takes the OS handle at slot `writeFd`, and `WriteFile`s the rows to it. Run standalone - no inherited block, or not on Windows - it falls back to plain stdout, which makes it easy to exercise from a terminal.

The parsing and row-building logic (status replies, beacons, arguments, the fd-inheritance block) is pure and unit-tested; `cargo test` runs natively on any host, no Windows needed.

## Building

A 64-bit Windows binary, cross-compiled with mingw-w64 (available from any Linux or macOS package manager - `apt install mingw-w64`, `brew install mingw-w64`):

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

Build from this directory so cargo picks up the crate's `.cargo/config.toml` (the mingw linker configuration). 64-bit is deliberate even though the game is 32-bit: a 64-bit helper spawns fine from the 32-bit game under WOW64, and Rust's 32-bit `*-windows-gnu` target has an unwinding-model mismatch with common mingw-w64 builds that the 64-bit (SEH) target avoids. TLS is rustls (pure Rust), so no OpenSSL or system TLS is needed at build or run time, and the binary runs on older Windows.

The artifact lands at `target/x86_64-pc-windows-gnu/release/iplist.exe`; ce-patch bundles it and installs it over the stock prober, backing the stock one up.
