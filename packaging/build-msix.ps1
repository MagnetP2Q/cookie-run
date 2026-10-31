param(
  [Parameter(Mandatory=$true)][string]$Publisher,
  [Parameter(Mandatory=$true)][string]$Certificate,
  [Parameter(Mandatory=$true)][string]$CertificatePassword,
  [string]$MakeAppx = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\makeappx.exe',
  [string]$SignTool = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe'
)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if (!(Test-Path -LiteralPath $MakeAppx) -or !(Test-Path -LiteralPath $SignTool) -or !(Test-Path -LiteralPath $Certificate)) { throw 'Windows SDK tools or signing certificate not found.' }
if ($Publisher -notmatch '^CN=[A-Za-z0-9 ._-]+$') { throw 'Publisher must be a simple CN= name matching the signing certificate subject.' }
npm run tauri -- build --no-bundle
if ($LASTEXITCODE -ne 0) { throw 'Tauri build failed.' }
$stage = Join-Path $PWD 'msix-stage'
New-Item -ItemType Directory -Force (Join-Path $stage 'Assets') | Out-Null
Copy-Item 'src-tauri\target\release\cookie-run.exe' (Join-Path $stage 'cookie-run.exe') -Force
Copy-Item 'src-tauri\icons\Square150x150Logo.png' (Join-Path $stage 'Assets\Square150x150Logo.png') -Force
Copy-Item 'src-tauri\icons\Square44x44Logo.png' (Join-Path $stage 'Assets\Square44x44Logo.png') -Force
Copy-Item 'src-tauri\icons\StoreLogo.png' (Join-Path $stage 'Assets\StoreLogo.png') -Force
$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:desktop="http://schemas.microsoft.com/appx/manifest/desktop/windows10" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap desktop rescap">
  <Identity Name="CookieRun.Bot" Publisher="$Publisher" Version="0.1.0.0" ProcessorArchitecture="x64" />
  <Properties><DisplayName>Cookie Run Bot</DisplayName><PublisherDisplayName>Cookie Run Bot</PublisherDisplayName><Logo>Assets\StoreLogo.png</Logo></Properties>
  <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" /></Dependencies>
  <Resources><Resource Language="en-us" /></Resources>
  <Applications><Application Id="App" Executable="cookie-run.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="Cookie Run Bot" Description="Cookie Run Bot" BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" /></Application></Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
</Package>
"@
[IO.File]::WriteAllText((Join-Path $stage 'AppxManifest.xml'), $manifest, [Text.UTF8Encoding]::new($false))
& $MakeAppx pack /d $stage /p 'CookieRun.msix' /o
if ($LASTEXITCODE -ne 0) { throw 'MSIX packing failed.' }
& $SignTool sign /fd SHA256 /f $Certificate /p $CertificatePassword 'CookieRun.msix'
if ($LASTEXITCODE -ne 0) { throw 'MSIX signing failed.' }
Write-Host 'Created CookieRun.msix. Install only on a machine that trusts the signing certificate.'
