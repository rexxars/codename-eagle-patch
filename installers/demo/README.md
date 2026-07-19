# Codename Eagle MP demo installer

A classic Windows setup wizard for the multiplayer demo, built entirely from
macOS/Linux with [NSIS](https://nsis.sourceforge.io/) (`brew install makensis`).

The payload is this repo's `game/common/` folder overlaid with `game/demo/`,
shipped as-is: the binaries in it are already patched (master-server redirect,
`cneagle://` handler, menudll, iplist, dgVoodoo), so unlike the ce-patch
installer there is no patch step at build or install time.

## Build

```bash
./build.sh                # -> out/codename-eagle-mp-demo-1.50-setup.exe
./build.sh 1.50 out.exe   # explicit version + output path
```

The script stages `../../game/common` merged with `../../game/demo`, strips
`.DS_Store`/`*.bak`, refuses to ship git-lfs pointer stubs (run `git lfs pull`
first) or case-duplicate paths, and compiles `installer.nsi`. The six dgVoodoo
files are staged into their own dir so the installer can offer them as an
optional component; the main file copy skips them. Pass `--stage-only` as the
first argument to stage and verify the payload without running makensis (the
staging dir is printed and kept for inspection).

The wizard also bundles [`menuinfo-nick.exe`](../../patch/menuinfo-nick), the
helper that writes the player's chosen multiplayer name into `menuinfo.dat`. It
is compiled fresh (never committed); `build.sh` picks it up from
`patch/menuinfo-nick/target/x86_64-pc-windows-gnu/release/`, or from
`MENUINFO_NICK_EXE`, and errors with the build recipe if it's absent. It is only
needed for the makensis step, so `--stage-only` (and the demo zip) don't require
it.

## What the setup.exe does

1. Copies the payload to the install folder - default `C:\Games\Codename Eagle`,
   deliberately **not** Program Files, because the game writes saves, logs and
   screenshots into its own folder and a non-elevated player can't do that under
   Program Files.
2. Adds three Windows Firewall allow-rules, `Codename Eagle (game)`,
   `Codename Eagle (lobby)` and `Codename Eagle (server browser)`, via `netsh
advfirewall`. This is why the installer requests elevation: without the
   rules, hosting the first game pops the firewall consent dialog, which
   minimizes the fullscreen game mid-handshake and strands it on a black
   taskbar preview, and `iplist.exe`'s LAN discovery (an inbound UDP :210
   listen) pops the same dialog on the first server-list refresh.
3. Registers the `cneagle://` URL protocol machine-wide
   (`HKLM\Software\Classes\cneagle` -> `"<instdir>\ce.exe" %1`), so one-click
   join links work before the game has ever been launched. `ce.exe` still
   re-registers itself per-user (`HKCU`) on every launch; the per-user key takes
   precedence and the two coexist.
4. Creates Start Menu shortcuts (plus an optional Desktop one) using `ce.exe`'s
   embedded icon, with the game folder as the working directory, writes the
   Add/Remove Programs entry, and drops an uninstaller.

A **Multiplayer name** wizard page (between the directory and install pages)
asks for a name of up to 10 characters, prefilled with `CEDemo`. After the
payload is copied, `menuinfo-nick.exe` (run from `$PLUGINSDIR`, never installed
into the game folder) writes that name into `menuinfo.dat`, so a fresh install
shows the player's own name in-game. The name is normalized (printable ASCII,
`"` removed, trimmed to 10, empty falls back to `CEDemo`), and a failure here is
non-fatal — the install continues with the default name.

The components page offers the **dgVoodoo graphics wrapper (recommended)** as a
separate component, checked by default. It bundles the six dgVoodoo files
(`dgVoodooCpl.exe`, `D3D8.dll`, `D3D9.dll`, `D3DImm.dll`, `DDraw.dll` and
`dgVoodoo.conf`), which fix rendering problems on modern Windows and make
options like anti-aliasing easy to turn on. Unchecking it installs none of the
six. When it is checked, `dgVoodoo.conf` is written only if it is absent, so a
config you tuned earlier survives a reinstall. It also installs a `dgVoodoo.txt`
notice that explains what dgVoodoo is, where it comes from, and how to remove it.

The uninstaller removes the firewall rules, the shortcuts, both protocol keys
(`HKLM` and `HKCU\Software\Classes\cneagle`) and the whole game folder including
saves/logs/screenshots. It refuses to delete a folder that doesn't contain
`ce.exe`.

## Testing

- Real Windows: run the setup exe, then check `netsh advfirewall firewall show
rule name="Codename Eagle (lobby)"`, open a `cneagle://<ip>:<port>/` link
  without ever launching the game, refresh the server list and host a game -
  no firewall popup for either.
- Quick smoke test of the wizard/paths also works under Wine/CrossOver, but the
  `netsh` calls and protocol launch only mean anything on real Windows.

## Caveat

The setup exe is unsigned, so SmartScreen shows "Windows protected your PC" on
first download ("More info" -> "Run anyway"). Code signing is a separate
project.
