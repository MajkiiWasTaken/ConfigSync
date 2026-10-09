# /************************************************
# * File: uninstall.ps1
# * Author: Michal Švrček
# * Windows per-user uninstaller (preserves settings/backups)
# * ver. 0.4.0
# *************************************************/
$ErrorActionPreference = 'Stop'
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\ConfigSync'
$binary = Join-Path $installDir 'csync.exe'
if (Test-Path $binary) { Remove-Item -LiteralPath $binary -Force }
$pathValue = [Environment]::GetEnvironmentVariable('Path','User')
$entries = @($pathValue -split ';' | Where-Object { $_ -and $_.TrimEnd('\') -ine $installDir.TrimEnd('\') })
[Environment]::SetEnvironmentVariable('Path', ($entries -join ';'), 'User')
Write-Host '[OK] Removed binary and PATH entry; configuration and backups retained.' -ForegroundColor Green
