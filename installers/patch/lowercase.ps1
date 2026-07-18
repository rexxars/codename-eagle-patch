# Lowercase every file and directory name under the given root that isn't
# already lowercase (LEVEL1 -> level1, GAME.EXE -> game.exe, ...). Cosmetic on
# Windows, but matches the repo/payload naming convention so patched installs
# look uniform.
#
# Invoked by the 1.50 patch installer (installer.nsi) from $PLUGINSDIR; safe to
# run by hand: powershell -NoProfile -ExecutionPolicy Bypass -File lowercase.ps1 <gamedir>
param([Parameter(Mandatory = $true)][string]$Root)

# Rename failures (locked files, exotic permissions) are cosmetic - keep going.
$ErrorActionPreference = 'Continue'

# Files first, then directories deepest-first, so no rename invalidates a path
# we still have to visit. Case-only FILE renames work directly on NTFS via
# Rename-Item; DIRECTORIES need the two-step below.
Get-ChildItem -LiteralPath $Root -Recurse -File |
  Where-Object { $_.Name -cne $_.Name.ToLowerInvariant() } |
  ForEach-Object { Rename-Item -LiteralPath $_.FullName -NewName $_.Name.ToLowerInvariant() }

# Rename-Item rejects case-only renames of directories ("Source and destination
# path must be different" - its check is case-insensitive), so hop through a
# temp name. If the second step fails, roll back so no MENU.lc-tmp is left
# behind - a wrongly-cased dir is cosmetic, a wrongly-NAMED one breaks the game.
Get-ChildItem -LiteralPath $Root -Recurse -Directory |
  Sort-Object { $_.FullName.Length } -Descending |
  Where-Object { $_.Name -cne $_.Name.ToLowerInvariant() } |
  ForEach-Object {
    $parent = $_.Parent.FullName
    $tmp = $_.Name + '.lc-tmp'
    $tmpPath = Join-Path $parent $tmp
    if (Test-Path -LiteralPath $tmpPath) { return } # stale tmp from a crash: skip
    try {
      Rename-Item -LiteralPath $_.FullName -NewName $tmp -ErrorAction Stop
      Rename-Item -LiteralPath $tmpPath -NewName $_.Name.ToLowerInvariant() -ErrorAction Stop
    } catch {
      if (Test-Path -LiteralPath $tmpPath) {
        Rename-Item -LiteralPath $tmpPath -NewName $_.Name -ErrorAction SilentlyContinue
      }
    }
  }
