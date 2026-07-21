# The Codename Eagle patch tool

> Building or contributing? You are in the right place. If you just want to play, use one of the [releases](https://github.com/rexxars/codename-eagle-patch/releases); you do not need anything in this folder.

This is the development tool that produces the Codename Eagle multiplayer and housekeeping fixes. It applies a set of byte edits to the game binaries and ships alongside a few small helper components. The installers apply its output for you, so end users never run it directly.

## End users do not run this

The installers under [`installers/`](../installers/) (the multiplayer-demo installer and the [full-game 1.50 patcher](../installers/patch/)) ship this tool's output **pre-applied** to the game binaries in [`game/common/`](../game/common/). This tool is the source of truth for the binary fixes: how every byte edit is generated and verified.

## One unit: patch + iplist + menudll + cemusic

The [`iplist/`](iplist/), [`menudll/`](menudll/) and [`cemusic/`](cemusic/) subdirectories live inside `patch/` because the server-browser and music fixes only work in combination with the `ce.exe`/`lobby.exe` byte patches this tool applies: the patched menu code launches `iplist.exe` and renders its output, and the patched engine loads `cemusic.dll` for CD-free music. They are built and shipped together as one unit.

[`textool/`](textool/) is the install-time texture patcher: the full-game patch installer bundles it to patch the 1.50 texture fixes (the gas-mask `INTERFC1` HUD atlas, the 32-bit sniper scope, the centered crosshair) into the player's own `24bits` texture archives, instead of shipping whole multi-MB rebuilt archives. Unlike the three components above it does not depend on the byte patches; like [`menuinfo-nick/`](menuinfo-nick/) it is compiled fresh at release time, runs only during install, and never lands in the game folder.

## What it changes and adds

<!-- GENERATED:changes:start -->

### Multiplayer and internet play

- **The in-game server browser finds internet games.** Click Refresh list and it finds live internet servers (from the community server list) and LAN games on your network, and shows each server's real map name and details, so you can see and join games straight from the menu. Before this the browser only ever found LAN games, and internet play meant passing IP addresses around.
- **Hosted games are announced to the community master server.** The community now has its own master server, ceservers.net, in place of the long-dead GameSpy master. Hosting a game announces it there automatically, so it shows up in everyone's server browser (for servers running 1.50 or later). You can also visit ceservers.net at any time to see who is playing online.
- **One-click join with cneagle:// links.** The game registers itself as the handler for cneagle:// links. Click a server link on the community site, in Discord or in a browser and you land straight in that server. This replaces the separate CELauncher tool, which is no longer needed.
- **Dead links updated.** The old codenameeagle.com URLs the game showed on version mismatches and similar now point at the current community site, codenameeagle.net.
- **No more black screen from a stuck session.** Multiplayer runs a background lobby.exe helper that owns the network port. If a previous game crashed, or a server is still running on the same PC, that leftover session keeps hold of the port and leaves you on a black screen when you next host or join. The patch clears out any other running ce.exe and lobby.exe process just before starting a game, so the newest game always launches cleanly. It never touches the game you are launching, and single player is unaffected. Note that starting a game on a machine that is also running a dedicated server will stop that server, so do not host a game on your server box.
- **Dedicated servers idle quietly.** A dedicated server used to peg a CPU core even with nobody connected, because the game loop runs as fast as it can. The patch throttles that loop while the server is empty, dropping idle CPU to a few percent, and switches back to full speed the instant a player joins. Normal players are unaffected.
- **Dedicated servers keep a log of who joins and leaves.** A dedicated server prints its events - server loaded, and players joining, leaving or losing connection - to its console window. That console is invisible when the server runs minimised, in the background or on a headless machine, and the lines could not be redirected to a file. The patch now also appends each of those lines to logs\server.log, so there is always a record of what happened on the server, however it was started. The console itself is unchanged. This applies to dedicated servers only (started with +dedicated); a normal game you host from the menu does not produce these events and gets no server.log.

### Gameplay and balance

- **The "8 trick" no longer works.** Re-selecting the weapon already in your hands (or switching away and back, usually macroed to a mouse button) used to cancel the weapon's fire delay, letting the bazooka, sniper rifle, grenade and gas fire roughly twice as fast as intended. Fire delays are now tracked per weapon: the weapon that fired keeps its own cooldown no matter how you switch, so the trick gains nothing, while switching to a different weapon fires as soon as it is raised, exactly like vanilla. Quick combos such as placing an explosive and immediately switching to the detonator still work at full speed.
- **The spontaneous explosion bug (SEB) is fixed.** Since v1.41, firing at the ground could randomly kill the shooter: a bullet that slipped through the terrain mesh and sank past the map's deepest point tripped an engine branch that dealt 3000 damage to its owner. The unofficial v1.42 only hid the bug on the stock multiplayer maps with invisible guard planes under the terrain, so it lived on in single player and on custom maps. The patch fixes the engine itself: a stray out-of-world projectile is now silently removed instead of detonating its shooter, everywhere, on every map. The old guard planes are removed from the shipped multiplayer levels, restoring the stock level geometry.
- **Single-player turret balance restored (full game).** In v1.41 the base turrets (pillbox, bunker and armored-car cannons) fired a far stronger gun in single player than in v1.36, triple the damage and projectile speed. That was really a multiplayer balance change that leaked into single player and made the campaign brutally hard. The patch restores the single-player turret to its v1.36 strength. Multiplayer is untouched (it uses a separate weapon table), so versus and co-op balance for the armored car, torpedo boats and helicopter stays exactly as in v1.41.

### Stability and housekeeping

- **A tidy game folder.** Log files now go in a logs\ folder, screenshots in a screenshots\ folder, and savegames (sg0.dat and friends) in a saves\ folder, instead of being scattered in the game directory or written to your C:\ drive, which modern Windows can block. Any existing savegames are moved into saves\ for you, so nothing is lost. The game also stops leaving stray player1.txt, player2.txt diagnostic files behind, and starts correctly from shortcuts and links regardless of working directory.
- **A sharper icon.** The game icon is now embedded directly in ce.exe instead of shipping as a separate ce.ico file, so Windows shows it everywhere the executable appears: Explorer, the taskbar, alt-tab and shortcuts, not only where a shortcut pointed at an icon file. The embedded icon is a hybrid: classic bitmap frames for the small sizes, which older systems such as Windows XP can display, plus a high-resolution 256x256 image that stays crisp on modern high-DPI displays. A second, classic icon is embedded alongside it as a selectable alternate: in a shortcut's Change Icon dialog you can point it at ce.exe and choose that one instead. Stock ce.exe carried no icon at all and showed the generic Windows executable icon.
- **Single-player crash near enemy bases fixed (full game).** The long-standing "floating point error" that could hit when you approach an enemy base, a bug in the mounted-turret code introduced in v1.41, is fixed, so the base guns behave normally.
- **Save-game crash on modern graphics cards fixed (full game).** Saving builds a small thumbnail by grabbing the screen, and on modern graphics cards a step of that grab could fail and crash the game. With the bundled dgVoodoo the grab succeeds and the thumbnail appears as normal. The change is a safe guard: it leaves the capture itself untouched and only removes the crash on the failure path, so in the rare case the grab does fail the game saves without a thumbnail instead of crashing.
- **Fortress terrain mesh repaired.** Fortress shipped with 19 defects in its terrain mesh: seven spots where two vertices sit less than one world unit apart, forming invisible slivers and near-vertical shoreline walls that the engine's water-pairing pass (InitWater) treats as fatal ("two land faces or two sea faces, nErrors=19") whenever it rebuilds the level's wcache.bin. Every stock install masked this by shipping the cache file, so deleting it made the level unloadable. The repair nudges 14 vertices by 0.05-0.13 world units, far too small to see; the mesh now validates cleanly, and the water/land pairing the game rebuilds from it is identical to the stock cache (2453 of 2453 pairs), so the level plays exactly as before. The cache file no longer ships: Fortress rebuilds it on first load, the same as every other level.

### Graphics and display

- **The aiming crosshair scales with resolution.** The screen-center crosshair was a fixed 8-pixel sprite, which is tiny and hard to see at modern resolutions (it was sized for the roughly 480p displays of 1999). It now scales with the screen height, so it stays the same apparent size, and stays visible, at 1080p, 1200p and beyond.
- **The sniper scope is smoother.** The scope overlay was a 24-bit texture whose transparency came from the engine's binary black color-key, so the lens edge and the reticle showed hard staircase pixels when upscaled to modern resolutions. It is replaced with a 32-bit version with real antialiased alpha: a smooth lens edge and clean reticle lines that read thinner at the center.
- **Better screenshots: full resolution, timestamped names, and F11.** Screenshots used to be scaled down to 640x480 no matter what resolution the game was running at, and were named shot1.tga, shot2.tga and so on with a counter that restarted on every launch, overwriting the previous session's shots one by one. They are now saved at the actual render resolution - a 1080p game produces a 1920x1080 screenshot - and named by date and time (screenshots\shot-20260720-153045-123.tga), so nothing is ever overwritten and shots from any number of sessions sort in the order they were taken. F11 also takes a screenshot: on Windows 11 the Print Screen key opens the Snipping Tool by default, which knocks the game out of fullscreen (if you prefer Print Screen, turn that off under Settings > Accessibility > Keyboard). F12 still opens the console, and savegame thumbnails are unaffected.
- **dgVoodoo graphics wrapper included.** The patch ships the dgVoodoo graphics wrapper, which fixes a number of rendering issues on modern versions of Windows and makes options like anti-aliasing easy to enable. Run dgVoodooCpl.exe in the game folder to configure the settings. The installers include it by default and let you opt out; the demo zip always includes it. If you keep it, your own tuned dgVoodoo.conf is left in place on reinstall.
- **Multiplayer-demo menu options show their selected state.** The old multiplayer-demo repack shipped a trimmed set of menu graphics that left out several of the images the menu draws for a selected option, so those options showed nothing when chosen and there was no way to tell they were active. The patch restores the missing images from the full game, fixing the selected-state graphic for: 16 channel audio, the 3Dfx display driver, the None and Medium GraphicFX settings, Invert Mouse, and the red team on the Join Game screen. This only affects installs of the old multiplayer demo; the full game always shipped the complete set.

### Single player (full game)

- **The gas mask in "Demolition Man" shows in your inventory.** In the "Demolition Man" mission the village elder hands you a gas mask, but stock only ever played the line and equipped it invisibly, so players kept looking for an item that never appeared. It now shows up as an inventory item with its own icon. This is purely cosmetic: the mask works exactly as before (you never had to select or use it), this just makes clear that you received it.
- **No more videos at every launch, and the campaign intro plays where it belongs.** With the CD cutscenes copied into the game folder, the game used to replay three videos at every single launch: the Refraction Games logo, the game story, and the campaign intro. The patch skips all three at startup and instead plays the game story and the campaign intro at the mission 1 opening, the first time you start the campaign in a session. The menu's view intro and view credits options still work. Because the opening now plays as the campaign starts, copy the CD's cutscenes into the game folder before playing the campaign: as with any Codename Eagle cutscene, if the file is missing the game asks for the CD when it would play.

### Music (full game)

- **No CD needed, and no CD crashes.** The game can play its soundtrack from music files in a music\ folder instead of the CD, with its own volume that the in-game music slider controls without affecting sound effects. It also fixes a crash on launch that could happen with a Codename Eagle CD in the drive (notably when running the multiplayer demo with a CD inserted), so the game no longer depends on or trips over the disc. The music playback (cemusic.dll) is part of the full game only; the crash fix always applies.

<!-- GENERATED:changes:end -->

## Applying it manually

The installers apply everything for you. To apply the fixes by hand instead:

1. Put `patch.exe`, `iplist.exe`, `menudll.dll` and `cemusic.dll` in your Codename Eagle folder (the one with `ce.exe`).
2. Run `patch.exe` (double-click it).
3. That is it. It updates the game, makes a backup of every file it touches, and tells you what it did.

## Music (optional)

Single player has a soundtrack that shipped on the original Codename Eagle CD. To play it without the CD, put Ogg Vorbis files named `track02.ogg`, `track03.ogg` and so on in a `music\` folder inside your Codename Eagle folder, named after the CD track numbers (CD audio track 2 becomes `music\track02.ogg`). The game loops the matching track just like the CD did, and the in-game music-volume slider controls it. Any track without a file is silent. With no `music\` folder (or with `cemusic.dll` removed) the game tries to use the CD as before.

The crash fix is separate from this and always applies: even with no `music\` folder, the patched game does not crash on launch with a CD in the drive, because it loads its assets from the game folder rather than the disc.

## Compatibility

The patcher in this directory works with Codename Eagle 1.41, 1.42 and 1.43. It checks each file before touching it and safely skips anything it does not recognise, so it will not damage an unexpected version. The installers ship full files for the most part, so they work with any Codename Eagle version.

## For developers

How it all works under the hood is in [docs/technical-details.md](docs/technical-details.md). The `ce.exe`/`lobby.exe` patch bytes are generated by `make-ce-patch.js`; the game icon is then embedded into the patched `ce.exe` by [`scripts/embed-ce-icon.js`](../scripts/embed-ce-icon.js) as a `.rsrc` section (appended, so it leaves the patched code and data untouched): [`patch/assets/ce.ico`](assets/ce.ico) as the default (a hybrid `.ico` with XP-safe bitmap frames plus a 256x256 PNG) and [`patch/assets/cespy.ico`](assets/cespy.ico) as a selectable alternate (icon group 2, offered in a shortcut's Change Icon dialog); the bundled `menudll.dll` (map-name display, relocation fix, `menu.log` and `saves\` redirects) is reproduced from stock by `menudll/make-menudll-patch.js`; `iplist.exe` is built from the [`iplist/`](iplist/) crate; `cemusic.dll` (file-based Ogg Vorbis music) is built from the [`cemusic/`](cemusic/) crate. Build the patcher with `cargo build --release --target x86_64-pc-windows-gnu`. Ship it as a folder containing `patch.exe`, `iplist.exe`, `menudll.dll`, `cemusic.dll` and this README, and run it pointed at the game folder (`patch.exe C:\path\to\CE`) so the originals get backed up.

### Provenance tests

Ignored-by-default tests prove that the pre-patched binaries shipped in [`game/common/`](../game/common/) (`ce.exe`, `lobby.exe`, `data4.bin` and `menudll.dll`) are exactly stock 1.43 plus this repo's edits and nothing else. Each test copies the pristine 1.43 file into a temp dir, runs the same patch routine the tool runs, and requires byte-for-byte equality with the shipped file. Two need `node` on PATH: `menudll.dll` is reproduced by the `menudll/make-menudll-patch.js` generator, and `ce.exe` additionally has its icon embedded by `scripts/embed-ce-icon.js` (the same final step that produces the shipped binary). On a mismatch it reports the differing byte count and first differing offset. Run them with two env vars pointing at a pristine 1.43 install and the repo's `game/common`:

```bash
CE_PRISTINE_143=/path/to/pristine/1.43 \
CE_GAME_COMMON=/path/to/repo/game/common \
cargo test -- --ignored provenance
```

The Windows setup wizards that distribute this patch's output to end users (game files, shortcuts, firewall rules and an uninstaller, built with NSIS from macOS or Linux) live under [`installers/`](../installers/).
