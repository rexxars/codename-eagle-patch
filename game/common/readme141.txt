=======================   CODENAME EAGLE Update v1.41   ========================

Official sites:
	             http://www.codenameeagle.com
	             http://www.talonsoft.com
	             http://www.dice.se
Community:
	             http://www.codenameeagle.net
	             http://pub30.ezboard.com/bceandbfcommunity
	             http://www.ce-bf.com

================================================================================
Contents

1.Introduction
2.Installation
3.What's cool in v1.41 ?
4.Gameplay features added in v1.33
5.Network changes
6.Dedicated Server
7.Server Console commands
8.Special Controls and Functionality
9.Setup Console Commands
10.Notes
11.Credits
12.Acknowledgements

================================================================================
1.Introduction

This is a multiplayer update for all versions of Codename Eagle. It contains all changes introduced in versions 1.33, 1.36 and the new version, v1.41.

Version 1.33 of Codename Eagle was hugely gameplay oriented and v1.36 focused on network issues. This version, v1.41, is an attempt to balance the gameplay towards a more intense action experience and introducing a couple of new really cool features.

During our work with this update, we have cooperated with a group of people from the CE community in documenting all there is to know about CE mapmaking. As a result of this we are hoping to see independent mod tools popping up close to the release date of this package.

================================================================================
2.Installation

Simply copy and paste the files into your Codename Eagle directory, replacing existing files. Backup your old game directory if you want to be able to revert to the earlier version.

================================================================================
3.What's cool in v1.41 ?

New Features

* 30 players in multiplayer!
This should allow for some truly crowded battlefields (this feature is only accessible through the dedicated server).

* Battle chopper
A brand new vehicle! This beauty takes a belly gunner and a driver who can fire death dealing rockets. A must see.

* Two people on the motorbike!
This feature adds a whole new dimension to the game. A driver and a blazing guns passenger.

* Maps
The existing maps have been modified to make good use of the above features. We also added one new huge CTF level, "Fortress".

* Teamchange command
"teamchange" console command allows you to change team up to three times per game. This is great for organizing clan games.

* Suicide command
"suicide" console command allows you to prompt a respawn at the cost of a four-death penalty. Great for when you're stuck swimming far out at sea.

Tweaking
- The zeppelin is faster but has a slower turning-speed.
- Planes have twice as much armor and their bullets do twice the damage.
- The AA-gun does more than twice the damage. Watch out!
- The fighter is a little bit faster. The bomber is a little bit slower.
- Hand grenades do real damage and has a larger damage radius.
- Gas is more lethal.

Bugfixes
- Miscellaneous optimization tuning.
- Planes can now fire and drop bombs simultaneously.
- The zeppelin no longer looses its collission.
- The tank ammo-gathering bug is no more.
- Further fixes to prevent name-cheating.
- Planes now have the correct team skins.

================================================================================
4.Gameplay features added in v1.33

* Destroyers
These huge warships will take one captain/front gunner, and one rear gunner... plus as many people as you can fit onto the deck.

* War Zeppelins
That's right... stuff your whole team in a zeppelin and roam the skies, raining bombs and bullets over enemy bases. Test teams really fell in love with this one.

* Parachutes
Finally you are able to parachute in over enemy territory and fight it out true commando style. The parachute (needless to say) often comes in handy when your plane goes down. Just remember to bring it with you.

* Bomber turrets
Bombers now take two players, one pilot (dropping bombs) and one rear gunner in a rotating turret, B17 style... You can also switch between positions if you pilot the plane by yourself.

* Boat turrets
The armoured boats are twice as fast and have machinegun turrets. Essentially armoured cars of the seas (and important anti-aircraft support for destroyers).

* Artillery
These static grenade launchers fire three screaming shells at once, painting black stripes in the sky. Keep out of range !

* Vehicle skins and various graphical fixes
New textures added for Russian fighter, Allied bomber and Allied armoured car. The game has also been overseen and polished here and there.

* Maps
The existing maps have been modified to make good use of the new features. We also added one new huge battle arena, Fever Valley.

================================================================================
5.Network changes

The result of the v1.36 test release was a noticeable improvement over v1.33 in terms of network stability. There are still problems if you have a high ping to the server or high packetloss, though players having these problems should no longer affect other players (for example: a player that logs on should not halt the game anymore).

See "Server Console Commands" for more info about network related console commands.

================================================================================
6.Dedicated Server

To set up a dedicated server, use the normal game.exe with this commandline :

game.exe +host +hostname "untitled" +dedicated +maxplayers 9 +map "fever valley" +game "teamplay" 

(Create a Codename Eagle shortcut to "game.exe", right click on the shortcut, then put the above line after the CE path in the "Target" field.)
  This should open a console window and start a dedicated teamplay server with the name "Untitled", for 8 players on the map "Fever Valley". Note: the dedicated server itself counts as one player, so if you want even teams, use an uneven '+maxplayers' amount.

================================================================================
7.Server Console commands

For the dedicated server, these commands should be present in the "default.cfg" file in CE's root directory. Ordinary servers can execute the commands from the console.

* Network related commands:

netstat <on/off>
Shows extended info on the connection.

kick <playername> 
(console) Expels a player from the server. The player will not be able to rejoin.

maxdup <1-4> 
This command tells the server how many old packets that should be "piggy-backed" to every
newly sent packet. Use a high value if joining players are experiencing much packetloss. 
Default value is 2.

maxresend <0-40>
This command tells the server the maximum number of packets that are sent to each player during packetloss.
Default value is 8.

latency <0-16>
This is sort of an experimental command... If you experience "jerky" lag, try a higher latency value to exchange the rough lag for a smoother, more "high-ping" lag. Does that make sense ? :)
Default value is 0.

* Game-related commands:

     scorelimit        ends game when reaching limit
     fraglimit         - " -
     timelimit         - " -
     map               changes map (map name)
     nextmap on        cycles through maps after game ends
     nextmap off       stops nextmap cycle

================================================================================
8.Special Controls and Functionality

* Save your console settings
The root directory holds a file called "default.cfg", in which you can type all your preferred console settings, like view distance, field of view and mouse sensitivity (this is already possible in the US retail version).

* Chat log 
All chat communication visible to your team is saved in the text file "chat.log" in the root directory. When the file reaches a size of 256 KB, it is cleared and starts over.

* Special in-game Controls

Mouse sensitivity       see "Setup Console Commands"
F8:                     global chat
F7:                     team chat
F12:                    bring up the console
Toggle weapon:          also 'switch turret' in bomber, helicopter and destroyer
PrintScreen:            grab screenshot
Jump:                   also 'drop bombs' for zeppelin
Pitch Down/Up:          also 'ascend/descend' for zeppelin
                        also 'advance/back' for helicopter
Forward/Back:           also 'accelerate/decelerate' for helicopter
Space:                  also 'fire rocket' for helicopter
F9:                     free camera
Ins/Home/Page/Del/End   move free camera around player

================================================================================
9.Setup Console Commands

All of these commands can be executed from either the console or 'default.cfg'.

mousesens         mouse sensitivity (1-20)
viewdist          view distance (100-2000)
fov               field of view (100-500)
fr on             frame rate display on
fr off            frame rate display off
connect           connect to tcp/ip host (ip)
latency           force delayed packets (0-8)
ping              displays ping
nettest on        info on the connection, packet loss in percent
nettest off       disables nettest

================================================================================
10.Notes

- Note that singleplayer is not guaranteed to function properly after installing this update.
- Unfortunately we have performance problems with ships on the 'Fortress' level and had to remove them. The level should still offer some exciting deathmatch moments though.

================================================================================

11.Credits

Mats Dal          Lead programmer
Johan Persson     Programming
Carl Lundgren     Graphics, Design
Lars Gustavsson   Graphics

================================================================================
12.Acknowledgements

Very special thanks to:

* Killaman and Sucka for the pizza, the movies and everything else!

* Mickle for excellent mapmaking tools.

* Digger, Zilla, Cliff, TheGrey, Toytown, Rexxie and Spy98 who put up with mapmaking torture for the greater good of the CE community :)

... and as always ... Tim Beggs

