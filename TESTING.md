# Dungeon and terrain minimap pilot 10 — upstream 0.5.0

Local test based on kubilaiswf's MIT-licensed Minimap, upstream v0.5.0 commit
5ae48a5522df02f896965145038cfd6c073d6f4c. Preserve LICENSE and the included
THIRD-PARTY-NOTICES.txt for Map for Goblins DLL's imported icon atlas.

## Current behavior and checks

55 checks passed with local game data. Coverage includes 719 outdoor navigation
files, 60 dungeon layouts / 1,080 frames, 1,681 DLC cells, 1,005 walking samples
(Village 550 / Ellac 181 / Rauh 274), and 33 sparse-overhead samples in the earlier
reported DLC tiles. The optional real-marker CSV test was not rerun because its
private fixture was absent; synthetic marker/coordinate checks passed.

The reported Auto error was reproduced using the real m60_36_42_00 window:
271 steps from covered ground near the plateau to the Scenic Isle lake retained
Local indefinitely. Connectivity spanned both areas. A smaller synthetic case
also failed before the fix. Auto now retains connected open ground only within
64 m of the last confirmed covered point, with the existing 48 m climb limit.
Outside that margin, 2 seconds of supported open ground restores Surface.
Nearby edge gaps, navigation holes and the existing Village/DLC walking checks
remain stable. Auto entry still needs five of nine overhead probes across 8 m.
This is a conservative geometric heuristic, not a verified atlas-area boundary.
Use F8 Local or Surface when an untested area needs manual selection.

Outdoor parchment terrain is rasterized on one background worker into a 1024 px
world-aligned image. A patch covers four times the zoom radius. Cached drawing
keeps ground shading, contours and water; scenery and game-visible markers remain
dynamic. Scenery floor bindings are prepared on the loader, and marker bindings
are cached per window. Camera turns do not rebuild terrain. Movement/zoom/floor
changes request a new patch when needed; pending patches keep a valid old image.
A new area never reuses the wrong window/region or rebuilds its mesh on the game
frame. It briefly shows "Preparing local terrain" if no valid image is ready.
Superseded work is cancelled. One GPU texture is reused across zoom/area changes;
uploads remain blocked during loading screens. Dungeon rendering is unchanged.

Measured in an isolated CPU-only drawing loop, with the same Scenic Isle fixture:

| Zoom radius | Earlier median | Cached median | Earlier vertices | Cached vertices |
| --- | --- | --- | --- | --- |
| 40 m | 2.847 ms | 0.015 ms | 17,066 | 1,018 |
| 80 m | 8.929 ms | 0.036 ms | 62,440 | 3,460 |
| 160 m | 30.415 ms | 0.146 ms | 247,900 | 10,276 |

Background image preparation took 38–59 ms for those three patches. These costs
exclude GPU upload/rendering, the complete HUD and other mods. They are not an
in-game FPS measurement. Tests limit repeated-frame geometry, check cache reuse
through movement/rotation, latest-request selection, region/window/height guards,
texture replacement and the loading-screen upload guard. Physical ground remains
available beyond AI navigation.

The source carries upstream 0.5.0's co-op arrows, native Map for Goblins DLL icons,
Stormveil conversion fix and new default keys. Direct world references take
priority over links to other dungeons; chains preserve Y offsets and DLC space.
Local co-op arrows use local coordinates/range and omit distant floors. Actual
multiplayer operation has not been tested. Existing installed F6/F7/F8 settings
are preserved; fresh source defaults are F7/F8/F11. Missing coop_players defaults
true; set false and restart to hide partners.

## Terrain drawing and private data

The new cartographic layer reads physical map-piece triangles, including water,
in addition to the AI navigation. Ground extends beyond AI paths where physical
geometry is available. Fixed-world shading and contours describe landforms;
recorded MSB objects draw small trees, rocks and structure symbols. Fine navigation
partitions and tiny disconnected patches are suppressed. Water planes are masked
by nearby matching-height navigation, preventing whole engine tiles from appearing
as rectangular lakes. Missing physical room floors/bridges retain validated path
ground. The renderer still describes geometry, not verified physical walls/routes.

Generated locally: 779 .land files (671 outdoor / 48 dungeon entries have physical
surfaces; remaining entries fall back), 10,832,807 terrain faces including 313,058
water faces, and 779 .scenery files with 17,643 classified placements. Aliases are
conservative: unknown objects are skipped. Asset-built architecture and deep-water
extent can be incomplete. No original geographic artwork is embedded in the DLL.
All 779 sidecar pairs decode with their own block identity. Invalid/missing optional
sidecars leave the navigation layout usable.

The pilot 10 update requires an existing navigation installation. Over pilot 9,
replace only the DLL and these notes; existing .land/.scenery files work unchanged.
Earlier installations may copy matching pilot 9 sidecars into their existing
dungeons/ and outdoor/ directories. Retain the INI, .nav files and profiles. Include the notes, LICENSE and
THIRD-PARTY-NOTICES.txt. The new DLL also works without the optional sidecars.

To reproduce from your own navigation/MSB exports, prepare scenery with
`tools/prepare_scenery.py` and physical map pieces with `tools/export-cartography.ps1`.
Both accept an existing navigation root and a separate output root for an update.
The exporter needs explicit GameRoot and SmithboxRoot; it reads original archives,
supports single-axis placement rotations and reports skipped/failed inputs.
`tools/build-pilot.ps1 -CartographySmithboxRoot ... -WorldMapGameRoot ...` runs both
exporters using its dungeon/outdoor export roots and copies the third-party notice.
These game-derived sidecars and fixture CSVs are private and ignored by Git.

Current previews use the real renderer with offline fixtures, not gameplay captures.
Tests cover physical ground beyond navigation, material anchoring, icon UVs, marker
coordinates/layers, co-op dungeon scaling, finite/clipped drawing and old INI keys.
Character-loaded alignment, the user's exact village/DLC route, multiplayer and
combined-profile gameplay performance remain unverified. Check those in game.

## Earlier pilot notes

The following records earlier builds. Current Auto rules and update requirements
above take precedence over the older DLL-only update instructions.

Pilot 8 replaces blue upper-floor fills with warm parchment relief: lighter
raised edges and a small offset shadow above, softer dark shading below. Current
ground stays bright. The selected connected outdoor terrain and Auto stability
rules from pilot 7 remain in use. Dungeon current/all-level controls still apply.

Local layouts now show game-visible map points and discovered graces. Retained
raw XYZ and block identity place indoor points in their own dungeon; outdoor
points use world XZ and the game's Y reference conversion, including DLC dungeons.
Nearby markers on other levels are dimmed with an up/down sign. Distant levels
are omitted, so Moonlight Altar icons do not appear as village-floor destinations.
The selected connected outdoor sheet keeps its own hillside landmarks current.
Markers inside buildings can attach to matching-height navigation within 12 m.

Entrance arches distinguish caves/catacombs, tunnels, gaols and forges. Landmark
families include churches, villages, towers, ruins, trees, bridges and crosses.
These are original code-drawn category symbols inspired by the map style, rather
than bundled copies of the game's icon atlas. DLC icon IDs 200-299 are treated as
map landmarks; 230-232 and 234 are entrances. Original map visibility/text flags
and grace discovery flags still control which points are supplied in live play.
Cleared points stay dimmed. Markers are decluttered and entrances take priority
over overlapping symbols; up to 24 can be drawn within the circular frame.

Outdoor player pins are restored as hollow numbered markers with a ? badge:
the world-map save has no height for these pins, so their floor is unknown.
Dungeon layouts omit world-map pins because their coordinate space is different.
The vanilla data has no outdoor conversion for one unused DLC forge block;
its marker is not assigned a guessed outdoor position. Normal surface-map points
also use the new landmark/entrance symbol families. No save flags are changed.
Update an existing pilot by replacing the DLL; keep the INI and navigation files.
The private pilot 8 update contains the DLL, notes, license and checksums.

Pilot 7 fixes outdoor area consistency. The selected connected 3D terrain sheet
is drawn as one current layer, including its hillsides. It is no longer cut away
each time the player's supporting height moves outside a narrow 6 m band.
Other nearby sheets still use height context. Terrain connectivity is geometric;
it is not a verified route graph or a claim that every surface can be traversed.
Grouping is built on the background loader, with 5 cm vertex welding across tiles.

Auto retains the local view across missing support and short overhead gaps.
While already below a surface, nearby overhead within 12 m can retain the area;
it cannot trigger entry. Returning to the surface requires 2 s of supported open
ground. Large position/elevation jumps clear the latch. A failed neighbouring
tile refresh keeps an existing covering window, and a successful refresh remaps
the selected terrain using the last supported position. Explicit Surface mode
still takes effect immediately through the existing floor key.

The outdoor caption is now stable: "Below-surface terrain" or "Local terrain".
The changing upper-surface gap and signed outdoor slope arrows are omitted because
they suggested floor changes on ordinary hillsides. Fixed slope contours remain;
indoor floor arrows and controls are unchanged. The data format and existing INI
remain compatible. Updating an installed pilot only needs the new DLL.
The private pilot 7 update folder contains the DLL, notes, license and checksums;
it requires existing navigation data for a fresh installation.

Pilot 6 repaints generated dungeon and below-surface outdoor layouts in a style
inspired by the original map: aged parchment, fine sepia navigation edges and
subtle 4 m contours on actual slopes. Current ground uses warm paper; nearby
upper/lower levels retain blue/brown cues and the signed slope arrows. The paper
is anchored to world coordinates so walking, turning and zooming do not slide it
across the paths. One embedded material is uploaded at startup and reused.
No game-derived geographic artwork or new routes are baked into the texture.
The original surface map is unchanged. The generated layouts still describe AI
walkable surfaces, not hand-surveyed rooms or a complete painted terrain atlas.

`parchment_maps = true` is the default even when missing from an older INI.
Set it to false and restart to restore pilot 5's flat style. Texture-upload failure
also falls back to that style. Updating an existing pilot 5 installation only
requires the new DLL; its navigation files and controls stay compatible.
The private pilot 6 update folder contains the DLL and these notes, not the data
needed for a fresh installation. Keep the existing pilot 5 data beside the DLL.

The original paper material was made with the built-in image-generation tool.
Its reproducible prompt and provenance are in `assets/cartography-paper.prompt.txt`;
the material is `assets/cartography-paper.png`. Actual geometry remains generated
from the local game data. Offline previews use the renderer's real draw commands,
including separate paper/font textures and wrapped paper sampling.

Pilot 5 adds local outdoor navigation layouts for 719 base-game/DLC tile variants
(447 base-game, 272 DLC; 47 event variants are included). These counts describe tiles,
not distinct named areas. Geometry is generated from the user's own game files.
At overlapping walkable levels, Auto switches from the flat map image to a layout
around the player's current height. The Albinauric village, Ellac River area and
Rauh Base have real lower/upper overlap samples in the offline tests.

Outdoors, the floor key cycles **Auto -> Local -> Surface**. Source defaults use
F11; an existing installation may use another key. Local forces the height-aware
view when supporting navigation exists; Surface restores the normal map artwork.
Gold highlights surfaces within 6 m of the supporting height; nearby upper/lower
context extends 40 m, with slope arrows and signed height differences. A caption
shows the nearest upper walkable level's height gap, not measured ceiling clearance.
Auto requires an upper walkable surface more than 12 m above and delays changes
by 0.35 s to reduce flicker. Small overhangs or paths absent from AI navigation
may need Local mode, or may have no usable layout.

Background loading joins a 3x3 tile window. Spatial indexing bounds per-frame height
queries; neighbouring windows overlap during walking. Warps between worlds or event
variants clear incompatible geometry. Missing data falls back to the ordinary map.
Original dungeon layouts and the world-map artwork remain available.

The package also retains optional navigation layouts for 60 small dungeon map blocks:
48 base-game catacombs, caves and tunnels (areas m30/m31/m32), plus 12 DLC
catacomb, gaol, forge and cave blocks (m40/m41/m42/m43). The catalog includes
one unnamed forge block, shown by its map ID. `dungeons/maps.tsv` lists the exact
IDs and available names; the count is map blocks, not verified distinct dungeons.
Detailed large legacy-dungeon and overhaul layouts remain outside this build.

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
For the outdoor feature also copy the complete `outdoor` directory. Keep
`outdoor_layers = true` (the default, including when absent from an older INI).
Keep the existing `minimap.ini`, ME3 profile and save. The data format, INI keys and
test save name are unchanged. A fresh install needs the generated data; updating
an existing install retains whatever navigation coverage is already present.

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

- Visit the Albinauric village below Moonlight Altar, Ellac River and Rauh Base.
  Check Auto's lower layout, position, rotation, stairs and height transitions.
  Try Local and Surface with the floor key; returning to Auto takes up to 0.35 s.
- Walk across outdoor tile edges, jump, use lifts, then warp between base/DLC and
  a dungeon. Check for stale geometry and excessive frame-time cost. Live gameplay
  alignment and combined-overlay performance are not established by offline draws.
- Outdoor local layouts omit world-map icons and the flat trail because those
  currently lack height information. They return in Surface mode.
- The scan included 897 tile variants: 155 were empty and 23 NVA placements could
  not be decoded by the installed reader. Those tiles have no generated layout.
  Unavailable AI paths, moving geometry and overhaul changes remain limitations.

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

Pilot 8 adds checks for coordinate-space separation, missing/nonfinite marker
heights, base/DLC entrance icon grouping, marker layers on connected hillsides,
far-plateau exclusion, duplicate graces, unknown pin height and symbol clipping.
An optional local fixture uses 894 vanilla map/grace parameter rows with XYZ and
nearest legacy reference conversions, including 11 projectable DLC small-dungeon
entrance rows. It checks and previews Village, Fog Rift entrance and Tombsward
placements. Offline examples include placement samples regardless of discovery;
the live mod continues to use game visibility flags. Character-loaded placement,
discovery changes and icon readability still require a gameplay check.

Set `MINIMAP_MARKER_TEST_CSV` only for that optional marker fixture test. CSV
columns are param,id,area,gx,gz,x,y,z,icon,worldArea,worldX,worldY,worldZ. Data comes
from WorldMapPointParam/BonfireWarpParam and WorldMapLegacyConvParam in the user's
own regulation. It is not committed. The build script skips this test when the
CSV or outdoor navigation is absent. Use `--features` with render-preview.py for
the mixed shadow/symbol/real-placement preview sheet.

Pilot 7 passed all 34 tests with local fixtures, retaining the 60-dungeon/1,080-frame
and 1,681-DLC-map-cell checks. Regression checks reproduce the old immediate surface flashes on missing
support and sparse overhead. A drawing comparison reproduces the moving-height-band
artifact and verifies both uphill/downhill positions keep their connected paths
current. Other checks cover nearby overhead, genuine exits, jumps, retained failed
refreshes, ignored stale results and terrain remapping across changed region indices.
Real-data walking replays follow 864 supported samples across Albinauric Village,
Ellac River and Rauh with periodic terrain rebinding and no region/mode flips in
the covered portions. These are offline trajectories, not a captured gameplay walk.

Pilot 6 passed all 28 tests with the same local data, including all 60 dungeons
at 1,080 sampled frames, lower/upper outdoor layouts at three zooms, and 1,681
DLC map cells. New checks cover world-anchored material coordinates, joined slope
contours, textured triangle seams and bounded texture draw-command batching.
The dense outdoor sample uses softer ink so partition overlap does not black out
small paths. The 18 outdoor frames averaged about 7.6 ms of CPU geometry work
on the development host; this is not a live frame-rate measurement.
Use `--painted` on the preview script for the mixed dungeon/below-surface sheet.

Pilot 5 passed 26 tests with local game data: all 719 outdoor files decoded;
spatial height queries matched full-mesh queries; lower/upper samples rendered at
three zooms; world/variant changes and manual modes were checked. The earlier
60-dungeon/1080-frame and 1681-DLC-tile checks still pass. These are offline checks,
not a loaded-character gameplay result. Use `-OutdoorNavigationRoots` with the
build script to include the outdoor data and these real-data tests.

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
