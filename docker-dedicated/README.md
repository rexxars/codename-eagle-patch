# docker-dedicated

Codename Eagle 1.50 dedicated server in a Docker container, running under Wine.
The game files are baked into the image straight from this repo's `game/` dir
(no staging/sync step), so the build context is the repo root.

## Quick start

```sh
cd docker-dedicated
docker compose up --build
```

Or without compose, from the repo root:

```sh
docker build --platform linux/amd64 -f docker-dedicated/Dockerfile -t ce-dedicated:1.50 .
docker run --rm -p 24711:24711/udp -p 4711:4711/udp ce-dedicated:1.50
```

Or run the prebuilt image from GHCR (published by `.github/workflows/ci.yaml`
on pushes to `main`; needs `docker login ghcr.io` while the repo is private):

```sh
docker run --rm -p 24711:24711/udp -p 4711:4711/udp ghcr.io/rexxars/codename-eagle-patch:latest
```

**Publish the UDP ports 1:1 - do not remap them to a different host port.** The
server announces _its own_ port to clients: the GameSpy `\status\` reply and the
master heartbeat carry the game port it bound (`hostport`, default `24711`) and
the fixed query port (`4711`). If you map a container port to a different host
port (e.g. `-p 25000:24711/udp`), players reach you on `25000` but the server
still advertises `24711`, so the in-game browser and the community master hand
them an unreachable port. Keep both sides equal (`-p 24711:24711/udp`,
`-p 4711:4711/udp`). To run on a non-default game port, set `CE_PORT` to it and
publish that same number on both sides (e.g. `CE_PORT=30000` with
`-p 30000:30000/udp`), so the announced port matches the reachable one. The
query port `4711` is fixed and cannot be changed, so always publish it as-is.

The server's logs are streamed to the container stdout, each line tagged with
its source:

- `[server]` — the dedicated-server events: server loaded, players joining,
  leaving and losing connection. The engine only ever prints these to its
  dedicated console (`WriteConsoleA`), which is discarded on a headless host, so
  the 1.50 patch also appends them to `logs\server.log`; that is what this
  streams. This is the stream you usually want.
- `[lobby]` — the background lobby helper's `logs\lobby.log`.
- `[error]` — the engine's `logs\error.log`. Off by default because it is very
  noisy; set `CE_LOG_ERROR=1` to include it. A healthy boot ends with `World and
sounds loaded` / `TotalLoadTime`; a failed one shows an `Error: %s` line, so
  turn it on when a server won't start.

The server answers GameSpy `\status\` queries over UDP on the query port (4711 by default), which is how the in-game server browser and the community master read its name, map and player count.

## Configuration

Environment variables (see `docker-compose.yml`):

| Variable        | Default        | Meaning                                  |
| --------------- | -------------- | ---------------------------------------- |
| `CE_HOSTNAME`   | `CE Dedicated` | Server name (max 39 chars)               |
| `CE_MAP`        | `No mans land` | Level name as listed in `levels.nfo`     |
| `CE_MAXPLAYERS` | `16`           | 2-30                                     |
| `CE_GAME`       | `deathmatch`   | `deathmatch`, `ctf` or `teamplay`        |
| `CE_PORT`       | (24711)        | Optional `+host <port>` override         |
| `CE_LOG_ERROR`  | `0`            | `1` also streams `error.log` (`[error]`) |

## LAN discovery (host networking)

A container on Docker's **default bridge network will not show up in LAN server
scans**, even with the ports above published. LAN discovery is push-based: the
host broadcasts a small `'D'` beacon from the game port to
`255.255.255.255:210` about once a second, and scanners listen on UDP `210`.
That beacon is an **outbound broadcast**, and broadcasts to `255.255.255.255`
are link-local - they do not cross the bridge NAT, so the datagram stays inside
the container's private subnet and never reaches your LAN. Publishing port
`210` does not help: `-p` is an inbound-unicast rule, the wrong direction and
cast for a beacon. (Even if the beacon did leak, the scanner keys the server's
address off the datagram's source IP, which on the bridge is the container's
internal `172.x` - unreachable for the follow-up `\status\` query. The server
needs a real presence on the LAN, not just the broadcast forwarded.)

To be discoverable on the LAN, put the container on the physical segment:

- **Host networking (simplest, Linux host).** Drop the `ports:` block and run
  with the host network stack, so the beacon leaves on the real NIC with the
  host's IP as its source:

  ```yaml
  services:
    ce-server:
      network_mode: host
      # remove the `ports:` mapping - it is ignored (and unnecessary) with host networking
  ```

  Or with `docker run`: `--network host` in place of the `-p` flags.

- **macvlan.** Give the container its own MAC/IP directly on the LAN if you
  want it to look like a distinct box and keep network isolation.

**Docker Desktop (macOS/Windows) cannot do LAN discovery.** Docker there runs
inside a Linux VM; neither host networking nor a broadcast crosses from that VM
onto the host's physical LAN, so a Desktop-hosted server won't appear in LAN
scans regardless of flags. Use the internet path instead: the 1.50 server
heartbeats to the community master (`27900/udp`, outbound), so it shows up in
the internet server list once listed, and its `\status\` on the published
`4711/udp` is directly queryable by IP. Internet discovery and direct queries
work from any networking mode - only the LAN beacon needs host/macvlan
networking.

## Design notes

- **Dedicated mode is real.** `ce.exe +dedicated +host` uses the engine's own
  server path: it skips the menu, cutscenes and video init, minimizes the
  window, allocates a "Codename: Eagle server console" and retitles the window
  "Codename: Eagle dedicated server". (`+join` clears dedicated mode, so this
  is host-only by construction.)
- **Xvfb is still required.** Dedicated mode skips _rendering_ init but the
  engine still creates and manipulates its window, so a display must exist.
- **Launched via a generated `.bat`, not argv.** The engine's string flags
  (`+game`/`+hostname`/`+map`/`+name`) require literal double quotes in the
  raw Windows command line, but Wine only re-quotes argv elements containing
  spaces - `wine ce.exe +game deathmatch` silently drops the flag (and
  `start "" /wait` re-parses argv, same problem). The entrypoint writes
  `c:\ce-launch.bat` and runs it with `wine cmd /c`, which passes the line
  verbatim and waits for the GUI process.
- **Audio is disabled** via `HKCU\Software\Wine\Drivers\Audio=""` during prefix
  init - there is no audio device in the container.
- **Ports:** `24711/udp` game/session traffic (hand-rolled reliable-UDP via
  `lobby.exe`, spawned by `ce.exe`), `4711/udp` GameSpy status queries.
  Outbound, the server heartbeats to the master on `27900/udp`.
- **Console capture (`[server]`).** `ce.exe +dedicated` prints its server events
  (loaded, player join/leave/lost connection) only with `WriteConsoleA` into a
  console it opens via `AllocConsole`. Under headless Wine that `AllocConsole`
  yields no usable handle, so every line is written to a NULL handle and lost;
  `>`/`|`/`tee` never see it either, since `WriteConsoleA` targets the console
  screen buffer, not a byte stream. The 1.50 patch fixes this at the source: it
  detours that one `WriteConsoleA` call site to also append the line to
  `logs\server.log` (see fix #18 in `patch/docs/technical-details.md`), which the
  entrypoint tails as `[server]`. No helper process or console is involved, so it
  works the same headless as on a real Windows console.
- **Game files are baked into the image** from the repo's `game/` dir, so the
  build context is the repo root (`context: ..` in compose). The
  `Dockerfile.dockerignore` (BuildKit) keeps `.git`, `installer/` and the rest
  of `docker-dedicated/` out of the context.
- **amd64 only:** ce.exe is 32-bit Win32, so the image needs `wine32:i386` on
  `linux/amd64`. On Apple Silicon, Docker Desktop runs it via Rosetta.
- The wine prefix lives in the `wineprefix` volume so first-boot init happens
  once. The engine's logs stream to stdout, so `docker logs` is the record for
  postmortems.

## Known limitations

- No graceful shutdown: `docker stop` SIGTERMs Wine; the engine has no
  shutdown handler. Harmless for a game server (no persistent state).
- `Error open cd` in the log is benign (no CD drive for CD audio).
- The GameSpy status reply reports `maxplayers` as N-1 (the dedicated host's
  reserved slot is subtracted), e.g. `CE_MAXPLAYERS=16` shows as 15.
