; NSIS installer for the Codename Eagle 1.50 full-game patch.
; Compiles on macOS/Linux/Windows with makensis - use build.sh, which stages the
; payload and passes the defines below.
;
; Required defines:
;   PAYLOAD_BASE    staged game/common/ minus the conditionally-written configs
;   PAYLOAD_FULL    staged game/full/ minus levels.nfo and 24bits\texsec.dat
;                   (full-game installs only)
;   CONFIGS_DIR     the config files written conditionally, never via File /r:
;                   default.cfg, keyconf.dat (write-if-absent + stock-refresh)
;                   and menuinfo.dat (write-if-absent only)
;   DGVOODOO_DIR    the six dgVoodoo files, installed by the optional dgVoodoo
;                   component (checked by default) so the user can uncheck it;
;                   dgVoodoo.conf is write-if-absent, the rest are always copied
;   LEVELS_NFO_FULL levels.nfo for full-game installs (SP 1-12 + MP 128-134)
;   LEVELS_NFO_DEMO levels.nfo for old-MP-demo installs (MP 128-134 only)
;   MENUPICS_DEMO   menu/menupics.dat with the menu textures the demo repack
;                   trimmed added back (demo installs only; a full-game install
;                   already has the complete file, so it is never overwritten)
;   STOCK_DIR       factory-stock config snapshots for the refresh check
;   LOWERCASE_PS1   the lowercase.ps1 next to this script (absolute path -
;                   File paths resolve against makensis' cwd, not the script)
;   RIPMUSIC_EXE    the CD->ogg soundtrack ripper, always dropped in $INSTDIR
;   TEXTOOL_EXE     the texture-archive patcher; runs from $PLUGINSDIR at
;                   install time and never lands in the game folder
;   TEXSEC_STOCK    stock 1.43 24bits\texsec.dat, written only when the target
;                   install has none (stock 1.0 shipped without it)
;   TEX_INTERFC1    the 1.50 INTERFC1.tga, patched into 24bits\texsec.dat
;   TEX_SNIPEMOD    the 1.50 SNIPEMOD.tga, patched into 24bits\textures.dat
;   TEX_TARGET      the 1.50 Target.tga, patched into 24bits\textures.dat
;   OUTFILE         where to write the patch exe
;   VERSION         display version, e.g. "1.50.0" or "1.50.0-beta.1"
;   VIVERSION       strictly-numeric X.X.X.X form of VERSION for the exe's
;                   VIProductVersion field (build.sh derives it)
;
; Unlike the demo installer this one ships no game: it upgrades an EXISTING
; Codename Eagle install (any version 1.0-1.43, or the old multiplayer demo)
; to 1.50 in one hop by overwriting. It therefore writes NO uninstaller and NO
; Add/Remove Programs entry - it patches an install it does not own.
;
; What it does, in order (main section):
;   1. detects the variant (level1\ present -> full game, else MP demo),
;   2. lowercases ALL_CAPS file/dir names (cosmetic; via lowercase.ps1),
;   3. deletes caches/runtime junk, the pre-1.50 No Mans Land leftovers and
;      level248\ (Fever valley is level134 now) - never touching hiscores,
;      saves or screenshots,
;   4. writes the payload (common + full, or the demo levels.nfo + the fixed
;      demo menu/menupics.dat),
;   5. patches the 1.50 texture fixes into the player's own 24bits archives
;      with textool.exe (full game only; writes the stock texsec.dat first
;      when none is present - the binaries still ship pre-patched, the
;      texture archives are the one thing patched at install time),
;   6. writes configs only if absent or still factory-stock (customized
;      keybinds etc. are preserved; menuinfo.dat only if absent),
;   7. drops ripmusic.exe in the game folder,
;   8. adds the firewall rules and the machine-wide cneagle:// registration
;      (same blocks and rule names as the demo installer - same game).
; A dgVoodoo section (checked by default) installs the graphics wrapper; the
; user can uncheck it, and its dgVoodoo.conf is write-if-absent so a tuned
; config survives. Two more optional sections rip the CD soundtrack
; (ripmusic.exe) and copy the CD's cutscn\ folder; both can be done manually
; later (see readme150.txt).

!ifndef PAYLOAD_BASE
  !error "PAYLOAD_BASE not defined - build with build.sh"
!endif
!ifndef PAYLOAD_FULL
  !error "PAYLOAD_FULL not defined - build with build.sh"
!endif
!ifndef CONFIGS_DIR
  !error "CONFIGS_DIR not defined - build with build.sh"
!endif
!ifndef DGVOODOO_DIR
  !error "DGVOODOO_DIR not defined - build with build.sh"
!endif
!ifndef LEVELS_NFO_FULL
  !error "LEVELS_NFO_FULL not defined - build with build.sh"
!endif
!ifndef LEVELS_NFO_DEMO
  !error "LEVELS_NFO_DEMO not defined - build with build.sh"
!endif
!ifndef MENUPICS_DEMO
  !error "MENUPICS_DEMO not defined - build with build.sh"
!endif
!ifndef STOCK_DIR
  !error "STOCK_DIR not defined - build with build.sh"
!endif
!ifndef LOWERCASE_PS1
  !error "LOWERCASE_PS1 not defined - build with build.sh"
!endif
!ifndef RIPMUSIC_EXE
  !error "RIPMUSIC_EXE not defined - build with build.sh"
!endif
!ifndef TEXTOOL_EXE
  !error "TEXTOOL_EXE not defined - build with build.sh"
!endif
!ifndef TEXSEC_STOCK
  !error "TEXSEC_STOCK not defined - build with build.sh"
!endif
!ifndef TEX_INTERFC1
  !error "TEX_INTERFC1 not defined - build with build.sh"
!endif
!ifndef TEX_SNIPEMOD
  !error "TEX_SNIPEMOD not defined - build with build.sh"
!endif
!ifndef TEX_TARGET
  !error "TEX_TARGET not defined - build with build.sh"
!endif
!ifndef OUTFILE
  !error "OUTFILE not defined - build with build.sh"
!endif
!ifndef VERSION
  !error "VERSION not defined - build with build.sh"
!endif

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!define APPNAME "Codename Eagle 1.50 Patch"
!define PUBLISHER "Codename Eagle Nation"
!define ABOUTURL "https://codenameeagle.net/"
; The demo installer's Add/Remove key - read-only here, as a directory hint
; when the MP demo is installed. This patcher never writes it.
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodenameEagleMPDemo"
!define FWRULE_GAME "Codename Eagle (game)"
!define FWRULE_LOBBY "Codename Eagle (lobby)"
!define FWRULE_IPLIST "Codename Eagle (server browser)"

Name "${APPNAME}"
OutFile "${OUTFILE}"
BrandingText "${PUBLISHER}"
SetCompressor /SOLID lzma

; Elevation is needed for the firewall rules and the HKLM protocol keys.
RequestExecutionLevel admin

; The trailing backslash matters: without it, NSIS treats the last path
; component as a folder suffix and the Browse dialog appends "\Codename Eagle"
; to whatever folder the user picks. This patcher targets an EXISTING install
; whose folder can be named anything, so Browse must use the picked folder
; verbatim (the suffix made the gate below fail on e.g. "C:\Games\CE").
InstallDir "C:\Games\Codename Eagle\"
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallLocation"

!ifndef VIVERSION
  !error "VIVERSION not defined - build with build.sh"
!endif
VIProductVersion "${VIVERSION}"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${APPNAME} setup"
VIAddVersionKey "LegalCopyright" "${PUBLISHER}"

; The game's own icon on the patch exe. Sourced from the repo, not the payload:
; the game icon ships embedded in ce.exe, not as a loose .ico.
!define MUI_ICON "${WIZARD_ICON}"

Var FullGame     ; "1" when $INSTDIR is a full-game install, "0" for the MP demo
Var CutscnFound  ; "1" once a CD with cutscn\ has been found and copied

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "This wizard upgrades an existing Codename Eagle installation to version 1.50 - any version from 1.0 to 1.43, or the old multiplayer demo, in one step.$\r$\n$\r$\nIt does NOT contain the game itself. If Codename Eagle is not installed yet, install it first, then run this patch.$\r$\n$\r$\nSaved games, hiscores and customized key bindings are preserved.$\r$\n$\r$\n$_CLICK"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!define MUI_DIRECTORYPAGE_TEXT_TOP "Point the installer at your Codename Eagle folder - any version from 1.0 to 1.43, or the multiplayer demo. If the game is not installed yet, install it first: this patch does not contain the game.$\r$\n$\r$\nNext stays disabled until the selected folder contains a Codename Eagle installation."
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "Codename Eagle has been updated to version 1.50.$\r$\n$\r$\nSkipped the optional CD steps? Both can be done later: run ripmusic.exe in the game folder to rip the CD soundtrack, and copy the CD's cutscn folder into the game folder for CD-free cutscenes. See readme150.txt for details."
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Play Codename Eagle now"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchGame
!define MUI_FINISHPAGE_LINK "${ABOUTURL}"
!define MUI_FINISHPAGE_LINK_LOCATION "${ABOUTURL}"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_LANGUAGE "English"

Function LaunchGame
  ; Launch through explorer so the game runs as the logged-in user, not as the
  ; elevated installer. The patched ce.exe sets its own working directory, so
  ; the missing "Start in" doesn't matter.
  Exec '"$WINDIR\explorer.exe" "$INSTDIR\ce.exe"'
FunctionEnd

Function .onInit
  ; Stage the helpers the main section compares/executes against: the
  ; factory-stock config snapshots and the lowercasing script.
  InitPluginsDir
  SetOutPath "$PLUGINSDIR\stock"
  File /r "${STOCK_DIR}/*"
  SetOutPath "$PLUGINSDIR"
  File "/oname=lowercase.ps1" "${LOWERCASE_PS1}"
FunctionEnd

; The gate: a dir only qualifies as a Codename Eagle install with both these
; files present - we never ship the game itself. ON_FAIL is the instruction to
; run on a miss; one macro for both call sites so the fingerprint can't drift.
!macro RequireGameDir ON_FAIL
  ${IfNot} ${FileExists} "$INSTDIR\Game.exe"
    ${ON_FAIL}
  ${EndIf}
  ${IfNot} ${FileExists} "$INSTDIR\dialogue.dat"
    ${ON_FAIL}
  ${EndIf}
!macroend

Function .onVerifyInstDir
  ; Runs on every keystroke in the directory box, so keep it to the gate only.
  ; A plain Abort greys out the Next button.
  !insertmacro RequireGameDir "Abort"
FunctionEnd

; Delete the cache files the game regenerates (they'd be stale after patching).
!macro CleanLevelDir DIR
  Delete "$INSTDIR\${DIR}\*cache.bin"
  Delete "$INSTDIR\${DIR}\*cache.dat"
!macroend

; Write-if-absent + stock-refresh for one config file. An existing file that is
; byte-identical to a known factory-stock version (fc /b exit code 0) gets the
; 1.50 version; anything else the user customized is left alone. STOCK2 may be
; "" when there is only one stock variant to check.
!macro RefreshConfig NAME STOCK1 STOCK2
  ${IfNot} ${FileExists} "$INSTDIR\${NAME}"
    DetailPrint "Writing ${NAME} (none present)"
    File "/oname=${NAME}" "${CONFIGS_DIR}/${NAME}"
  ${Else}
    StrCpy $R0 "0" ; becomes "1" when the existing file is a known stock copy
    nsExec::ExecToLog 'cmd /c fc /b "$INSTDIR\${NAME}" "$PLUGINSDIR\stock\${STOCK1}" >nul'
    Pop $0
    ${If} $0 == "0"
      StrCpy $R0 "1"
    ${EndIf}
    !if "${STOCK2}" != ""
      ${If} $R0 == "0"
        nsExec::ExecToLog 'cmd /c fc /b "$INSTDIR\${NAME}" "$PLUGINSDIR\stock\${STOCK2}" >nul'
        Pop $0
        ${If} $0 == "0"
          StrCpy $R0 "1"
        ${EndIf}
      ${EndIf}
    !endif
    ${If} $R0 == "1"
      DetailPrint "Replacing factory-stock ${NAME} with the 1.50 version"
      File "/oname=${NAME}" "${CONFIGS_DIR}/${NAME}"
    ${Else}
      DetailPrint "Keeping your customized ${NAME}"
    ${EndIf}
  ${EndIf}
!macroend

Section "Codename Eagle 1.50 patch (required)" SecPatch
  SectionIn RO

  ; Silent installs (/S /D=dir) skip the directory page, so .onVerifyInstDir
  ; never ran - re-check the gate before touching anything, because this
  ; section renames and deletes files in $INSTDIR.
  !insertmacro RequireGameDir 'Abort "$INSTDIR does not look like a Codename Eagle folder (Game.exe/dialogue.dat missing) - install the game first."'

  ; 1) Detect the variant: SP levels on disk means a full-game install.
  ${If} ${FileExists} "$INSTDIR\level1\*.*"
    StrCpy $FullGame "1"
    DetailPrint "Detected a full-game install."
  ${Else}
    StrCpy $FullGame "0"
    DetailPrint "Detected a multiplayer-demo install."
  ${EndIf}

  ; 2) Lowercase pass BEFORE writing the payload, so LEVEL1\, SOUNDS\ etc. get
  ;    normalized and the files we then write keep their payload casing.
  ;    Non-fatal - purely cosmetic.
  DetailPrint "Normalizing file name casing..."
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\lowercase.ps1" "$INSTDIR"'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Warning: could not normalize file name casing (cosmetic only, continuing)."
  ${EndIf}

  ; 3) Clean up caches and runtime junk. NEVER touch user data: hiscores.dat,
  ;    sg0.dat/saves, screenshots, custom configs.
  ;    level133\wcache.bin goes too: 1.50 repairs the Fortress terrain (stock
  ;    terrain tripped the fatal "two land faces or two sea faces" error in
  ;    InitWater while rebuilding it), so the game recreates it on first load
  ;    like every other cache. See game/README.md.
  DetailPrint "Removing stale caches and runtime junk..."
  !insertmacro CleanLevelDir "level1"
  !insertmacro CleanLevelDir "level2"
  !insertmacro CleanLevelDir "level3"
  !insertmacro CleanLevelDir "level4"
  !insertmacro CleanLevelDir "level5"
  !insertmacro CleanLevelDir "level6"
  !insertmacro CleanLevelDir "level7"
  !insertmacro CleanLevelDir "level8"
  !insertmacro CleanLevelDir "level9"
  !insertmacro CleanLevelDir "level10"
  !insertmacro CleanLevelDir "level11"
  !insertmacro CleanLevelDir "level12"
  !insertmacro CleanLevelDir "level128"
  !insertmacro CleanLevelDir "level129"
  !insertmacro CleanLevelDir "level130"
  !insertmacro CleanLevelDir "level131"
  !insertmacro CleanLevelDir "level132"
  !insertmacro CleanLevelDir "level133"
  !insertmacro CleanLevelDir "level134"
  Delete "$INSTDIR\diacache.dat"
  Delete "$INSTDIR\lobby.log"
  Delete "$INSTDIR\player*.txt"
  Delete "$INSTDIR\*.bak"
  ; A textool run killed mid-write can leave texsec.dat.tmp/textures.dat.tmp
  ; behind (textool cleans up on its own error paths; this heals hard kills on
  ; the next run).
  Delete "$INSTDIR\24bits\*.tmp"
  ; 1.50 removed these from No Mans Land - a 1.43 install still has them, and
  ; leaving them would contradict the reworked level data.
  Delete "$INSTDIR\level128\cactus1.scr"
  Delete "$INSTDIR\level128\cactuss.scr"
  Delete "$INSTDIR\level128\switch1.scr"
  ; Fever valley moved 248 -> 134 (matches the demo + the 1.50 level table).
  RMDir /r "$INSTDIR\level248"

  ; 4) The payload. Full-game installs also get the SP fixes and their own
  ;    levels.nfo; demo installs keep an MP-only levels.nfo so no phantom SP
  ;    level entries appear.
  DetailPrint "Writing the 1.50 files..."
  SetOutPath "$INSTDIR"
  File /r "${PAYLOAD_BASE}/*"
  ${If} $FullGame == "1"
    File /r "${PAYLOAD_FULL}/*"
    File "/oname=levels.nfo" "${LEVELS_NFO_FULL}"
  ${Else}
    File "/oname=levels.nfo" "${LEVELS_NFO_DEMO}"
    ; The demo repack shipped a trimmed menu/menupics.dat missing six menu
    ; textures the menu code still references (blank slots in-game). Ship the
    ; fixed archive - demo only: a full-game install has the complete original
    ; menupics.dat (~60 MB) and must keep it. SetOutPath creates menu\ if the
    ; casing pass or an odd install left it absent, then restores $INSTDIR.
    SetOutPath "$INSTDIR\menu"
    File "${MENUPICS_DEMO}"
    SetOutPath "$INSTDIR"
  ${EndIf}

  ; 5) Texture fixes, full game only: patch the changed textures into the
  ;    player's OWN 24bits archives with textool.exe instead of shipping the
  ;    whole multi-MB archives, so any other textures the player modded
  ;    survive. Non-fatal: a failed patch just leaves the stock textures in
  ;    place. (Demo installs need none of this - the demo payload ships its
  ;    own tiny texsec.dat.)
  ${If} $FullGame == "1"
    ; textool runs from $PLUGINSDIR and is never installed into the game
    ; folder (an install-time tool, not a game file - same treatment as
    ; menuinfo-nick.exe in the demo installer).
    File "/oname=$PLUGINSDIR\textool.exe" "${TEXTOOL_EXE}"
    File "/oname=$PLUGINSDIR\INTERFC1.tga" "${TEX_INTERFC1}"
    File "/oname=$PLUGINSDIR\SNIPEMOD.tga" "${TEX_SNIPEMOD}"
    File "/oname=$PLUGINSDIR\Target.tga" "${TEX_TARGET}"
    ; A stock 1.0 install has no 24bits\texsec.dat at all, and it can't be
    ; skipped (most of its textures exist in no other archive), so write the
    ; stock 1.43 one when absent. 1.41/1.43 installs keep their own copy -
    ; textool patches it in place, so texture mods survive.
    ${IfNot} ${FileExists} "$INSTDIR\24bits\texsec.dat"
      DetailPrint "Writing 24bits\texsec.dat (none present)"
      SetOutPath "$INSTDIR\24bits"
      File "/oname=texsec.dat" "${TEXSEC_STOCK}"
      SetOutPath "$INSTDIR"
    ${EndIf}
    ; Files copied off a CD often carry the read-only attribute, which would
    ; make textool's atomic rename fail - clear it before patching.
    SetFileAttributes "$INSTDIR\24bits\texsec.dat" NORMAL
    DetailPrint "Patching the 1.50 texture fixes into the 24bits archives..."
    nsExec::ExecToLog '"$PLUGINSDIR\textool.exe" set "$INSTDIR\24bits\texsec.dat" "$PLUGINSDIR\INTERFC1.tga"'
    Pop $0
    ${If} $0 != 0
      DetailPrint "Warning: could not patch 24bits\texsec.dat (code $0) - the current textures are kept."
    ${EndIf}
    ${If} ${FileExists} "$INSTDIR\24bits\textures.dat"
      SetFileAttributes "$INSTDIR\24bits\textures.dat" NORMAL
      nsExec::ExecToLog '"$PLUGINSDIR\textool.exe" set "$INSTDIR\24bits\textures.dat" "$PLUGINSDIR\SNIPEMOD.tga" "$PLUGINSDIR\Target.tga"'
      Pop $0
      ${If} $0 != 0
        DetailPrint "Warning: could not patch 24bits\textures.dat (code $0) - the current textures are kept."
      ${EndIf}
    ${Else}
      DetailPrint "24bits\textures.dat not found - skipping its texture fixes"
    ${EndIf}
  ${EndIf}

  ; 6) Configs: write-if-absent, refresh-if-factory-stock (so ancient installs
  ;    get the current binds while customized configs survive).
  !insertmacro RefreshConfig "keyconf.dat" "keyconf-1.0.dat" "keyconf-1.36.dat"
  !insertmacro RefreshConfig "default.cfg" "default-1.33.cfg" ""
  ; menuinfo.dat is the saved profile - it holds single-player progress
  ; (LevelsDone) as well as options, so refreshing a factory-stock one would
  ; reset a returning player's campaign progress. Write-if-absent only: a fresh
  ; install gets our default profile (name "CEDemo", 1024x768, nothing completed);
  ; any existing menuinfo (stock or customized) is the user's and is left alone.
  ${IfNot} ${FileExists} "$INSTDIR\menuinfo.dat"
    DetailPrint "Writing menuinfo.dat (none present)"
    File "/oname=menuinfo.dat" "${CONFIGS_DIR}/menuinfo.dat"
  ${Else}
    DetailPrint "Keeping your existing menuinfo.dat"
  ${EndIf}
  ; dgVoodoo is handled by its own optional section below, so it is not written
  ; here.

  ; 7) The soundtrack ripper always lands in the game dir so the optional rip
  ;    can be run later by hand (readme150.txt documents it).
  File "/oname=ripmusic.exe" "${RIPMUSIC_EXE}"

  ; Pre-authorize the networked binaries in Windows Firewall. Without this,
  ; hosting the first game pops the firewall consent dialog, which minimizes the
  ; fullscreen game mid-handshake and strands it, and iplist.exe's LAN discovery
  ; (an inbound UDP :210 listen) pops the same dialog on the first server-list
  ; refresh. Delete-then-add keeps repeated runs (or a changed install dir)
  ; from piling up duplicate rules.
  DetailPrint "Adding Windows Firewall rules..."
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${FWRULE_GAME}"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${FWRULE_LOBBY}"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${FWRULE_IPLIST}"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${FWRULE_GAME}" dir=in action=allow program="$INSTDIR\ce.exe" enable=yes profile=any'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${FWRULE_LOBBY}" dir=in action=allow program="$INSTDIR\lobby.exe" enable=yes profile=any'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${FWRULE_IPLIST}" dir=in action=allow program="$INSTDIR\iplist.exe" enable=yes profile=any'
  Pop $0

  ; cneagle:// one-click join, machine-wide so links work before first launch.
  ; Same key shape ce.exe writes to HKCU on startup; a per-user HKCU key takes
  ; precedence over this one, so the two coexist.
  DetailPrint "Registering the cneagle:// protocol..."
  WriteRegStr HKLM "Software\Classes\cneagle" "" "URL:CE Protocol"
  WriteRegStr HKLM "Software\Classes\cneagle" "URL Protocol" ""
  WriteRegStr HKLM "Software\Classes\cneagle\DefaultIcon" "" "$INSTDIR\ce.exe,0"
  WriteRegStr HKLM "Software\Classes\cneagle\shell\open\command" "" '"$INSTDIR\ce.exe" %1'
SectionEnd

Section "dgVoodoo graphics wrapper (recommended)" SecDgVoodoo
  SetOutPath "$INSTDIR"
  File "${DGVOODOO_DIR}/dgVoodoo.txt"
  File "${DGVOODOO_DIR}/dgVoodooCpl.exe"
  File "${DGVOODOO_DIR}/D3D8.dll"
  File "${DGVOODOO_DIR}/D3D9.dll"
  File "${DGVOODOO_DIR}/D3DImm.dll"
  File "${DGVOODOO_DIR}/DDraw.dll"
  ; dgVoodoo.conf is user-tunable (resolution, anti-aliasing and so on), so
  ; write-if-absent only: an existing one is the user's and is never overwritten.
  ${IfNot} ${FileExists} "$INSTDIR\dgVoodoo.conf"
    DetailPrint "Writing dgVoodoo.conf (none present)"
    File "/oname=dgVoodoo.conf" "${DGVOODOO_DIR}/dgVoodoo.conf"
  ${Else}
    DetailPrint "Keeping your existing dgVoodoo.conf"
  ${EndIf}
SectionEnd

Section /o "Rip CD soundtrack to music\ (needs the CE CD)" SecMusic
  DetailPrint "Ripping the CD soundtrack to $INSTDIR\music ..."
  nsExec::ExecToLog '"$INSTDIR\ripmusic.exe" "$INSTDIR"'
  Pop $0
  ${If} $0 != 0
    DetailPrint "ripmusic failed (code $0) - you can run ripmusic.exe in the game folder later."
  ${EndIf}
SectionEnd

; ${GetDrives} callback: $9 = drive root ("D:\"). Copy the first CD that has
; the cutscenes, then stop scanning.
Function CopyCutscnCallback
  StrCpy $0 "" ; anything but "StopGetDrives" keeps the drive scan going
  ${If} ${FileExists} "$9cutscn\*.smk"
    DetailPrint "Copying cutscenes from $9cutscn ..."
    CreateDirectory "$INSTDIR\cutscn"
    ; Non-silent CopyFiles shows a progress window - it's ~215 MB off a CD.
    CopyFiles "$9cutscn\*.*" "$INSTDIR\cutscn"
    StrCpy $CutscnFound "1"
    StrCpy $0 "StopGetDrives"
  ${EndIf}
  Push $0
FunctionEnd

Section /o "Copy cutscenes from CD (~215 MB)" SecCutscn
  AddSize 220160
  StrCpy $CutscnFound "0"
  ${GetDrives} "CDROM" "CopyCutscnCallback"
  ${If} $CutscnFound == "0"
    DetailPrint "No CE CD with a cutscn folder found - see readme150.txt for copying it later."
  ${EndIf}
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecPatch} "Updates the installed game to version 1.50. Saved games, hiscores and customized key bindings are preserved."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDgVoodoo} "Fixes rendering problems on modern Windows and makes options like anti-aliasing easy to turn on. Leave it checked unless you have a specific reason not to. An existing dgVoodoo.conf is kept as is."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMusic} "Rips the CD soundtrack to Ogg Vorbis files in the music folder, so the game plays music without the CD. Needs the Codename Eagle CD. Can be done later by running ripmusic.exe in the game folder."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCutscn} "Copies the cutscene videos from the CD into the game folder, so they play without the CD. Can be done later by copying the CD's cutscn folder yourself."
!insertmacro MUI_FUNCTION_DESCRIPTION_END
