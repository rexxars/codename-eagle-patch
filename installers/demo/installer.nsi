; NSIS installer for the Codename Eagle multiplayer demo (1.50-MPDEMO).
; Compiles on macOS/Linux/Windows with makensis - use build.sh, which stages the
; payload and passes the defines below.
;
; Required defines:
;   PAYLOAD_DIR  staged merge of the repo's game/common/ + game/demo/ folders
;                (ships pre-patched), minus the six dgVoodoo files
;   DGVOODOO_DIR the six dgVoodoo files, installed by the optional dgVoodoo
;                component so unchecking it leaves them out
;   OUTFILE      where to write the setup exe
;   VERSION      display version for Add/Remove Programs, e.g. "1.50.0" or
;                "1.50.0-beta.1"
;   VIVERSION    strictly-numeric X.X.X.X form of VERSION for the exe's
;                VIProductVersion field (build.sh derives it)
;
; Unlike the ce-patch installer this one runs no patch step: game/common/ and
; game/demo/ in this repo already contain the patched binaries (master-server
; redirect, cneagle:// self-register, menudll, iplist, dgVoodoo, ...).
;
; What the installer does beyond copying files:
;   - adds Windows Firewall allow-rules for ce.exe, lobby.exe and iplist.exe,
;     so the firewall consent popup never minimizes the fullscreen game
;     mid-hosting (stock CE freezes on a black taskbar preview when that
;     happens) and never appears on the first server-list refresh,
;   - registers the cneagle:// URL protocol machine-wide (HKLM), so one-click
;     join links work before the game has ever been launched; ce.exe still
;     re-registers per-user (HKCU) on every launch,
;   - Start Menu + optional Desktop shortcuts using ce.exe's icon, Add/Remove Programs
;     entry, uninstaller that also removes the firewall rules and both protocol
;     keys.

!ifndef PAYLOAD_DIR
  !error "PAYLOAD_DIR not defined - build with build.sh"
!endif
!ifndef DGVOODOO_DIR
  !error "DGVOODOO_DIR not defined - build with build.sh"
!endif
!ifndef OUTFILE
  !error "OUTFILE not defined - build with build.sh"
!endif
!ifndef VERSION
  !error "VERSION not defined - build with build.sh"
!endif
!ifndef MENUINFO_NICK_EXE
  !error "MENUINFO_NICK_EXE not defined - build with build.sh"
!endif

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WinMessages.nsh"
!include "WordFunc.nsh"
!include "nsDialogs.nsh"

!define APPNAME "Codename Eagle Multiplayer Demo"
!define PUBLISHER "Codename Eagle Nation"
!define ABOUTURL "https://codenameeagle.net/"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodenameEagleMPDemo"
; Same rule names as installers/patch/installer.nsi (same game); this
; installer's uninstaller is what cleans them up.
!define FWRULE_GAME "Codename Eagle (game)"
!define FWRULE_LOBBY "Codename Eagle (lobby)"
!define FWRULE_IPLIST "Codename Eagle (server browser)"

Name "${APPNAME}"
OutFile "${OUTFILE}"
BrandingText "${PUBLISHER}"
SetCompressor /SOLID lzma

; Elevation is needed for the firewall rules and the HKLM keys.
RequestExecutionLevel admin

; NOT Program Files: the game writes saves/logs/screenshots into its own folder,
; which a non-elevated player can't do under Program Files.
InstallDir "C:\Games\Codename Eagle"
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallLocation"

!ifndef VIVERSION
  !error "VIVERSION not defined - build with build.sh"
!endif
VIProductVersion "${VIVERSION}"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${APPNAME} setup"
VIAddVersionKey "LegalCopyright" "${PUBLISHER}"

; The game's own icon on the setup/uninstall exes. Sourced from the repo, not
; the payload: the game icon ships embedded in ce.exe, not as a loose .ico.
!define MUI_ICON "${WIZARD_ICON}"
!define MUI_UNICON "${WIZARD_ICON}"

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
; Custom page: pick the multiplayer name written into menuinfo.dat post-copy.
Page custom NickPageCreate NickPageLeave
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Play Codename Eagle now"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchGame
!define MUI_FINISHPAGE_LINK "${ABOUTURL}"
!define MUI_FINISHPAGE_LINK_LOCATION "${ABOUTURL}"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function LaunchGame
  ; Launch through explorer so the game runs as the logged-in user, not as the
  ; elevated installer. The patched ce.exe sets its own working directory, so
  ; the missing "Start in" doesn't matter.
  Exec '"$WINDIR\explorer.exe" "$INSTDIR\ce.exe"'
FunctionEnd

; The multiplayer name the player types; menuinfo-nick.exe writes it into
; menuinfo.dat after the payload is copied. Defaults to the shipped "CEDemo".
Var Nickname
Var NickTextBox

Function NickPageCreate
  !insertmacro MUI_HEADER_TEXT "Multiplayer name" "Choose the name other players see when you join or host games."
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  ${NSD_CreateLabel} 0 0 100% 24u "Up to 10 characters (letters and numbers). Leave it as CEDemo if you're not sure - you can change it in-game later."
  Pop $0

  ${NSD_CreateText} 0 30u 100% 12u "$Nickname"
  Pop $NickTextBox
  ; Cap typed input at the 10 chars the game broadcasts into multiplayer.
  SendMessage $NickTextBox ${EM_SETLIMITTEXT} 10 0

  nsDialogs::Show
FunctionEnd

Function NickPageLeave
  ${NSD_GetText} $NickTextBox $Nickname
  ; Strip double-quotes so the name can't break the quoted command-line argument
  ; passed to menuinfo-nick.exe; the exe does the rest of the normalization
  ; (non-ASCII, length, empty -> CEDemo).
  ${WordReplace} "$Nickname" '"' "" "+" $Nickname
FunctionEnd

Function .onInit
  SetShellVarContext all
  StrCpy $Nickname "CEDemo"
FunctionEnd

Function un.onInit
  SetShellVarContext all
FunctionEnd

Section "Game files (required)" SecGame
  SectionIn RO
  SetOutPath "$INSTDIR"
  File /r "${PAYLOAD_DIR}/*"

  ; Write the chosen multiplayer name into the just-copied menuinfo.dat. The
  ; helper runs from $PLUGINSDIR and is never installed into the game folder
  ; (it's an install-time tool, not a game file). Non-fatal: on any failure the
  ; shipped default name (CEDemo) simply stays, rather than aborting the install.
  InitPluginsDir
  File "/oname=$PLUGINSDIR\menuinfo-nick.exe" "${MENUINFO_NICK_EXE}"
  DetailPrint "Setting multiplayer name to $Nickname..."
  nsExec::ExecToLog '"$PLUGINSDIR\menuinfo-nick.exe" "$INSTDIR\menuinfo.dat" "$Nickname"'
  Pop $0
  ${If} $0 <> 0
    DetailPrint "Could not set the multiplayer name (code $0) - keeping the default."
  ${EndIf}

  ; Pre-authorize the networked binaries in Windows Firewall. Without this,
  ; hosting the first game pops the firewall consent dialog, which minimizes the
  ; fullscreen game mid-handshake and strands it, and iplist.exe's LAN discovery
  ; (an inbound UDP :210 listen) pops the same dialog on the first server-list
  ; refresh. Delete-then-add keeps reinstalls (or a changed install dir) from
  ; piling up duplicate rules.
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

  ; Start Menu (SetOutPath above doubles as the shortcuts' "Start in" folder).
  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortCut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\ce.exe" "" "$INSTDIR\ce.exe" 0
  CreateShortCut "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk" "$INSTDIR\uninstall.exe"

  ; Add/Remove Programs
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\ce.exe,0"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKLM "${UNINST_KEY}" "URLInfoAbout" "${ABOUTURL}"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${UNINST_KEY}" "EstimatedSize" "$0"

  WriteUninstaller "$INSTDIR\uninstall.exe"
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
  ; write it only when it is absent - a config the player already tuned survives
  ; a reinstall untouched.
  ${IfNot} ${FileExists} "$INSTDIR\dgVoodoo.conf"
    DetailPrint "Writing dgVoodoo.conf (none present)"
    File "${DGVOODOO_DIR}/dgVoodoo.conf"
  ${Else}
    DetailPrint "Keeping your existing dgVoodoo.conf"
  ${EndIf}
SectionEnd

Section "Desktop shortcut" SecDesktop
  SetOutPath "$INSTDIR"
  CreateShortCut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\ce.exe" "" "$INSTDIR\ce.exe" 0
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecGame} "The game itself, patched for modern Windows and online multiplayer."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDgVoodoo} "Fixes rendering problems on modern Windows and makes options like anti-aliasing easy to turn on. Leave it checked unless you have a specific reason not to."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "A Codename Eagle Multiplayer Demo shortcut on the desktop."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  ; The removal below is recursive - refuse anything that doesn't look like the
  ; folder we installed (stale/tampered InstallLocation, drive roots).
  StrLen $0 "$INSTDIR"
  ${If} $0 < 5
    Abort "Refusing to remove $INSTDIR."
  ${EndIf}
  ${IfNot} ${FileExists} "$INSTDIR\ce.exe"
    MessageBox MB_OK|MB_ICONSTOP "$INSTDIR doesn't look like a Codename Eagle folder (no ce.exe) - not removing any files."
    Abort "Not a Codename Eagle folder."
  ${EndIf}

  DetailPrint "Removing Windows Firewall rules..."
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${FWRULE_GAME}"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${FWRULE_LOBBY}"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${FWRULE_IPLIST}"'
  Pop $0

  ; cneagle:// protocol handler: ours (HKLM) and the per-user key ce.exe
  ; re-registers on every launch (HKCU, only stays gone once the game is
  ; removed).
  DeleteRegKey HKLM "Software\Classes\cneagle"
  DeleteRegKey HKCU "Software\Classes\cneagle"

  Delete "$DESKTOP\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk"
  RMDir "$SMPROGRAMS\${APPNAME}"

  ; The whole game folder, including logs, screenshots and saves.
  RMDir /r "$INSTDIR"

  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
