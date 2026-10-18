param(
    [Parameter(Mandatory)][string]$NavigationRoot,
    [string]$PythonExe = 'python',
    [string]$OutputRoot = "$PSScriptRoot\..\dist\Tombsward-Minimap-Test",
    [string]$BuildRoot = '',
    [string]$CargoHome = '',
    [string]$RustupHome = '',
    [string]$GccBin = '',
    [string]$WorldMapGameRoot = '',
    [string[]]$OutdoorNavigationRoots = @(),
    [switch]$Offline,
    [switch]$SmallDungeons
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
$profileName=if ($SmallDungeons) {'dungeon-test.me3'} else {'tombsward-test.me3'}
Copy-Item -LiteralPath (Join-Path $sourceRoot 'package\tombsward-test.me3') -Destination (Join-Path $OutputRoot $profileName)
Copy-Item -LiteralPath (Join-Path $sourceRoot 'LICENSE'),(Join-Path $sourceRoot 'TESTING.md') -Destination $OutputRoot
$ini = Get-Content -Raw -LiteralPath (Join-Path $sourceRoot 'package\minimap.ini')
$ini = $ini.Replace('dungeon_mode = false','dungeon_mode = true').Replace('size = 1.0','size = 1.5')
[IO.File]::WriteAllText((Join-Path $OutputRoot 'minimap.ini'),$ini,[Text.UTF8Encoding]::new($false))
$dataOptions=@()
if ($SmallDungeons) { $dataOptions+='--small-dungeons' }
& $PythonExe (Join-Path $PSScriptRoot 'prepare_dungeon.py') $NavigationRoot $OutputRoot @dataOptions
if ($LASTEXITCODE -ne 0) { throw 'Dungeon data preparation failed' }
if ($OutdoorNavigationRoots.Count) {
    & $PythonExe (Join-Path $PSScriptRoot 'prepare_outdoor.py') @OutdoorNavigationRoots --output $OutputRoot
    if ($LASTEXITCODE -ne 0) { throw 'Outdoor data preparation failed' }
    $env:OUTDOOR_NAV_TEST_DIR=Join-Path $OutputRoot 'outdoor'
} else { Remove-Item Env:OUTDOOR_NAV_TEST_DIR -ErrorAction SilentlyContinue }
$env:TOMBSWARD_NAV_TEST_FILE = Join-Path $OutputRoot 'tombsward.nav'
if ($SmallDungeons) { $env:DUNGEON_NAV_TEST_DIR=Join-Path $OutputRoot 'dungeons' }
else { Remove-Item Env:DUNGEON_NAV_TEST_DIR -ErrorAction SilentlyContinue }
$testOptions=@('--include-ignored')
if (-not $env:MINIMAP_MARKER_TEST_CSV -or -not $OutdoorNavigationRoots.Count) { $testOptions+=@('--skip','real_map_points_and_graces_align_with_local_layouts') }
if (-not $OutdoorNavigationRoots.Count) { $testOptions+=@('--skip','real_outdoor_tiles_decode_and_reveal_overlapping_walkable_levels','--skip','real_outdoor_lower_and_upper_layers_render_separately','--skip','real_outdoor_loader_keeps_worlds_and_event_variants_separate','--skip','real_lower_terrain_walks_keep_the_region_and_mode_stable') }
if ($WorldMapGameRoot) { $env:MINIMAP_WORLD_TEST_ROOT=[IO.Path]::GetFullPath($WorldMapGameRoot) }
else { $testOptions+=@('--skip','real_dlc_atlas_selects_and_decodes_game_tiles') }
if (-not $SmallDungeons) { $testOptions+=@('--skip','real_maps_decode_and_background_loader_switches_exact_blocks','--skip','all_small_dungeons_render_at_multiple_heights_views_and_zooms','--skip','dungeon_selection_clears_old_geometry_during_rapid_warps') }
& cargo test --release --lib @common -- @testOptions
if ($LASTEXITCODE -ne 0) { throw 'Geometry/data tests failed' }
Get-ChildItem -LiteralPath $OutputRoot -File -Recurse | Where-Object Name -NE 'SHA256SUMS.txt' | Sort-Object FullName | Get-FileHash -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash.ToLower())  $([IO.Path]::GetRelativePath($OutputRoot,$_.Path).Replace('\','/'))" } |
    Set-Content -LiteralPath (Join-Path $OutputRoot 'SHA256SUMS.txt') -Encoding ascii
Write-Output "Private local test package: $OutputRoot"
