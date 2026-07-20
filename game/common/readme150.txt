==================   CODENAME EAGLE Community Patch v1.50   ===================

Community sites:
	             https://codenameeagle.net
	             https://codenameeaglemultiplayer.com

================================================================================
Contents

1.Introduction
2.Installation
3.What's new in v1.50 ?
4.Compatibility
5.Credits
6.Acknowledgements

================================================================================
1.Introduction

This is a COMMUNITY-CREATED update for Codename Eagle. It is not an official
patch, and it is in no way endorsed or supported by DICE or Talonsoft. It was
made by fans to bring Codename Eagle back to life on modern systems and
today's internet - fixing the single-player crashes that broke it after 1.41
and teaching the in-game server browser to find internet games (it only ever
found LAN games before).

Version 1.50 builds on everything that came before it: the official updates
(v1.33, v1.36, v1.41), the unofficial v1.42 SEB fix, and Dafoosa's unofficial
v1.43 which has been the online standard for years (see readme143.txt).

The 1.50 patch was created by Espen Hovlandsdal, known as "Rexxie" back in the
old Codename Eagle days.

================================================================================
2.Installation

This package comes pre-patched - just play.

To upgrade your own full installation of Codename Eagle, download the 1.50
patch installer from
https://codenameeagle.net, run it, and point it at your Codename Eagle
folder. Any version from 1.0 to 1.43 is brought to v1.50 in a single step.
The installer verifies that the folder contains the game before touching
anything, and your saved games, hiscores and customized key bindings are
preserved.

Two optional steps need the Codename Eagle CD - both can also be done later
at any time:

* CD soundtrack: run ripmusic.exe in the game folder to convert the CD audio
  to music files the game plays without the CD.
* Cutscenes: copy the CD's cutscn folder into the game folder to watch the
  cutscenes without the CD. Do this before playing the campaign - the campaign
  opening plays as mission 1 starts, and like any Codename Eagle cutscene the
  game asks for the CD if the file is missing.

================================================================================
3.What's new in v1.50 ?

Multiplayer and internet play

* The in-game server browser finds internet games
Click Refresh list and the browser finds live internet servers and LAN games, with real server names, map names and player counts, so you can join straight from the menu.

* Hosted games are announced to the community master server
Hosting a game announces it to the community master server (ceservers.net), the replacement for the long-dead GameSpy master, so friends can find it.

* One-click join with cneagle:// links
The game registers itself as the handler for cneagle:// links, so a server link from the community site, Discord or a browser launches the game and drops you straight into that server.

* Dead links updated
The old URLs the game showed on version mismatches and similar now point at the current community site, codenameeagle.net.

* No more black screen from a stuck session
Starting a game clears out any leftover ce.exe and lobby.exe processes from a crashed or abandoned session, so hosting or joining no longer leaves you on a black screen.

* Dedicated servers idle quietly
An empty dedicated server no longer pegs a CPU core; the game loop is throttled while nobody is connected and switches back to full speed the instant a player joins.

* Dedicated servers keep a log of who joins and leaves
A dedicated server now records its events (server loaded, players joining, leaving and losing connection) to logs\server.log, so you have a record even when the server console is not visible.

Gameplay and balance

* The "8 trick" no longer works
Fire delays are tracked per weapon, so re-selecting the weapon in your hands no longer cancels its cooldown to fire twice as fast, while switching to a different weapon still fires as soon as it is raised.

* The spontaneous explosion bug (SEB) is fixed
Firing at the ground can no longer randomly kill the shooter; a stray out-of-world projectile is silently removed instead of detonating its owner, on every map.

* Single-player turret balance restored (full game)
The single-player base turrets are back to their v1.36 strength, undoing a v1.41 multiplayer balance change that leaked into the campaign and made it brutally hard. Multiplayer balance is untouched.

Stability and housekeeping

* A tidy game folder
Logs go in logs\, screenshots in screenshots\ and savegames in saves\ (existing saves are moved there for you), the stray player<N>.txt dumps are gone, and the game starts correctly regardless of working directory.

* A sharper icon
The game icon is now embedded directly in ce.exe, so Windows shows it in Explorer, the taskbar and shortcuts. It uses a high-resolution image that stays crisp on modern displays and still renders correctly on older systems such as Windows XP. A second, classic icon is embedded as an alternate you can pick from a shortcut.

* Single-player crash near enemy bases fixed (full game)
The long-standing "floating point error" crash that could hit when you approach an enemy base is fixed (a bug in the mounted-turret code introduced in v1.41).

* Save-game crash on modern graphics cards fixed (full game)
Saving the game no longer crashes on modern graphics cards. With the bundled dgVoodoo the save-slot thumbnail is captured as normal; the change is a safe guard that only steps in to prevent the crash if the capture ever fails.

* Fortress terrain mesh repaired
The Fortress terrain shipped with defects that made the level exit with a fatal error ("two land faces or two sea faces") whenever its wcache.bin cache file was missing. The terrain is repaired, so the game can now rebuild the cache itself.

Graphics and display

* The aiming crosshair scales with resolution
The screen-center crosshair now scales with screen height, so it stays visible and the same apparent size at 1080p, 1200p and beyond instead of being a tiny fixed 8-pixel sprite.

* The sniper scope is smoother
The sniper scope overlay is a 32-bit texture with real antialiased alpha, giving a smooth lens edge and clean reticle lines instead of hard staircase pixels when upscaled.

* dgVoodoo graphics wrapper included
The dgVoodoo graphics wrapper is bundled to fix rendering issues on modern versions of Windows and to make options like anti-aliasing easy to enable. Run dgVoodooCpl.exe to configure it.

* Multiplayer-demo menu options show their selected state
On the old multiplayer demo, several menu options never showed a checkmark when selected, so you could not tell they were active. The demo was missing the graphics those states use; the patch adds them back from the full game, so the affected options display correctly again. The full game was never affected.

Single player (full game)

* The gas mask in "Demolition Man" shows in your inventory
In the "Demolition Man" mission the gas mask the village elder hands you now appears as an inventory item with its own icon, instead of being equipped invisibly. It is purely cosmetic.

* No more videos at every launch, and the campaign intro plays where it belongs
With the CD cutscenes copied into the game folder, the game no longer replays three videos at every launch; the game story and campaign intro play at the mission 1 opening the first time you start the campaign in a session.

Music (full game)

* No CD needed, and no CD crashes
The soundtrack can play from Ogg Vorbis files in a music\ folder instead of the CD, with its own volume on the in-game music slider, and the crash on launch with a CD in the drive is gone.

================================================================================
4.Compatibility

- v1.50 changes the network protocol version: you can only play online with
  other v1.50 players. Send your friends to https://codenameeagle.net !

================================================================================
5.Credits

Espen Hovlandsdal ("Rexxie")    Community patch v1.50
Dafoosa                         Unofficial patch v1.43

================================================================================
6.Acknowledgements

None of this would exist without the original Codename Eagle team - thank you
for the game we still cannot put down, more than two decades later. In
particular the team behind the multiplayer updates:

Mats Dal          Lead programmer
Johan Persson     Programming
Carl Lundgren     Graphics, Design
Lars Gustavsson   Graphics

Thanks also to the whole old CE community - the mapmakers, the modders, the
clans and everyone who kept the servers warm all these years.

