# Removes TokenStat for the current user and restores your Claude Code settings.
# Use -KeepData to keep %LOCALAPPDATA%\TokenStat (settings, pill position, last reading).
param([switch]$KeepData)
$ErrorActionPreference = 'Stop'

$bin = "$env:LOCALAPPDATA\Programs\TokenStat"
$data = "$env:LOCALAPPDATA\TokenStat"
$settings = "$env:USERPROFILE\.claude\settings.json"

Get-Process tokenstat -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'TokenStat' -ErrorAction SilentlyContinue
Remove-Item 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\TokenStat' -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\TokenStat.lnk" -Force -ErrorAction SilentlyContinue

if (Test-Path $settings) {
    Copy-Item $settings "$settings.tokenstat-uninstall-backup-$(Get-Date -Format yyyyMMdd-HHmmss)"
    $cfg = Get-Content $settings -Raw | ConvertFrom-Json

    if ($cfg.statusLine -and $cfg.statusLine.command -match 'tokenstat') {
        $original = $null
        if (Test-Path "$data\original-statusline.json") { $original = Get-Content "$data\original-statusline.json" -Raw | ConvertFrom-Json }
        if ($original) { $cfg.statusLine = $original } else { $cfg.PSObject.Properties.Remove('statusLine') }
    }

    if ($cfg.hooks -and $cfg.hooks.Stop) {
        $stop = @(@($cfg.hooks.Stop) | Where-Object { $_ -and -not ($_.hooks.command -match 'tokenstat') })
        if ($stop.Count -gt 0) { $cfg.hooks.Stop = $stop } else { $cfg.hooks.PSObject.Properties.Remove('Stop') }
        if (@($cfg.hooks.PSObject.Properties).Count -eq 0) { $cfg.PSObject.Properties.Remove('hooks') }
    }

    [IO.File]::WriteAllText($settings, (ConvertTo-Json -InputObject $cfg -Depth 50), (New-Object Text.UTF8Encoding $false))
}

Remove-Item $bin -Recurse -Force -ErrorAction SilentlyContinue
if (-not $KeepData) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue }
Write-Output "TokenStat removed. Restart any open Claude Code sessions."
