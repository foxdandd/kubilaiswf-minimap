# Tombsward Catacombs pilot 2

Local test build based on kubilaiswf's MIT-licensed Minimap, upstream commit
ff2b9e4ce7b71ddcb503acd202081953183173aa. Keep the original copyright and LICENSE.

## What to test

This adds an optional navigation-layout view only inside Tombsward Catacombs,
map m30_00_00_00. It draws locally generated AI walkable surfaces and their edges,
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
The user confirmed pilot 1 displays in game; alignment and pilot 2 gameplay remain
to be checked.

To update an existing working pilot 1 folder, exit the game and replace only
`tombsward-minimap-test.dll`. Its data format, INI keys and test save name are unchanged.

## Start the isolated test

Requires installed ME3 and Elden Ring executable version 2.7.1.0 (app 1.17.1).
The inherited version gate disables the DLL on other executable versions.

1. Keep all files of the local test package together.
2. Open `tombsward-test.me3` with ME3, or use:
   `me3 launch --profile "C:\full\path\to\tombsward-test.me3"`
3. This profile starts offline, enables only this DLL, and uses the separate save
   `ER0000_MINIMAP_TOMBSWARD_TEST.mod`. On its first launch ME3 copies the normal
   game save; it does not inherit an existing overhaul/mod profile's save.
   Existing game/mod profiles are not edited by this package.
4. Visit Tombsward Catacombs in the Weeping Peninsula. Other caves are not pilot maps.

Controls: F9 show/hide; F10 cycles 15/30/60 m radius in Tombsward; F11 switches
layered/current-level-only/all-levels view. Layered is the startup default.
Other locations retain the original zoom distances.
The package enables `dungeon_mode = true`, size 1.5 and a 2 m height band.
Settings are read from `minimap.ini` next to the DLL at game startup.
To disable the experimental mode set `dungeon_mode = false` and restart.

## Gameplay checks still required

- At the entrance and Site of Grace, check whether the arrow stays on the actual corridor.
- Walk both directions along stairs and around corners; check direction and scale.
- Check that the bright floor follows stairs smoothly, jumping does not highlight
  an overhead floor, and chevrons/height labels agree with the path's actual rise.
- Cross the main room, the lower passages and the boss room. Compare F11 views
  at vertically overlapping paths. Check for wrong-floor selection or missing paths.
- Rotate the camera; verify north-up mode with `rotate = false` after a restart.
- Check F9/F10/F11, world map/menu/loading hiding, then warp outdoors and return.
- Check that surfaces and gold edges stay inside the minimap disc.
- If geometry cannot load, `minimap-test.log` records the data error and the normal
  minimap remains active. No gameplay success is implied by successful data loading.

## Build and data

Run `cargo build --release --locked` and `cargo test --release --lib --locked`.
`tools/build-pilot.ps1 -NavigationRoot PATH` performs both and makes the package.
Optional arguments select isolated Cargo/Rustup/GCC/build directories; `-Offline`
uses downloaded dependencies. GNU Windows builds need an ASCII tool/build path.
The current suite includes 12 tests, with real-data draw checks at three positions
in all three views. To capture these offline frames, set `MINIMAP_PREVIEW_DIR` before
the test run, then use `tools/render-preview.py CAPTURE_DIRECTORY OUTPUT.png`
(optional NumPy and Pillow). The preview draws a simple frame/arrow around actual
ImGui floor geometry; it is not an in-game screenshot.

`tools/prepare_dungeon.py NAVIGATION_ROOT OUTPUT_DIRECTORY` reads the earlier
local SoulsFormats/HKLib exports, validates the NVA placements and face counts,
triangulates the polygons, and omits links between sections from the boundary
lines. It writes `tombsward.nav` and a data receipt. The DLL has no embedded game
geometry. Keep generated data and test archives local; a public source fork
should contain the generator, code and documentation, with users generating data
from their own install. This pilot does not yet include an end-user archive reader
for generating navmesh data without Smithbox.

Automated geometry/data tests cannot verify live game alignment, GPU hook
compatibility, actual frame rate or player-only traversal coverage. Those require
the gameplay checks above.
