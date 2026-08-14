param(
  [Parameter(Mandatory = $true)][string]$Template,
  [Parameter(Mandatory = $true)][string]$Output,
  [Parameter(Mandatory = $true)][string]$Installer,
  [Parameter(Mandatory = $true)][string]$InstalledExecutable,
  [Parameter(Mandatory = $true)][string]$Operator
)

$ErrorActionPreference = "Stop"

if (-not $IsWindows -and $env:OS -ne "Windows_NT") {
  throw "the Windows beta smoke receipt must be recorded on Windows"
}
if (Test-Path -LiteralPath $Output) {
  throw "refusing to overwrite evidence output: $Output"
}
foreach ($path in @($Template, $Installer, $InstalledExecutable)) {
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
    throw "required file is missing: $path"
  }
}
if ([string]::IsNullOrWhiteSpace($Operator)) {
  throw "Operator must be non-empty"
}
if ((Split-Path -Leaf $InstalledExecutable) -ne "voxelle-tauri-host.exe") {
  throw "installed executable must be the Voxelle NSIS payload voxelle-tauri-host.exe"
}

$evidence = Get-Content -LiteralPath $Template -Raw | ConvertFrom-Json
if ($evidence.format -ne "voxelle-beta-evidence/v1") {
  throw "unsupported beta evidence template format"
}
$actualHash = (Get-FileHash -LiteralPath $Installer -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $evidence.windows.installer_sha256.ToLowerInvariant()) {
  throw "installer does not match the signed hash in the evidence template"
}
if ((Split-Path -Leaf $Installer) -ne $evidence.windows.installer_name) {
  throw "installer name does not match the signed evidence template"
}

$architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($architecture -ne "X64") {
  throw "native Windows beta evidence requires X64 Windows; found $architecture"
}

$process = Start-Process -FilePath $InstalledExecutable -PassThru
$deadline = [DateTime]::UtcNow.AddSeconds(30)
$windowVisible = $false
while ([DateTime]::UtcNow -lt $deadline) {
  Start-Sleep -Milliseconds 500
  $process.Refresh()
  if ($process.HasExited) {
    throw "Voxelle exited before presenting its main window (exit $($process.ExitCode))"
  }
  if ($process.MainWindowHandle -ne 0 -and $process.MainWindowTitle -eq "Voxelle") {
    $windowVisible = $true
    break
  }
}
if (-not $windowVisible) {
  throw "Voxelle stayed alive but did not present a visible main window within 30 seconds"
}

$currentVersion = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion"
$evidence.windows.os_product_name = [string]$currentVersion.ProductName
$evidence.windows.os_version = [Environment]::OSVersion.Version.ToString()
$evidence.windows.os_build = [string][Environment]::OSVersion.Version.Build
$evidence.windows.architecture = $architecture
$evidence.windows.installed_executable_name = Split-Path -Leaf $InstalledExecutable
$evidence.windows.process_started = $true
$evidence.windows.main_window_visible = $true
$evidence.windows.first_launch_utc = [DateTime]::UtcNow.ToString("o")
$evidence.windows.operator = $Operator

$outputFull = [IO.Path]::GetFullPath($Output)
$outputDir = Split-Path -Parent $outputFull
if (-not (Test-Path -LiteralPath $outputDir)) {
  New-Item -ItemType Directory -Path $outputDir | Out-Null
}
$temporary = Join-Path $outputDir ((Split-Path -Leaf $outputFull) + ".tmp")
if (Test-Path -LiteralPath $temporary) {
  throw "refusing to overwrite temporary evidence file: $temporary"
}
$json = $evidence | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($temporary, $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
Move-Item -LiteralPath $temporary -Destination $outputFull

Write-Host "Recorded native Windows first-launch evidence in $outputFull"
Write-Host "Voxelle remains open for operator inspection."
