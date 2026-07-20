// Single source of truth for the user-facing description of every change in the
// 1.50 community patch. The prose here is generated into the other places it
// needs to appear (the root README change list, game/common/readme150.txt, and
// patch/README.md) by scripts/generate-docs.js. Edit a change here, run that
// script, and every document stays in sync.
//
// Fields per entry:
//   id        stable slug, used for anchors and ordering
//   title     short headline for the change
//   category  one of the keys in `categories` below
//   scope     'all'  applies to every build
//             'mp'   multiplayer demo and dedicated server
//             'full' full game only (single player and music)
//   summary   one or two sentences, used for the root README change list and the
//             readme150 "What's new" list
//   body      the fuller explanation, used for the detailed list in
//             patch/README.md
//   images    optional before/after screenshot pair (paths relative to repo root)

export const categories = [
  {key: 'multiplayer', title: 'Multiplayer and internet play'},
  {key: 'gameplay', title: 'Gameplay and balance'},
  {key: 'stability', title: 'Stability and housekeeping'},
  {key: 'graphics', title: 'Graphics and display'},
  {key: 'singleplayer', title: 'Single player'},
  {key: 'music', title: 'Music'},
]

export const changes = [
  {
    id: 'server-browser',
    title: 'The in-game server browser finds internet games',
    category: 'multiplayer',
    scope: 'mp',
    summary:
      'Click Refresh list and the browser finds live internet servers and LAN games, with real server names, map names and player counts, so you can join straight from the menu.',
    body: "Click Refresh list and it finds live internet servers (from the community server list) and LAN games on your network, and shows each server's real map name and details, so you can see and join games straight from the menu. Before this the browser only ever found LAN games, and internet play meant passing IP addresses around.",
  },
  {
    id: 'master-announce',
    title: 'Hosted games are announced to the community master server',
    category: 'multiplayer',
    scope: 'mp',
    summary:
      'Hosting a game announces it to the community master server (ceservers.net), the replacement for the long-dead GameSpy master, so friends can find it.',
    body: "The community now has its own master server, ceservers.net, in place of the long-dead GameSpy master. Hosting a game announces it there automatically, so it shows up in everyone's server browser (for servers running 1.50 or later). You can also visit ceservers.net at any time to see who is playing online.",
  },
  {
    id: 'one-click-join',
    title: 'One-click join with cneagle:// links',
    category: 'multiplayer',
    scope: 'mp',
    summary:
      'The game registers itself as the handler for cneagle:// links, so a server link from the community site, Discord or a browser launches the game and drops you straight into that server.',
    body: 'The game registers itself as the handler for cneagle:// links. Click a server link on the community site, in Discord or in a browser and you land straight in that server. This replaces the separate CELauncher tool, which is no longer needed.',
  },
  {
    id: 'dead-links',
    title: 'Dead links updated',
    category: 'multiplayer',
    scope: 'all',
    summary:
      'The old URLs the game showed on version mismatches and similar now point at the current community site, codenameeagle.net.',
    body: 'The old codenameeagle.com URLs the game showed on version mismatches and similar now point at the current community site, codenameeagle.net.',
  },
  {
    id: 'black-screen-session',
    title: 'No more black screen from a stuck session',
    category: 'multiplayer',
    scope: 'mp',
    summary:
      'Starting a game clears out any leftover ce.exe and lobby.exe processes from a crashed or abandoned session, so hosting or joining no longer leaves you on a black screen.',
    body: 'Multiplayer runs a background lobby.exe helper that owns the network port. If a previous game crashed, or a server is still running on the same PC, that leftover session keeps hold of the port and leaves you on a black screen when you next host or join. The patch clears out any other running ce.exe and lobby.exe process just before starting a game, so the newest game always launches cleanly. It never touches the game you are launching, and single player is unaffected. Note that starting a game on a machine that is also running a dedicated server will stop that server, so do not host a game on your server box.',
  },
  {
    id: 'dedicated-idle-cpu',
    title: 'Dedicated servers idle quietly',
    category: 'multiplayer',
    scope: 'mp',
    summary:
      'An empty dedicated server no longer pegs a CPU core; the game loop is throttled while nobody is connected and switches back to full speed the instant a player joins.',
    body: 'A dedicated server used to peg a CPU core even with nobody connected, because the game loop runs as fast as it can. The patch throttles that loop while the server is empty, dropping idle CPU to a few percent, and switches back to full speed the instant a player joins. Normal players are unaffected.',
  },
  {
    id: 'dedicated-server-log',
    title: 'Dedicated servers keep a log of who joins and leaves',
    category: 'multiplayer',
    scope: 'mp',
    summary:
      'A dedicated server now records its events (server loaded, players joining, leaving and losing connection) to logs\\server.log, so you have a record even when the server console is not visible.',
    body: 'A dedicated server prints its events - server loaded, and players joining, leaving or losing connection - to its console window. That console is invisible when the server runs minimised, in the background or on a headless machine, and the lines could not be redirected to a file. The patch now also appends each of those lines to logs\\server.log, so there is always a record of what happened on the server, however it was started. The console itself is unchanged. This applies to dedicated servers only (started with +dedicated); a normal game you host from the menu does not produce these events and gets no server.log.',
  },

  {
    id: 'eight-trick',
    title: 'The "8 trick" no longer works',
    category: 'gameplay',
    scope: 'all',
    summary:
      'Fire delays are tracked per weapon, so re-selecting the weapon in your hands no longer cancels its cooldown to fire twice as fast, while switching to a different weapon still fires as soon as it is raised.',
    body: "Re-selecting the weapon already in your hands (or switching away and back, usually macroed to a mouse button) used to cancel the weapon's fire delay, letting the bazooka, sniper rifle, grenade and gas fire roughly twice as fast as intended. Fire delays are now tracked per weapon: the weapon that fired keeps its own cooldown no matter how you switch, so the trick gains nothing, while switching to a different weapon fires as soon as it is raised, exactly like vanilla. Quick combos such as placing an explosive and immediately switching to the detonator still work at full speed.",
  },
  {
    id: 'seb',
    title: 'The spontaneous explosion bug (SEB) is fixed',
    category: 'gameplay',
    scope: 'all',
    summary:
      'Firing at the ground can no longer randomly kill the shooter; a stray out-of-world projectile is silently removed instead of detonating its owner, on every map.',
    body: "Since v1.41, firing at the ground could randomly kill the shooter: a bullet that slipped through the terrain mesh and sank past the map's deepest point tripped an engine branch that dealt 3000 damage to its owner. The unofficial v1.42 only hid the bug on the stock multiplayer maps with invisible guard planes under the terrain, so it lived on in single player and on custom maps. The patch fixes the engine itself: a stray out-of-world projectile is now silently removed instead of detonating its shooter, everywhere, on every map. The old guard planes are removed from the shipped multiplayer levels, restoring the stock level geometry.",
  },
  {
    id: 'turret-balance',
    title: 'Single-player turret balance restored',
    category: 'gameplay',
    scope: 'full',
    summary:
      'The single-player base turrets are back to their v1.36 strength, undoing a v1.41 multiplayer balance change that leaked into the campaign and made it brutally hard. Multiplayer balance is untouched.',
    body: 'In v1.41 the base turrets (pillbox, bunker and armored-car cannons) fired a far stronger gun in single player than in v1.36, triple the damage and projectile speed. That was really a multiplayer balance change that leaked into single player and made the campaign brutally hard. The patch restores the single-player turret to its v1.36 strength. Multiplayer is untouched (it uses a separate weapon table), so versus and co-op balance for the armored car, torpedo boats and helicopter stays exactly as in v1.41.',
  },

  {
    id: 'tidy-folder',
    title: 'A tidy game folder',
    category: 'stability',
    scope: 'all',
    summary:
      'Logs go in logs\\, screenshots in screenshots\\ and savegames in saves\\ (existing saves are moved there for you), the stray player<N>.txt dumps are gone, and the game starts correctly regardless of working directory.',
    body: 'Log files now go in a logs\\ folder, screenshots in a screenshots\\ folder, and savegames (sg0.dat and friends) in a saves\\ folder, instead of being scattered in the game directory or written to your C:\\ drive, which modern Windows can block. Any existing savegames are moved into saves\\ for you, so nothing is lost. The game also stops leaving stray player1.txt, player2.txt diagnostic files behind, and starts correctly from shortcuts and links regardless of working directory.',
  },
  {
    id: 'sharper-icon',
    title: 'A sharper icon',
    category: 'stability',
    scope: 'all',
    summary:
      'The game icon is now embedded directly in ce.exe, so Windows shows it in Explorer, the taskbar and shortcuts. It uses a high-resolution image that stays crisp on modern displays and still renders correctly on older systems such as Windows XP. A second, classic icon is embedded as an alternate you can pick from a shortcut.',
    body: "The game icon is now embedded directly in ce.exe instead of shipping as a separate ce.ico file, so Windows shows it everywhere the executable appears: Explorer, the taskbar, alt-tab and shortcuts, not only where a shortcut pointed at an icon file. The embedded icon is a hybrid: classic bitmap frames for the small sizes, which older systems such as Windows XP can display, plus a high-resolution 256x256 image that stays crisp on modern high-DPI displays. A second, classic icon is embedded alongside it as a selectable alternate: in a shortcut's Change Icon dialog you can point it at ce.exe and choose that one instead. Stock ce.exe carried no icon at all and showed the generic Windows executable icon.",
  },
  {
    id: 'sp-crash-turret',
    title: 'Single-player crash near enemy bases fixed',
    category: 'stability',
    scope: 'full',
    summary:
      'The long-standing "floating point error" crash that could hit when you approach an enemy base is fixed (a bug in the mounted-turret code introduced in v1.41).',
    body: 'The long-standing "floating point error" that could hit when you approach an enemy base, a bug in the mounted-turret code introduced in v1.41, is fixed, so the base guns behave normally.',
  },
  {
    id: 'sp-crash-save',
    title: 'Save-game crash on modern graphics cards fixed',
    category: 'stability',
    scope: 'full',
    summary:
      'Saving the game no longer crashes on modern graphics cards. With the bundled dgVoodoo the save-slot thumbnail is captured as normal; the change is a safe guard that only steps in to prevent the crash if the capture ever fails.',
    body: 'Saving builds a small thumbnail by grabbing the screen, and on modern graphics cards a step of that grab could fail and crash the game. With the bundled dgVoodoo the grab succeeds and the thumbnail appears as normal. The change is a safe guard: it leaves the capture itself untouched and only removes the crash on the failure path, so in the rare case the grab does fail the game saves without a thumbnail instead of crashing.',
  },
  {
    id: 'fortress-terrain',
    title: 'Fortress terrain mesh repaired',
    category: 'stability',
    scope: 'all',
    summary:
      'The Fortress terrain shipped with defects that made the level exit with a fatal error ("two land faces or two sea faces") whenever its wcache.bin cache file was missing. The terrain is repaired, so the game can now rebuild the cache itself.',
    body: 'Fortress shipped with 19 defects in its terrain mesh: seven spots where two vertices sit less than one world unit apart, forming invisible slivers and near-vertical shoreline walls that the engine\'s water-pairing pass (InitWater) treats as fatal ("two land faces or two sea faces, nErrors=19") whenever it rebuilds the level\'s wcache.bin. Every stock install masked this by shipping the cache file, so deleting it made the level unloadable. The repair nudges 14 vertices by 0.05-0.13 world units, far too small to see; the mesh now validates cleanly, and the water/land pairing the game rebuilds from it is identical to the stock cache (2453 of 2453 pairs), so the level plays exactly as before. The cache file no longer ships: Fortress rebuilds it on first load, the same as every other level.',
  },

  {
    id: 'crosshair-scaling',
    title: 'The aiming crosshair scales with resolution',
    category: 'graphics',
    scope: 'all',
    summary:
      'The screen-center crosshair now scales with screen height, so it stays visible and the same apparent size at 1080p, 1200p and beyond instead of being a tiny fixed 8-pixel sprite.',
    body: 'The screen-center crosshair was a fixed 8-pixel sprite, which is tiny and hard to see at modern resolutions (it was sized for the roughly 480p displays of 1999). It now scales with the screen height, so it stays the same apparent size, and stays visible, at 1080p, 1200p and beyond.',
    images: {
      before: 'screenshots/crosshairs-stock.png',
      after: 'screenshots/crosshairs-upgraded.png',
      alt: 'aiming crosshair',
    },
  },
  {
    id: 'sniper-scope',
    title: 'The sniper scope is smoother',
    category: 'graphics',
    scope: 'all',
    summary:
      'The sniper scope overlay is a 32-bit texture with real antialiased alpha, giving a smooth lens edge and clean reticle lines instead of hard staircase pixels when upscaled.',
    body: "The scope overlay was a 24-bit texture whose transparency came from the engine's binary black color-key, so the lens edge and the reticle showed hard staircase pixels when upscaled to modern resolutions. It is replaced with a 32-bit version with real antialiased alpha: a smooth lens edge and clean reticle lines that read thinner at the center.",
    images: {
      before: 'screenshots/sniper-stock.png',
      after: 'screenshots/sniper-upgraded.png',
      alt: 'sniper scope',
    },
  },
  {
    id: 'dgvoodoo',
    title: 'dgVoodoo graphics wrapper included',
    category: 'graphics',
    scope: 'all',
    summary:
      'The dgVoodoo graphics wrapper is bundled to fix rendering issues on modern versions of Windows and to make options like anti-aliasing easy to enable. Run dgVoodooCpl.exe to configure it.',
    body: 'The patch ships the dgVoodoo graphics wrapper, which fixes a number of rendering issues on modern versions of Windows and makes options like anti-aliasing easy to enable. Run dgVoodooCpl.exe in the game folder to configure the settings. The installers include it by default and let you opt out; the demo zip always includes it. If you keep it, your own tuned dgVoodoo.conf is left in place on reinstall.',
  },

  {
    id: 'demo-menu-graphics',
    title: 'Multiplayer-demo menu options show their selected state',
    category: 'graphics',
    scope: 'mp',
    summary:
      'On the old multiplayer demo, several menu options never showed a checkmark when selected, so you could not tell they were active. The demo was missing the graphics those states use; the patch adds them back from the full game, so the affected options display correctly again. The full game was never affected.',
    body: 'The old multiplayer-demo repack shipped a trimmed set of menu graphics that left out several of the images the menu draws for a selected option, so those options showed nothing when chosen and there was no way to tell they were active. The patch restores the missing images from the full game, fixing the selected-state graphic for: 16 channel audio, the 3Dfx display driver, the None and Medium GraphicFX settings, Invert Mouse, and the red team on the Join Game screen. This only affects installs of the old multiplayer demo; the full game always shipped the complete set.',
  },

  {
    id: 'gas-mask',
    title: 'The gas mask in "Demolition Man" shows in your inventory',
    category: 'singleplayer',
    scope: 'full',
    summary:
      'In the "Demolition Man" mission the gas mask the village elder hands you now appears as an inventory item with its own icon, instead of being equipped invisibly. It is purely cosmetic.',
    body: 'In the "Demolition Man" mission the village elder hands you a gas mask, but stock only ever played the line and equipped it invisibly, so players kept looking for an item that never appeared. It now shows up as an inventory item with its own icon. This is purely cosmetic: the mask works exactly as before (you never had to select or use it), this just makes clear that you received it.',
  },
  {
    id: 'cutscenes',
    title: 'No more videos at every launch, and the campaign intro plays where it belongs',
    category: 'singleplayer',
    scope: 'full',
    summary:
      'With the CD cutscenes copied into the game folder, the game no longer replays three videos at every launch; the game story and campaign intro play at the mission 1 opening the first time you start the campaign in a session.',
    body: "With the CD cutscenes copied into the game folder, the game used to replay three videos at every single launch: the Refraction Games logo, the game story, and the campaign intro. The patch skips all three at startup and instead plays the game story and the campaign intro at the mission 1 opening, the first time you start the campaign in a session. The menu's view intro and view credits options still work. Because the opening now plays as the campaign starts, copy the CD's cutscenes into the game folder before playing the campaign: as with any Codename Eagle cutscene, if the file is missing the game asks for the CD when it would play.",
  },

  {
    id: 'cd-free-music',
    title: 'No CD needed, and no CD crashes',
    category: 'music',
    scope: 'full',
    summary:
      'The soundtrack can play from Ogg Vorbis files in a music\\ folder instead of the CD, with its own volume on the in-game music slider, and the crash on launch with a CD in the drive is gone.',
    body: 'The game can play its soundtrack from music files in a music\\ folder instead of the CD, with its own volume that the in-game music slider controls without affecting sound effects. It also fixes a crash on launch that could happen with a Codename Eagle CD in the drive (notably when running the multiplayer demo with a CD inserted), so the game no longer depends on or trips over the disc. The music playback (cemusic.dll) is part of the full game only; the crash fix always applies.',
  },
]
