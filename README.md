# Elden Ring Minimap — dungeons and Shadow of the Erdtree

An enhanced fork of [kubilaiswf's Minimap](https://github.com/kubilaiswf/kubilaiswf-minimap). It keeps the rotating circular map and Map for Goblins compatibility, and adds small-dungeon layouts and the Shadow of the Erdtree outdoor map.

## Available on GitHub

The public source on `main` includes layouts for **60 small-dungeon map areas** (48 base-game and 12 DLC areas), floor-height colours and slope hints, and the DLC outdoor world map with graces, map points and numbered beacons. The original project's releases do not include these additions.

Dungeon layouts load in the background. They come from AI navigation surfaces: they describe walkable geometry, not verified room boundaries or physical walls. The count is 60 map areas, not 60 confirmed distinct dungeons. One unnamed forge is identified by its map ID.

## Latest local test: pilot 8

**Pilot 8 is a local test build. Its latest source and packaged data have not been published to GitHub.** The public development branch also predates these changes. Cloning this repository currently gives the public baseline described above.

The local test adds:

- Below-surface outdoor layouts from 719 locally generated base-game/DLC navigation tile variants, including terrain hidden below the ordinary map artwork.
- Stable connected terrain around Albinauric Village and layered DLC areas. A continuous hillside stays one current area; Auto holds the local view through short support or overhead gaps to reduce surface-map flashes.
- Original-map-inspired parchment shading: bright current ground, lighter raised edges and a small shadow for upper areas, soft dark shading for lower areas. Outdoor captions stay stable while walking on slopes.
- Height-aware entrance, landmark and discovered-grace symbols. Entrances use arches, tunnels, gaols or forge symbols; landmarks include churches, towers, villages, trees, bridges and crosses. Nearby other-level markers are dimmed with an up/down sign; distant levels are hidden.
- Outdoor player pins with a `?` badge because the world-map save has no pin height. Their floor is unknown.

Symbols are drawn in code by category; they are not bundled copies of the game's icon atlas. Game visibility and grace-discovery flags still apply, and no save flags are changed. Overlapping symbols are reduced, with entrances taking priority.

**Validation:** all 39 local automated tests passed, including all 719 outdoor files, 60 dungeon layouts, 1,080 rendered dungeon frames, 1,681 DLC map tiles, 864 walking-replay samples and marker placement fixtures for Albinauric Village, Fog Rift and Tombsward. These are offline checks. Character-loaded alignment, transitions and performance with other overlays still need gameplay testing.

The private update requires the existing local navigation data. Extracted game data, saves, local paths and test packages are not uploaded here. Unsupported areas fall back to the normal map. Large legacy-dungeon interiors, changing doors/lifts/traps and a complete hand-painted underground atlas are not included.

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

See [TESTING.md](TESTING.md) for the checks and preparation supported by the public source. Pilot 8's outdoor tools and new rendering are pending source publication.

## Use with an existing ME3 profile

1. Exit the game and put the built DLL in its own mod folder.
2. Keep `minimap.ini` next to the DLL. For dungeon layouts, copy the complete `dungeons` directory there and set `dungeon_mode = true`.
3. Add one native entry to your existing profile, keeping its other mods and save settings.

For a test package copied to `natives/DungeonMinimap`:

```toml
[[natives]]
path = 'natives/DungeonMinimap/tombsward-minimap-test.dll'
```

For a plain Cargo build, use `kubilaiswf_minimap.dll`. Load one minimap DLL per profile and keep `start_online = false`. The separate test profile is an alternative for isolated testing; ME3 may copy the normal save on first use, so this does not automatically create a fresh character.

## Controls and settings

| Action | Default key |
| --- | --- |
| Show or hide the minimap | F9 |
| Change zoom | F10 |
| Cycle layered / current / all floors | F11 |

The existing local test installation uses **F6 / F7 / F8** instead. In pilot 8, outdoors F8 cycles **Auto / Local / Surface**. Inside supported dungeons it cycles the floor views. Updating that installation preserves its INI and navigation data.

Settings are read from `minimap.ini` beside the DLL at startup. Restart after changing them.

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

`rotate = false` keeps north up. `size` accepts 0.5–2.5; `margin` is in pixels at 1080p. The dungeon height band accepts 0.5–5.0 metres. Outdoor zoom radii are 40 / 80 / 160 m; dungeon radii are 15 / 30 / 60 m. The local pilot also supports `parchment_maps` and `outdoor_layers`, both enabled by default when absent from an older INI.

## Known limits and startup compatibility

Navigation data follows vanilla layouts; overhaul mods may change them. AI walkable surfaces can differ from paths a player can reach. World-map terrain is fully revealed on the minimap; this does not grant map fragments or change discovery flags. [Map for Goblins](https://www.nexusmods.com/eldenring/mods/3091) points retain their game's visibility rules.

If startup closes or freezes with QuestPath enabled, set `renderer = window` under `[overlay_render]` in its INI and restart. This supported separate-window renderer passed a previous 60-second combined-profile game-window check. It is a local compatibility workaround; character loading and overlay appearance still require gameplay testing.

## Credits and license

- [kubilaiswf](https://github.com/kubilaiswf/kubilaiswf-minimap) — original minimap, artwork and implementation.
- [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs) — game structures.
- [hudhook](https://github.com/veeenu/hudhook) — overlay rendering.
- [soarqin/EROverlay](https://github.com/soarqin/EROverlay) — beacon and map-point research.
- [SoulsFormats](https://github.com/soulsmods/SoulsFormatsNEXT), [UXM](https://github.com/Nordgaren/UXM-Selective-Unpack) and [Smithbox](https://github.com/vawser/Smithbox) — file formats and map research.

MIT, inherited from the original project. Keep the copyright notice and [LICENSE](LICENSE).
