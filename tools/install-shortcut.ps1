# Raccourci bureau vers FastIpSwitch (release)
# Usage : pwsh -File tools\install-shortcut.ps1
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\fastipswitch.exe'

if (-not (Test-Path $exe)) {
    throw "Binaire introuvable : $exe`nLance d'abord : cargo build --release"
}

$desktop = [Environment]::GetFolderPath('Desktop')
$linkPath = Join-Path $desktop 'FastIpSwitch.lnk'

$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($linkPath)
$link.TargetPath = (Resolve-Path -LiteralPath $exe).Path
$link.WorkingDirectory = (Split-Path -Parent (Resolve-Path -LiteralPath $exe).Path)
$link.IconLocation = "$((Resolve-Path -LiteralPath $exe).Path),0"
$link.Description = 'FastIpSwitch - changement rapide IP et IP virtuelles'
$link.Arguments = '--elevate'
$link.Save()

Write-Output "Raccourci créé : $linkPath"
