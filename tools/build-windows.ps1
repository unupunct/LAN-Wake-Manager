# Builds the Windows release: dashboard -> Rust services (release) -> desktop window (Tauri) -> three MSI installers.
# Requires: Rust, Node.js, WiX v5 (dotnet tool install --global wix; wix extension add -g WixToolset.UI.wixext WixToolset.Util.wixext WixToolset.Firewall.wixext)
param([string]$Version = "1.0.0", [switch]$SkipUi, [switch]$SkipDesktop)
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."; Set-Location $root
$stage = Join-Path $root "target\stage"; $out = Join-Path $root "release\windows"
New-Item -ItemType Directory -Force $stage, $out | Out-Null

if (-not $SkipUi) { Push-Location ui; npm ci; npm run build; Pop-Location }
cargo build --release --workspace
if ($LASTEXITCODE) { throw "cargo build failed" }
foreach ($b in "lwm-controller", "lwm-relay", "lwm-agent") { Copy-Item "target\release\$b.exe" $stage -Force }
if (-not $SkipDesktop) {
  Push-Location desktop\src-tauri; cargo build --release; if ($LASTEXITCODE) { throw "desktop build failed" }; Pop-Location
}
Copy-Item "desktop\src-tauri\target\release\lan-wake-manager.exe" $stage -Force

$lic = (Get-Content "$root\LICENSE" -Raw)
$rtf = "{\rtf1\ansi\deff0{\fonttbl{\f0 Segoe UI;}}\fs18 " + ($lic -replace '\\', '\\\\' -replace '\{', '\{' -replace '\}', '\}' -replace "`r?`n", '\par ') + "}"
$rtfPath = Join-Path $stage "License.rtf"; [IO.File]::WriteAllText($rtfPath, $rtf, [Text.Encoding]::ASCII)
$icon = Join-Path $root "desktop\src-tauri\icons\icon.ico"
$ext = "-ext", "WixToolset.UI.wixext", "-ext", "WixToolset.Util.wixext", "-ext", "WixToolset.Firewall.wixext"
foreach ($p in @(@("Setup", "LAN-Wake-Manager-Setup"), @("Relay", "LAN-Wake-Manager-Relay-Setup"), @("Agent", "LAN-Wake-Manager-Agent-Setup"))) {
  wix build "installer\windows\$($p[0]).wxs" -arch x64 @ext -d Version=$Version -d StageDir=$stage -d IconFile=$icon -d LicenseRtf=$rtfPath -o "$out\$($p[1]).msi"
  if ($LASTEXITCODE) { throw "wix build $($p[0]) failed" }
}
Get-ChildItem $out -Filter *.msi | Select-Object Name, @{n = "MB"; e = { [math]::Round($_.Length / 1MB, 1) } }
