# Small dungeon minimap pilot 3

Local test build based on kubilaiswf's MIT-licensed Minimap, upstream commit
ff2b9e4ce7b71ddcb503acd202081953183173aa. Keep the original copyright and LICENSE.

## What to test

This adds an optional navigation-layout view for 60 small dungeon map blocks:
48 base-game catacombs, caves and tunnels (areas m30/m31/m32), plus 12 DLC
catacomb, gaol, forge and cave blocks (m40/m41/m42/m43). The catalog includes
one unnamed forge block, shown by its map ID. `dungeons/maps.tsv` lists the exact
IDs and available names; the count is map blocks, not verified distinct dungeons.
Large legacy dungeons, overworld areas and overhaul layouts are outside this build.

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

## Build and data

Run `cargo build --release --locked` and `cargo test --release --lib --locked`.
`tools/build-pilot.ps1 -NavigationRoot PATH` performs both and makes the package.
Add `-SmallDungeons` for this 60-map package; without it the builder retains the
single-map pilot. The multi-map build also enables real-data catalog/loader tests.
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
