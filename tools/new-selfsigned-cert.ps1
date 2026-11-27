# Crée un certificat auto-signé de signature de code pour FastIpSwitch.
# Usage (Windows PowerShell, pas nécessairement PowerShell 7) :
#   powershell.exe -NoProfile -File tools\new-selfsigned-cert.ps1
# Le PFX et le certificat public sont écrits dans certs\ (ignoré par git).
param(
    [string]$Subject = "CN=FastIpSwitch (auto-signe)",
    [string]$OutDir = (Join-Path (Split-Path -Parent $PSScriptRoot) 'certs'),
    [securestring]$Password
)

$ErrorActionPreference = 'Stop'

if (-not $Password) {
    $Password = Read-Host -AsSecureString "Mot de passe du PFX"
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -KeyUsage DigitalSignature `
    -FriendlyName "FastIpSwitch Code Signing" `
    -CertStoreLocation Cert:\CurrentUser\My `
    -NotAfter (Get-Date).AddYears(5)

$pfx = Join-Path $OutDir 'fastipswitch-codesign.pfx'
$cer = Join-Path $OutDir 'fastipswitch-codesign.cer'

Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $Password | Out-Null
Export-Certificate -Cert $cert -FilePath $cer | Out-Null

Write-Host ""
Write-Host "Certificat créé."
Write-Host "  PFX          : $pfx"
Write-Host "  Certificat   : $cer"
Write-Host "  Thumbprint   : $($cert.Thumbprint)"
Write-Host ""
Write-Host "Pour faire confiance au certificat sur un poste cible (admin non requis, magasin utilisateur) :"
Write-Host "  certutil -addstore -user Root `"$cer`""
Write-Host "  certutil -addstore -user TrustedPublisher `"$cer`""
Write-Host ""
Write-Host "Puis signer :"
Write-Host "  `$env:FASTIPSWITCH_PFX = `"$pfx`""
Write-Host "  `$env:FASTIPSWITCH_PFX_PASSWORD = `"...`""
Write-Host "  pwsh -File tools\sign.ps1"
