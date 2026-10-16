# Dungeon and Shadow of the Erdtree minimap pilot 4

Local test build based on kubilaiswf's MIT-licensed Minimap, upstream commit
ff2b9e4ce7b71ddcb503acd202081953183173aa. Keep the original copyright and LICENSE.

## What to test

This adds an optional navigation-layout view for 60 small dungeon map blocks:
48 base-game catacombs, caves and tunnels (areas m30/m31/m32), plus 12 DLC
catacomb, gaol, forge and cave blocks (m40/m41/m42/m43). The catalog includes
one unnamed forge block, shown by its map ID. `dungeons/maps.tsv` lists the exact
IDs and available names; the count is map blocks, not verified distinct dungeons.
Detailed navigation layouts for large legacy dungeons, overworld areas and overhaul
layouts are outside this build. Outdoor views use the game's world-map textures.

Pilot 4 adds the Shadow of the Erdtree outdoor map (area m61, atlas M10), read
directly from the installed game's map-tile archives. No extra outdoor map download
or generated map sidecar is needed. Like the base-game view, it shows fully revealed
terrain without granting fragments or changing discovery/save state. Discovered
graces, visible world-map points, numbered beacons and the optional trail now use
the DLC's separate map identity. Legacy areas with a conversion to m61 use the DLC
world-map view; this is not an interior room layout for those large dungeons.

It draws locally generated AI walkable surfaces and their edges,
tracks the player's block-local XYZ position, and selects a height band around
the supporting walkable surface. Navigation partitions are not verified room labels
and the gold edges are not guaranteed physical walls. Doors, lifts, traps and
other changing geometry are not represented. Vanilla layout only.

The normal outdoor minimap remains available. Dungeon trail, world-map points,
graces and beacons are omitted from this pilot view because they use another
coordinate system. The first test must establish alignment before adding icons.

Pilot 2 removes the dark internal triangle seams. The current height band is gold;
nearby upper paths are dim blue and lower paths are dim brown (within 8 m vertically).
Double chevrons point along detected slopes toward higher/lower endpoints, with
signed height differences in metres relative to the selected floor. These are
geometric hints, not verified stair, door or route labels. Crowded hints are omitted.
Floor selection prefers support below the feet and smooths changes while walking.
The user confirmed pilot 1 displays in Tombsward; alignment, the new height display
and other dungeons' gameplay remain to be checked.

Pilot 3 selects each dungeon using the player's exact four-part block ID. A background
worker loads one map at a time; old geometry is cleared on a block change. Missing or
bad data falls back to the normal minimap behaviour and logs a data error. It does
not show another dungeon's geometry while loading. Location captions use catalog
names, shortened to fit the frame when necessary. Without `dungeons/maps.tsv`, the
DLL still supports the older single `tombsward.nav` layout.

To update an existing working pilot folder, exit the game, replace
`tombsward-minimap-test.dll`, and copy the complete `dungeons` directory next to it.
Keep the existing `minimap.ini`, ME3 profile and save. The data format, INI keys and
test save name are unchanged. Replacing the DLL alone retains Tombsward-only coverage.

## Start the isolated test

Requires installed ME3 and Elden Ring executable version 2.7.1.0 (app 1.17.1).
The inherited version gate disables the DLL on other executable versions.

1. Keep all files of the local test package together.
2. Open `dungeon-test.me3` with ME3, or use:
   `me3 launch --profile "C:\full\path\to\dungeon-test.me3"`
3. This profile starts offline, enables only this DLL, and uses the separate save
   `ER0000_MINIMAP_TOMBSWARD_TEST.mod`. On its first launch ME3 copies the normal
   game save; it does not inherit an existing overhaul/mod profile's save.
   Existing game/mod profiles are not edited by this package.
4. Visit a catalog dungeon, for example Impaler's Catacombs, Groveside Cave,
   Limgrave Tunnels, Fog Rift Catacombs, Belurat Gaol or Dragon's Pit.
   The map changes automatically. DLC files do not grant access to DLC areas.

Controls: F9 show/hide; F10 cycles 15/30/60 m radius in supported dungeons; F11 switches
layered/current-level-only/all-levels view. Layered is the startup default.
Other locations retain the original zoom distances.
The package enables `dungeon_mode = true`, size 1.5 and a 2 m height band.
Settings are read from `minimap.ini` next to the DLL at game startup.
To disable the experimental mode set `dungeon_mode = false` and restart.

## Gameplay checks still required

- In the DLC, check Gravesite Plain, Scadu Altus and a southern area. Compare the
  arrow, road direction, zoom and discovered graces with the game's map. Place a
  numbered beacon nearby and verify it belongs to the DLC map. Warp between base
  game and DLC areas; check that tiles, trail and markers change worlds correctly.
- At each dungeon's entrance and Site of Grace, check whether the arrow stays on the actual corridor.
- Warp between two supported dungeons, then outdoors and back. Check the location
  caption and that the old dungeon does not appear during loading.
- Walk both directions along stairs and around corners; check direction and scale.
- Check that the bright floor follows stairs smoothly, jumping does not highlight
  an overhead floor, and chevrons/height labels agree with the path's actual rise.
- Cross rooms, lower passages and boss rooms. Compare F11 views
  at vertically overlapping paths. Check for wrong-floor selection or missing paths.
- Rotate the camera; verify north-up mode with `rotate = false` after a restart.
- Check F9/F10/F11, world map/menu/loading hiding, then warp outdoors and return.
- Check that surfaces and gold edges stay inside the minimap disc.
- If geometry cannot load, `minimap-test.log` records the data error and the normal
  minimap remains active. No gameplay success is implied by successful data loading.

## QuestPath startup regression

The local combined profile closed within about 7 seconds with QuestPath's
`ingame` renderer and the minimap enabled. Removing either mod allowed startup.
Windows reported stack overflow failures, often inside Steam's overlay module.
This narrows the problem to interacting graphics hooks, but does not identify
which component creates the recursive call chain.

QuestPath's own INI documents a supported alternative for crashes or freezes:

```ini
[overlay_render]
renderer = window
```

Change this setting in `QuestPath.ini`, next to `QuestPath.dll`, then restart.
It draws QuestPath in a separate transparent window. The local game already used
borderless mode; exclusive fullscreen, HDR and other display setups are untested.
This leaves both mods enabled and does not require a new minimap DLL.

With the original pilot 4 DLL and this one setting changed, the full original
profile passed a 60-second startup check. The check found the actual Elden Ring
window and confirmed it responded, separately from QuestPath's overlay window.
Character loading, visible overlays, alignment and long sessions remain untested.
An attempted minimap hook change passed once but failed on retest and was discarded.

Geometry/unit tests cannot reproduce this interaction: it needs a real DirectX 12
game process, QuestPath and the usual overlays. Use a test profile for the check:

```powershell
pwsh -NoProfile -File .\tools\check-startup.ps1 `
  -ProfilePath 'C:\path\to\test.me3' `
  -GameExe 'C:\path\to\ELDEN RING\Game\eldenring.exe' `
  -Seconds 60
```

The check records startup results under `dist/startup-check`, exits nonzero if
the game closes or stays unresponsive, and closes only the game process it starts.
The game can update the profile's save during startup. Loading a character and
checking visible overlays still require a separate gameplay test.

## Build and data

Run `cargo build --release --locked` and `cargo test --release --lib --locked`.
`tools/build-pilot.ps1 -NavigationRoot PATH` performs both and makes the package.
Add `-SmallDungeons` for this 60-map package; without it the builder retains the
single-map pilot. The multi-map build also enables real-data catalog/loader tests.
Add `-WorldMapGameRoot GAME_DIRECTORY` to decode and verify every selected M10
cell from the local game, plus real asynchronous DLC/base/DLC window switching.
Without that argument this external-data test is explicitly skipped. Set
`MINIMAP_WORLD_PREVIEW_DIR` to write an offline atlas PNG during this test.
Optional arguments select isolated Cargo/Rustup/GCC/build directories; `-Offline`
uses downloaded dependencies. GNU Windows builds need an ASCII tool/build path.
The expanded suite checks all 60 data files and renders each at three sampled
heights, in three views, at two zoom levels (1,080 offline frames). It also checks
background loading, rapid block changes and the original Tombsward cases.
To capture offline frames, set `MINIMAP_PREVIEW_DIR` before
the test run, then use `tools/render-preview.py CAPTURE_DIRECTORY OUTPUT.png`
(optional NumPy and Pillow). The preview draws a simple frame/arrow around actual
ImGui floor geometry; it is not an in-game screenshot.

`tools/prepare_dungeon.py NAVIGATION_ROOT OUTPUT_DIRECTORY --small-dungeons` reads the earlier
local SoulsFormats/HKLib exports, validates the NVA placements and face counts,
triangulates the polygons, and omits links between sections from the boundary
lines. It writes `dungeons/*.nav`, a map catalog and a source-hash receipt, plus
`tombsward.nav` for compatibility with the earlier pilot. The DLL has no embedded game
geometry. Keep generated data and test archives local; a public source fork
should contain the generator, code and documentation, with users generating data
from their own install. This pilot does not yet include an end-user archive reader
for generating navmesh data without Smithbox.

Automated geometry/data tests cannot verify live game alignment, GPU hook
compatibility, actual frame rate or player-only traversal coverage. Those require
the gameplay checks above.
