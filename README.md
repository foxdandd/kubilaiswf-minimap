# Elden Ring Minimap — dungeons and Shadow of the Erdtree

An enhanced fork of [kubilaiswf's Minimap](https://github.com/kubilaiswf/kubilaiswf-minimap). It keeps the rotating circular map and Map for Goblins compatibility, and adds small-dungeon layouts and the Shadow of the Erdtree outdoor map.

## Available on GitHub

The `main` branch contains the **experimental pilot 10 source**, based on upstream
0.5.0, with support for 60 small-dungeon map areas, the Shadow of the Erdtree
outdoor map, below-surface terrain, local landmarks and cached outdoor drawing.
The original project's releases do not include these fork additions.

**No pilot 10 binary release has been published.** Older release downloads do not
contain all of the current source features. Game-derived map data and test saves
are not included in the repository.

Dungeon layouts load in the background. The local test used 48 base-game and
12 DLC map areas. These follow AI navigation surfaces, not verified room walls;
60 map areas does not mean 60 confirmed distinct dungeons. One unnamed forge is
identified by its map ID. Physical terrain/scenery are optional local sidecars.

## Experimental pilot 10 / upstream 0.5.0

Upstream history is merged through `5ae48a5` (v0.5.0). The fork retains its DLC,
local-map and cached-terrain extensions alongside the original runtime changes.

The source carries the runtime changes from [upstream v0.5.0](https://github.com/kubilaiswf/kubilaiswf-minimap/releases/tag/v0.5.0), commit `5ae48a5`: co-op partner arrows, Map for Goblins DLL icons, the Stormveil/world-reference conversion fix and the new F7/F8 defaults. The fork retains DLC coordinate support and accumulates height through chained dungeon conversions. Its installed test profile keeps F6/F7/F8. Local partner arrows use the dungeon scale and suppress distant floors.

Pilot 10 adds:

- Below-surface layouts from 719 base-game/DLC navigation tile variants. Auto requires broad overhead coverage and keeps nearby gaps stable. Retention on connected open ground is limited to 64 m from confirmed cover; after leaving that margin, 2 seconds of supported open ground restores Surface. This fixes the reproduced Scenic Isle lake case. F8 Local and Surface remain available for areas where the heuristic needs manual selection.
- Outdoor terrain is prepared as a cached image on a background worker. Walking and camera turns reuse it; the game draws a small texture disc instead of rebuilding the physical mesh every frame. Stale areas are rejected, zoom replaces one texture, and loading screens do not upload images. A short “Preparing local terrain” message can appear during a new patch.
- A geographic drawing layer from physical map-piece geometry, rather than navigation edges alone: parchment ground, shaded landforms, height contours and muted water. Recorded object placements supply engraved tree, rock and structure symbols. Navigation remains the source for floor selection, missing physical ground and the shallow-water mask.
- Height-aware entrances, landmarks and discovered graces. Nearby other-level markers are dimmed with up/down signs; distant levels are hidden. Outdoor player pins have a `?` badge because their height is unknown.

The private export has physical surfaces for 671 outdoor tile variants and 48 small-dungeon areas, plus scenery sidecars for all 779 supported navigation entries. Other entries retain the navigation drawing. This is a procedural cartographic rendering of available geometry, not a complete hand-painted atlas. Asset-built architecture, deep-water boundaries and unlabelled scenery can be incomplete; changing doors, lifts and traps are not tracked.

**Validation:** 55 local checks passed, including the real Scenic Isle exit replay, existing Village/DLC stability traces, terrain caching, texture reuse, loading-screen upload protection and the prior data/render checks. In the isolated Scenic Isle CPU drawing benchmark at the widest zoom, median cost fell from 30.4 ms to 0.15 ms; vertices fell from 247,900 to 10,276. Preparing that image took about 59 ms on the worker. These figures exclude GPU upload/rendering and other mods, and are not gameplay FPS. The optional exported-marker CSV test was not rerun because its private fixture was absent; synthetic marker checks passed. Character-loaded walking and multiplayer remain to test.

Original code-drawn map symbols and imported Map for Goblins DLL icons follow game visibility/discovery flags. No save flags are changed. The icon credits and licenses are retained in the package's third-party notices, copied from [upstream](https://github.com/kubilaiswf/kubilaiswf-minimap/blob/v0.5.0/package/THIRD-PARTY-NOTICES.txt).

Extracted geometry, saves, local paths and test packages stay local. An update needs the new DLL and optional `.land` / `.scenery` sidecars beside the matching navigation files. Existing INI and navigation files remain compatible. Unsupported areas use the normal map; large legacy-dungeon interiors are not covered by these generated layouts.

## Requirements

- Elden Ring 1.17.1, executable version **2.7.1.0**. The DLL disables itself on other executable versions.
- [ME3](https://github.com/garyttierney/me3), using an offline profile.
- Your own installed game files. DLC content requires Shadow of the Erdtree.
- Locally generated navigation data for dungeon layouts.

## Build the public source

```powershell
git clone https://github.com/foxdandd/kubilaiswf-minimap.git
cd kubilaiswf-minimap
cargo build --release --locked
cargo test --release --lib --locked
```

The DLL is written to `target/release/kubilaiswf_minimap.dll`. Base-game and DLC world-map textures are read from your installation. Tests needing local game data are ignored by the basic test command.

The dungeon generator requires existing SoulsFormats/HKLib navigation exports. It cannot yet create dungeon data directly from a raw game install; the earlier exporter is not included. With those exports available:

```powershell
pwsh -NoProfile -File .\tools\build-pilot.ps1 `
  -NavigationRoot 'C:\path\to\navigation-exports' `
  -OutputRoot '.\dist\Dungeon-Minimap-Test' `
  -SmallDungeons `
  -WorldMapGameRoot 'C:\path\to\ELDEN RING\Game'
```

The package contains `tombsward-minimap-test.dll`, `minimap.ini`, dungeon data and a separate `dungeon-test.me3` profile. The DLL retains its early test filename. `-WorldMapGameRoot` enables real-game DLC tile tests and is optional. GNU Windows builds need an ASCII toolchain/build path; the script accepts `-CargoHome`, `-RustupHome`, `-GccBin` and `-BuildRoot`.

See [TESTING.md](TESTING.md) for checks and data preparation. The outdoor/cartography tools are included in this source update; the first navigation/MSB export step still needs external developer tools.

## Use with an existing ME3 profile

1. Exit the game and put the built DLL in its own mod folder.
2. Keep `minimap.ini` next to the DLL. For dungeon layouts, copy the complete `dungeons` directory there and set `dungeon_mode = true`.
3. Add one native entry to your existing profile, keeping its other mods and save settings.

For a test package copied to `natives/DungeonMinimap`:

```toml
[[natives]]
path = 'natives/DungeonMinimap/tombsward-minimap-test.dll'
```

Use one `natives` format in a profile: remove an empty `natives = []` before
adding `[[natives]]`, or add the DLL inside an existing nonempty inline list.
ME3 cannot read both formats together.

For a plain Cargo build, use `kubilaiswf_minimap.dll`. Load one minimap DLL per profile and keep `start_online = false`. The separate test profile is an alternative for isolated testing; ME3 may copy the normal save on first use, so this does not automatically create a fresh character.

## Controls and settings

These are the current source defaults. Existing `minimap.ini` files keep their own key choices.

| Action | Default key |
| --- | --- |
| Show or hide the minimap | F7 |
| Change zoom | F8 |
| Change floor / outdoor view | F11 |

The existing local test installation uses **F6 / F7 / F8** instead. In pilot 10, outdoors F8 cycles **Auto / Local / Surface**. Inside supported dungeons it cycles the floor views. Updating that installation preserves its INI and navigation data.

Settings are read from `minimap.ini` beside the DLL at startup. Restart after changing them.

```ini
trail = true
rotate = true
compass = true
coop_players = true
hide_in_boss_fights = false
hide_in_caves = true
size = 1.0
margin = 24
toggle_key = F7
zoom_key = F8
dungeon_mode = false
dungeon_height_band = 2.0
dungeon_floor_key = F11
```

`rotate = false` keeps north up. `size` accepts 0.5–2.5; `margin` is in pixels at 1080p. The dungeon height band accepts 0.5–5.0 metres. Outdoor zoom radii are 40 / 80 / 160 m; dungeon radii are 15 / 30 / 60 m. The source also supports `parchment_maps` and `outdoor_layers`, both enabled by default when absent from an older INI.

## Known limits and startup compatibility

Navigation data follows vanilla layouts; overhaul mods may change them. AI walkable surfaces can differ from paths a player can reach. World-map terrain is fully revealed on the minimap; this does not grant map fragments or change discovery flags. [Map for Goblins](https://www.nexusmods.com/eldenring/mods/3091) points retain their game's visibility rules.

If startup closes or freezes with QuestPath enabled, set `renderer = window` under `[overlay_render]` in its INI and restart. This supported separate-window renderer passed a previous 60-second combined-profile game-window check. It is a local compatibility workaround; character loading and overlay appearance still require gameplay testing.

## Preparing a public mod download

Start with an experimental pre-release after character-loaded checks at Albinauric
Village, the Scenic Isle lake and representative DLC lower areas. Test switching,
warps, zoom, installation and frame pacing on a clean profile. CPU drawing
benchmarks above do not establish gameplay FPS.

The private five-file pilot 10 update is an upgrade for an existing installation;
it does not include the navigation data needed by a first-time user. Before a
full dungeon/terrain release, provide a usable local data-generation workflow or
confirm redistribution permissions for the extracted data. The current tools
still require external navigation/MSB exports. Keep game-derived files private
while those permissions are unresolved.

A public package should include the DLL, a generic INI/profile, installation and
uninstall instructions, supported game version, known issues, LICENSE,
THIRD-PARTY-NOTICES.txt and checksums. Exclude saves, account information, logs,
local configuration and original game archives. Include the source link and credit
kubilaiswf and the icon authors. The parchment material was AI-generated; code was
developed with AI assistance. Apply the required Nexus AI/content tags according
to its current [file submission guidelines](https://help.nexusmods.com/article/28-file-submission-guidelines).

The configured GitHub build ZIP contains the DLL and generic configuration, without generated
map sidecars. It supports the original surface-map functionality; custom local
layouts need the separately generated data. With GitHub Actions enabled, pushes to `main` build this artifact;
a `v*` tag also triggers the existing public release workflow. Create a release
tag only when its contents and description are ready for public download.

## Credits and license

- [kubilaiswf](https://github.com/kubilaiswf/kubilaiswf-minimap) — original minimap, artwork and implementation.
- [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs) — game structures.
- [hudhook](https://github.com/veeenu/hudhook) — overlay rendering.
- [Map for Goblins DLL](https://github.com/VirusAlex/ERR-MapForGoblins-DLL) — imported icon atlas; see its retained notices.
- [soarqin/EROverlay](https://github.com/soarqin/EROverlay) — beacon and map-point research.
- [SoulsFormats](https://github.com/soulsmods/SoulsFormatsNEXT), [UXM](https://github.com/Nordgaren/UXM-Selective-Unpack) and [Smithbox](https://github.com/vawser/Smithbox) — file formats and map research.

MIT, inherited from the original project. Keep the copyright notice, [LICENSE](LICENSE)
and all [third-party notices](package/THIRD-PARTY-NOTICES.txt).
