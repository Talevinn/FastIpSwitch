# Signe l'exécutable avec signtool (Authenticode).
# Usage :
#   $env:FASTIPSWITCH_PFX = "certs\fastipswitch-codesign.pfx"
#   $env:FASTIPSWITCH_PFX_PASSWORD = "..."
#   pwsh -File tools\sign.ps1
# ou avec un certificat déjà installé dans le magasin utilisateur :
#   $env:FASTIPSWITCH_CERT_THUMBPRINT = "ABCDEF..."
param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\fastipswitch.exe'),
    [string]$Pfx = $env:FASTIPSWITCH_PFX,
    [string]$PfxPassword = $env:FASTIPSWITCH_PFX_PASSWORD,
    [string]$Thumbprint = $env:FASTIPSWITCH_CERT_THUMBPRINT
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $Exe)) {
    throw "Binaire introuvable : $Exe (lance d'abord cargo build --release)"
}

$kits = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
    "$env:ProgramFiles\Windows Kits\10\bin"
) | Where-Object { $_ -and (Test-Path $_) }

$signtool = $null
foreach ($kit in $kits) {
    $signtool = Get-ChildItem (Join-Path $kit '*\x64\signtool.exe') -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
    if ($signtool) { break }
}

if (-not $signtool) {
    throw "signtool.exe introuvable. Installe les Windows SDK Signing Tools (ou le Windows SDK)."
}

$timestamp = 'http://timestamp.digicert.com'

if ($Pfx) {
    if (-not (Test-Path $Pfx)) { throw "PFX introuvable : $Pfx" }
    & $signtool sign /fd SHA256 /td SHA256 /tr $timestamp /f $Pfx /p $PfxPassword $Exe
} elseif ($Thumbprint) {
    & $signtool sign /fd SHA256 /td SHA256 /tr $timestamp /sha1 $Thumbprint $Exe
} else {
    throw "Fournis FASTIPSWITCH_PFX + FASTIPSWITCH_PFX_PASSWORD, ou FASTIPSWITCH_CERT_THUMBPRINT."
}

if ($LASTEXITCODE -ne 0) {
    throw "signtool sign a échoué (code $LASTEXITCODE)."
}

& $signtool verify /pa /v $Exe
if ($LASTEXITCODE -ne 0) {
    throw "La vérification de signature a échoué (code $LASTEXITCODE)."
}

$hash = (Get-FileHash $Exe -Algorithm SHA256).Hash
Write-Host ""
Write-Host "Signature OK."
Write-Host "  Exe    : $Exe"
Write-Host "  SHA256 : $hash"
Write-Host "Pense à mettre à jour le SHA-256 de la release après signature."
