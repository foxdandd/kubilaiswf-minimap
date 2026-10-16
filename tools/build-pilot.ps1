param(
    [Parameter(Mandatory)][string]$NavigationRoot,
    [string]$PythonExe = 'python',
    [string]$OutputRoot = "$PSScriptRoot\..\dist\Tombsward-Minimap-Test",
    [string]$BuildRoot = '',
    [string]$CargoHome = '',
    [string]$RustupHome = '',
    [string]$GccBin = '',
    [switch]$Offline
)
$ErrorActionPreference = 'Stop'
$sourceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
if ($CargoHome) { $env:CARGO_HOME = [IO.Path]::GetFullPath($CargoHome); $env:PATH = "$env:CARGO_HOME\bin;$env:PATH" }
if ($RustupHome) { $env:RUSTUP_HOME = [IO.Path]::GetFullPath($RustupHome) }
if ($BuildRoot) { $env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($BuildRoot) }
if ($GccBin) {
    $GccBin = [IO.Path]::GetFullPath($GccBin)
    $env:PATH = "$GccBin;$env:PATH"
    $env:CC = Join-Path $GccBin 'gcc.exe'
    $env:CXX = Join-Path $GccBin 'g++.exe'
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $env:CC
    $env:RUSTFLAGS = '-C link-self-contained=yes'
}
$common = @('--locked', '--manifest-path', (Join-Path $sourceRoot 'Cargo.toml'))
if ($Offline) { $common += '--offline' }
& cargo build --release @common
if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $sourceRoot 'target' }
New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null
Copy-Item -LiteralPath (Join-Path $target 'release\kubilaiswf_minimap.dll') -Destination (Join-Path $OutputRoot 'tombsward-minimap-test.dll')
Copy-Item -LiteralPath (Join-Path $sourceRoot 'package\tombsward-test.me3') -Destination $OutputRoot
Copy-Item -LiteralPath (Join-Path $sourceRoot 'LICENSE'),(Join-Path $sourceRoot 'TESTING.md') -Destination $OutputRoot
$ini = Get-Content -Raw -LiteralPath (Join-Path $sourceRoot 'package\minimap.ini')
$ini = $ini.Replace('dungeon_mode = false','dungeon_mode = true').Replace('size = 1.0','size = 1.5')
[IO.File]::WriteAllText((Join-Path $OutputRoot 'minimap.ini'),$ini,[Text.UTF8Encoding]::new($false))
& $PythonExe (Join-Path $PSScriptRoot 'prepare_dungeon.py') $NavigationRoot $OutputRoot
if ($LASTEXITCODE -ne 0) { throw 'Dungeon data preparation failed' }
$env:TOMBSWARD_NAV_TEST_FILE = Join-Path $OutputRoot 'tombsward.nav'
& cargo test --release --lib @common -- --include-ignored
if ($LASTEXITCODE -ne 0) { throw 'Geometry/data tests failed' }
Get-ChildItem -LiteralPath $OutputRoot -File | Where-Object Name -NE 'SHA256SUMS.txt' | Get-FileHash -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash.ToLower())  $([IO.Path]::GetFileName($_.Path))" } |
    Set-Content -LiteralPath (Join-Path $OutputRoot 'SHA256SUMS.txt') -Encoding ascii
Write-Output "Private local test package: $OutputRoot"
