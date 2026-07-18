#!/bin/sh
# Boot a Codename Eagle dedicated server under Wine.
#
# ce.exe's +dedicated mode skips the menu, cutscenes and video init and opens a
# server console, but it still creates a window - hence the Xvfb display.
set -eu

: "${CE_HOSTNAME:=CE Dedicated}"
: "${CE_MAP:=No mans land}"
: "${CE_MAXPLAYERS:=16}"
: "${CE_GAME:=ctf}"     # ctf | deathmatch | teamplay
: "${CE_PORT:=}"        # optional +host port override (default 24711)
: "${CE_LOG_ERROR:=0}"  # 1 to also stream the noisy engine error.log ([error])

export WINEPREFIX="${WINEPREFIX:-/wineprefix}"
export WINEDEBUG="${WINEDEBUG:--all}"
# The image ships no graphics wrapper, so Wine uses its builtin ddraw/d3d by
# default, which is fine for a headless server under Xvfb. Only suppress the
# mshtml/mono installer prompts here.
export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:-mshtml=;mscoree=}"

export DISPLAY=:99
Xvfb :99 -screen 0 640x480x16 -nolisten tcp &

if [ ! -f "$WINEPREFIX/system.reg" ]; then
	echo "[ce] initializing wine prefix at $WINEPREFIX"
	wineboot --init
	wineserver -w
	# No audio device in the container: disable the audio driver outright.
	wine reg add 'HKCU\Software\Wine\Drivers' /v Audio /t REG_SZ /d '' /f
	# Don't pop the winedbg crash dialog (would hang a headless container).
	wine reg add 'HKCU\Software\Wine\WineDbg' /v ShowCrashDialog /t REG_DWORD /d 0 /f
	wineserver -w
fi

cd /opt/ce

# Logs streamed to the container stdout, each line tagged by source:
#   [server] the dedicated-server events (server loaded, player join/leave/lost
#            connection/out of sync). The engine prints these only via
#            WriteConsoleA, which is discarded on a headless host (no usable
#            console), so the 1.50 patch also appends them to logs/server.log -
#            that is what we tail here.
#   [lobby]  the background lobby helper's logs/lobby.log.
#   [error]  the engine's logs/error.log (name picked from the first cmdline
#            char; '+...' selects the default). Very noisy, so opt-in via
#            CE_LOG_ERROR=1.
# The game creates the files on launch; tail -F waits for them and follows a
# truncate/recreate.
LOGDIR=/opt/ce/logs
mkdir -p "$LOGDIR"
: >"$LOGDIR/server.log"
: >"$LOGDIR/lobby.log"
(tail -F "$LOGDIR/server.log" 2>/dev/null | sed -u 's/^/[server] /') &
(tail -F "$LOGDIR/lobby.log" 2>/dev/null | sed -u 's/^/[lobby] /') &
if [ "$CE_LOG_ERROR" != "0" ]; then
	: >"$LOGDIR/error.log"
	(tail -F "$LOGDIR/error.log" 2>/dev/null | sed -u 's/^/[error] /') &
fi

# The engine's string flags (+game/+hostname/+map/+name) require literal
# double quotes in the raw Windows command line, but Wine only re-quotes argv
# elements that contain spaces - a bare `+game deathmatch` is silently
# dropped. A .bat launcher sidesteps this: cmd reads it verbatim from disk
# and passes the raw line to CreateProcess (cmd in batch mode also waits for
# GUI apps like ce.exe, keeping the container alive).
BAT="$WINEPREFIX/drive_c/ce-launch.bat"
printf '%s\r\n' \
	'@echo off' \
	'cd /d Z:\opt\ce' \
	"ce.exe +dedicated +host $CE_PORT +maxplayers $CE_MAXPLAYERS +game \"$CE_GAME\" +hostname \"$CE_HOSTNAME\" +map \"$CE_MAP\"" \
	>"$BAT"

echo "[ce] starting dedicated server: hostname='$CE_HOSTNAME' map='$CE_MAP' maxplayers=$CE_MAXPLAYERS game=$CE_GAME port=${CE_PORT:-24711}"
exec wine cmd /c 'c:\ce-launch.bat'
