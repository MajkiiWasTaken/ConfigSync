# /************************************************
# * File: install.ps1
# * Author: Michal Švrček
# * Windows per-user installer
# * ver. 0.4.0
# *************************************************/
[CmdletBinding()]
param([string]$BinaryPath)
$ErrorActionPreference = 'Stop'
if (-not $BinaryPath) { $BinaryPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\csync.exe' }
if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) { throw "Binary missing: $BinaryPath. Run cargo build --release first." }
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\ConfigSync'
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
$destination = Join-Path $installDir 'csync.exe'
if ((Resolve-Path -LiteralPath $BinaryPath).Path -ine $destination) { Copy-Item -LiteralPath $BinaryPath -Destination $destination -Force }
$pathValue = [Environment]::GetEnvironmentVariable('Path','User')
$entries = @($pathValue -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
if (-not @($entries | Where-Object { $_.TrimEnd('\') -ieq $installDir.TrimEnd('\') }).Count) {
    [Environment]::SetEnvironmentVariable('Path', (($entries + $installDir) -join ';'), 'User')
}
& $destination --version
Write-Host "[OK] Installed to $destination" -ForegroundColor Green
Write-Host 'Open a new terminal and use: csync paths'
