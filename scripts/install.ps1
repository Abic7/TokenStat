# Installs TokenStat for the current user. No admin rights needed.
# - Copies tokenstat.exe to %LOCALAPPDATA%\Programs\TokenStat
# - Backs up %USERPROFILE%\.claude\settings.json, then adds TokenStat's status line and Stop hook.
#   An existing status line is kept: TokenStat runs it and shows its output.
# - Starts TokenStat. "Start with Windows" and auto-refresh stay off until you turn them on in the menu.
param(
    [string]$Exe = "$env:LOCALAPPDATA\TokenStat-build\release\tokenstat.exe"
)
$ErrorActionPreference = 'Stop'

$bin = "$env:LOCALAPPDATA\Programs\TokenStat"
$data = "$env:LOCALAPPDATA\TokenStat"
$settings = "$env:USERPROFILE\.claude\settings.json"
$target = "$bin\tokenstat.exe"

if (-not (Test-Path $Exe)) { throw "tokenstat.exe not found at $Exe. Build first: cargo build --release" }

Get-Process tokenstat -ErrorAction SilentlyContinue | Stop-Process -Force
New-Item -ItemType Directory -Force $bin, $data | Out-Null
Copy-Item $Exe $target -Force

$cfg = [pscustomobject]@{}
if (Test-Path $settings) {
    Copy-Item $settings "$settings.tokenstat-backup-$(Get-Date -Format yyyyMMdd-HHmmss)"
    $cfg = Get-Content $settings -Raw | ConvertFrom-Json
} else {
    New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null
}

$statusCmd = "`"$target`" --statusline"
$activityCmd = "`"$target`" --activity"

# Remember the user's own status line once, so uninstall can restore it and the hook can chain it.
$original = "$data\original-statusline.json"
$current = $cfg.statusLine
$isOurs = $current -and ($current.command -match 'tokenstat|capture-statusline')
if (-not (Test-Path $original)) {
    if ($current -and -not $isOurs) { $current | ConvertTo-Json -Depth 10 | Set-Content $original -Encoding UTF8 }
    else { 'null' | Set-Content $original -Encoding UTF8 }
}
if ($current -and -not $isOurs -and $current.command) {
    [IO.File]::WriteAllText("$data\chain.txt", $current.command)
}

$cfg | Add-Member -NotePropertyName statusLine -NotePropertyValue ([pscustomobject]@{ type = 'command'; command = $statusCmd }) -Force

if (-not $cfg.hooks) { $cfg | Add-Member -NotePropertyName hooks -NotePropertyValue ([pscustomobject]@{}) -Force }
$stop = @(@($cfg.hooks.Stop) | Where-Object { $_ -and -not ($_.hooks.command -match 'tokenstat') })
$stop += [pscustomobject]@{ hooks = @([pscustomobject]@{ type = 'command'; command = $activityCmd }) }
$cfg.hooks | Add-Member -NotePropertyName Stop -NotePropertyValue $stop -Force

[IO.File]::WriteAllText($settings, (ConvertTo-Json -InputObject $cfg -Depth 50), (New-Object Text.UTF8Encoding $false))

# Start menu shortcut and an entry in Settings > Apps, both per-user.
$shortcut = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\TokenStat.lnk"
$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut($shortcut)
$lnk.TargetPath = $target
$lnk.WorkingDirectory = $bin
$lnk.IconLocation = "$target,0"
$lnk.Description = 'Claude usage at a glance'
$lnk.Save()

Copy-Item "$PSScriptRoot\uninstall.ps1" "$bin\uninstall.ps1" -Force
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\TokenStat'
New-Item -Path $uninstallKey -Force | Out-Null
$props = @{
    DisplayName     = 'TokenStat'
    DisplayIcon     = "$target,0"
    DisplayVersion  = '0.1.0'
    Publisher       = 'TokenStat (open source, MIT)'
    InstallLocation = $bin
    UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$bin\uninstall.ps1`""
    NoModify        = 1
    NoRepair        = 1
}
foreach ($k in $props.Keys) {
    $type = if ($props[$k] -is [int]) { 'DWord' } else { 'String' }
    New-ItemProperty -Path $uninstallKey -Name $k -Value $props[$k] -PropertyType $type -Force | Out-Null
}

Start-Process $target
Write-Output "TokenStat installed to $bin and started. Find it in the Start menu as TokenStat."
Write-Output "Close and reopen any running Claude Code sessions (terminal and VS Code) so they pick up the new settings."
