# dgVoodoo

[dgVoodoo2](https://dege.freeweb.hu/dgVoodoo2/) by Dege, a graphics wrapper that translates the game's old DirectX calls to modern Direct3D. It fixes rendering problems on modern versions of Windows and makes options like anti-aliasing easy to enable. Players configure it by running `dgVoodooCpl.exe` in the game folder.

This is the 2.79.3 build. Newer releases have shown problems with Codename Eagle, so it is pinned to this version. To update it, download the version you want from Dege's site, replace the files here with the ones from the package root (`dgVoodoo.conf`, `dgVoodooCpl.exe`) and its `MS/x86/` folder (the DLLs), and commit.

The files are:

- `dgVoodooCpl.exe` (the control panel)
- `dgVoodoo.conf` (the configuration)
- `D3D8.dll`, `D3D9.dll`, `D3DImm.dll`, `DDraw.dll` (the wrapper DLLs)
- `dgVoodoo.txt` (the notice shipped to players, describing dgVoodoo and how to remove it)

## Redistribution

dgVoodoo is third-party software with its own terms, separate from this repository's MIT license. Dege's [redistribution notes](https://dege.freeweb.hu/dgVoodoo2/ReadmeGeneral/) allow shipping the individual dgVoodoo files together with a game or game mod, which is how the installers and the demo zip include them. Every deliverable that ships dgVoodoo also ships `dgVoodoo.txt` so players know what it is, where it comes from, and how to remove it.

## How it ships

The installers offer dgVoodoo as an optional component that is checked by default, and the demo zip always includes it (see [`installers/`](../installers/)). `dgVoodoo.conf` is written only if it is absent, so a player's tuned configuration survives a reinstall. The dedicated server image does not include dgVoodoo; it runs headless and forces Wine's builtin Direct3D instead.
