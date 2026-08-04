$ErrorActionPreference = "Stop"
$repoDir = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $repoDir "crates/voxelle-tauri-host")

cargo tauri build --bundles nsis
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$bundleDir = Join-Path $repoDir "target/release/bundle/nsis"
Get-ChildItem $bundleDir -Filter *.exe | ForEach-Object {
  $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  "$hash  $($_.Name)"
} | Set-Content (Join-Path $bundleDir "SHA256SUMS.txt")
Write-Host "Built unsigned Windows release in $bundleDir"
