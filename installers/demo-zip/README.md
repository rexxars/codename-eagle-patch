# Codename Eagle multiplayer demo - zip package

The extract-and-play counterpart of the [demo installer](../demo/): the exact
same game files (this repo's `game/common/` overlaid with `game/demo/`, all
pre-patched to 1.50), packaged as a plain zip with a top-level
`codename-eagle-mp-demo/` folder. For players who prefer no installer, and for
mirrors that want a plain archive.

What the installer does and this zip deliberately doesn't:

- no Windows Firewall allow-rules (the first server-list refresh and the first
  hosted game each trigger the firewall prompt, which can minimize a
  fullscreen game),
- no machine-wide `cneagle://` protocol registration (the patched `ce.exe`
  still self-registers it for the current user on first launch),
- no desktop shortcut, no Add/Remove Programs entry, no uninstaller.

A bundled `README-zip.txt` tells players about exactly those differences.

## Build

```bash
./build.sh                # -> out/codename-eagle-mp-demo-1.50.zip
./build.sh 1.50 out.zip   # explicit version + output path
```

The payload is staged by `../demo/build.sh --stage-only`, so the payload
definition and its safety checks (git-lfs pointer stubs, case-duplicate paths,
mac cruft) live in one place; this script only adds the note and zips. Keep
the two artifacts in step: rebuild both whenever `game/` changes.

## Testing

Extract on Windows and run `ce.exe` from the extracted folder: the game must
boot to the multiplayer menu, the server browser must list internet servers,
and hosting must work after allowing the firewall prompt. `cneagle://` links
must work after the game has been launched once.
