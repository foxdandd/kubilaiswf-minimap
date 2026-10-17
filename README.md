# kubilaiswf's Minimap - Rotating Map Overlay (Map for Goblins Compatible) + Dungeon Layouts & Shadow of the Erdtree Map

An enhanced fork of [kubilaiswf's Minimap](https://github.com/kubilaiswf/kubilaiswf-minimap). It keeps the original rotating map overlay and Map for Goblins compatibility, and adds navigation layouts for **60 small-dungeon map areas**, floor height cues, and the **Shadow of the Erdtree outdoor map**.

The dungeon and DLC map additions are carried on `main`. The below-surface outdoor
pilot is developed on `codex/tombsward-pilot`. The original project's releases do
not include these additions.

## What this fork adds

| Feature | Current coverage |
| --- | --- |
| Small-dungeon layouts | 48 base-game and 12 DLC map areas, loaded automatically when you enter them |
| Floor height display | Gold for the current level, dim blue for paths above, dim brown for paths below |
| Slope hints | Direction arrows and signed height differences, such as `+5 m` or `-3 m` |
| Shadow of the Erdtree | Outdoor terrain read from the installed game, with DLC graces, map points and numbered beacons |
| Below-surface outdoor paths | Optional height-aware layouts for 719 base/DLC tile variants, generated locally |

Dungeon maps load in the background. The display clears the old layout when you change areas, smooths floor selection on stairs, and reduces overlapping labels.

The original circular frame, camera rotation, outdoor trail, discovered graces and map points remain available. World-map terrain is fully revealed on the minimap; this does not grant map fragments or change discovery flags in your save.

The dungeon layouts come from AI navigation surfaces. They show walkable geometry, **not verified room boundaries or physical walls**. The count is 60 map areas, not 60 confirmed distinct dungeons; one forge area has no name and is shown by its map ID.

## Requirements

- Elden Ring 1.17.1, executable version **2.7.1.0**. The DLL disables itself on other executable versions.
- [ME3](https://github.com/garyttierney/me3), using an offline profile.
- Your own installed game files. DLC access requires Shadow of the Erdtree.
- Locally generated navigation data for the optional dungeon layouts.

## Build from source

```powershell
git clone https://github.com/foxdandd/kubilaiswf-minimap.git
cd kubilaiswf-minimap
cargo build --release --locked
cargo test --release --lib --locked
```

The DLL is written to `target/release/kubilaiswf_minimap.dll`. Base-game and DLC world-map textures are read directly from your install; they need no separate map download. Tests that require local game data are ignored by the basic test command.

### Optional dungeon test package

The dungeon generator currently requires existing SoulsFormats/HKLib navigation exports. It cannot yet create the dungeon data directly from a raw game install, and the earlier navigation exporter is not included in this fork.

With those exports available, use Python and PowerShell to build the test package:

```powershell
pwsh -NoProfile -File .\tools\build-pilot.ps1 `
  -NavigationRoot 'C:\path\to\navigation-exports' `
  -OutputRoot '.\dist\Dungeon-Minimap-Test' `
  -SmallDungeons `
  -WorldMapGameRoot 'C:\path\to\ELDEN RING\Game'
```

The package contains `tombsward-minimap-test.dll`, `minimap.ini`, the `dungeons` directory and an isolated `dungeon-test.me3` profile. The DLL keeps its earlier test filename even though it now supports more areas.

`-WorldMapGameRoot` enables the real-game DLC tile tests. It is optional when building. GNU Windows builds need an ASCII toolchain/build path; the script accepts `-CargoHome`, `-RustupHome`, `-GccBin` and `-BuildRoot` for this setup.

See [TESTING.md](TESTING.md) for data preparation, build options and gameplay checks. Generated navigation files and test archives are kept local and are not committed to this repository.

## Use with an existing ME3 profile

1. Exit the game and place the built DLL in its own mod folder.
2. Keep `minimap.ini` next to the DLL. For dungeon layouts, also copy the complete `dungeons` directory there and set `dungeon_mode = true`.
3. Add one native entry to your existing `.me3` profile, keeping its current mods and save settings.

For a built test package copied to `natives/DungeonMinimap`:

```toml
[[natives]]
path = 'natives/DungeonMinimap/tombsward-minimap-test.dll'
```

For a plain Cargo build, use the actual filename `kubilaiswf_minimap.dll` instead. Load one minimap DLL per profile and keep the profile offline with `start_online = false`.

The package's separate `dungeon-test.me3` is an alternative for isolated testing. It uses `ER0000_MINIMAP_TOMBSWARD_TEST.mod`; ME3 copies the normal save on first use, so this does not automatically create a fresh character.

## Controls

Pilot 5 can show lower outdoor paths hidden by the normal map artwork, including
the Albinauric village and layered DLC terrain. With generated `outdoor/` data
beside the DLL, **Auto** detects an upper walkable surface and selects the local
ground layout. Outdoors, the floor key cycles **Auto / Local / Surface**; inside
supported dungeons it retains the existing floor-view cycle. `outdoor_layers = false`
disables this feature. Missing data keeps the normal map.

Generate the outdoor files from existing navigation exports:

```powershell
python tools/prepare_outdoor.py 'C:\path\to\outdoor-exports' --output '.\dist\Dungeon-Minimap-Test'
```

Multiple export roots may be supplied, including event-variant exports. The build
script also accepts `-OutdoorNavigationRoots`. These exports are separate from the
small-dungeon exports; the earlier exporter limitation still applies. Game geometry
stays local. See [TESTING.md](TESTING.md) for coverage, missing tiles and live checks.

| Action | Default key |
| --- | --- |
| Show or hide the minimap | F9 |
| Change zoom | F10 |
| Cycle layered / current level / all levels | F11 |

Outdoor zoom uses a 40 / 80 / 160 m radius. Supported dungeon layouts use 15 / 30 / 60 m. Floor view starts in layered mode, with nearby upper and lower paths visible within 8 m vertically.

If other mods already use F9-F11, you can use this alternative in `minimap.ini`:

```ini
toggle_key = F6
zoom_key = F7
dungeon_floor_key = F8
```

## Settings

Settings are read from `minimap.ini` next to the DLL at game startup. Restart after changing them.

```ini
trail = true
rotate = true
compass = true
hide_in_boss_fights = false
hide_in_caves = true
size = 1.0
margin = 24
toggle_key = F9
zoom_key = F10
dungeon_mode = false
dungeon_height_band = 2.0
dungeon_floor_key = F11
```

- `rotate = false` keeps north up.
- `size` accepts 0.5-2.5; `margin` is measured in pixels at 1080p.
- `dungeon_mode = true` enables supported layouts when local data is present. The test package enables it by default.
- `dungeon_height_band` sets the metres above and below the selected floor to highlight in gold; accepts 0.5-5.0, default 2 m.
- `hide_in_caves` still applies when no supported dungeon layout is active.

## Test status and limits

Pilot 4 passed **21 automated tests** using local game data. This included decoding **1,681 DLC map tiles**, switching between DLC and base-game tile windows, and drawing **1,080 dungeon frames** across the 60 map areas, sampled heights, views and zoom levels.

Gameplay verification is still incomplete. DLC alignment, marker placement, floor hints and performance with other overlays need in-game checks.

Pilot 5's lower/upper outdoor layouts are checked with local navigation data.
Gold highlights the current height, blue is above, and brown is below. These are
AI walkable surfaces, not a new hand-drawn underground map. Local layouts omit
world-map icons and trail until those have height data; Surface mode restores them.

If the game closes or freezes at startup with QuestPath enabled, set `renderer = window` under `[overlay_render]` in `QuestPath.ini`, then restart. This uses QuestPath's supported separate-window renderer. The local combined profile passed a 60-second game-window response check with this setting and the unchanged pilot 4 minimap DLL. This is a configuration workaround, not a general fix for graphics-hook conflicts. See [the startup regression check](TESTING.md#questpath-startup-regression); character loading and overlay appearance still need gameplay testing.

- Detailed interior layouts for large legacy dungeons are not included. Where available, those areas use the ordinary world-map view.
- Doors, lifts, traps and other changing geometry are not tracked.
- AI navigation surfaces can differ from paths the player can reach.
- Dungeon icons and trails are not yet drawn in the detailed layout view.
- Navigation data is based on vanilla layouts; overhaul mods may change them.
- World-map icons, including [Map for Goblins](https://www.nexusmods.com/eldenring/mods/3091) points, use simple shapes.

## Credits and license

- [kubilaiswf](https://github.com/kubilaiswf/kubilaiswf-minimap) — original minimap, artwork and implementation.
- [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs) — game structures.
- [hudhook](https://github.com/veeenu/hudhook) — overlay rendering.
- [soarqin/EROverlay](https://github.com/soarqin/EROverlay) — beacon and map-point research.
- [SoulsFormats](https://github.com/soulsmods/SoulsFormatsNEXT), [UXM](https://github.com/Nordgaren/UXM-Selective-Unpack) and [Smithbox](https://github.com/vawser/Smithbox) — file formats and map research.

MIT, inherited from the original project. Keep the copyright notice and [LICENSE](LICENSE).
