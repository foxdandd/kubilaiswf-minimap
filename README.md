# kubilaiswf's Minimap

<p align="center">
  <a href="https://github.com/kubilaiswf/kubilaiswf-minimap/releases/latest"><img src="https://img.shields.io/github/v/release/kubilaiswf/kubilaiswf-minimap?label=release" alt="Latest release"></a>
  <a href="https://github.com/kubilaiswf/kubilaiswf-minimap/releases"><img src="https://img.shields.io/github/downloads/kubilaiswf/kubilaiswf-minimap/total" alt="Downloads"></a>
  <a href="https://github.com/kubilaiswf/kubilaiswf-minimap/actions/workflows/release.yml"><img src="https://img.shields.io/github/actions/workflow/status/kubilaiswf/kubilaiswf-minimap/release.yml?branch=main" alt="Build"></a>
  <a href="https://www.nexusmods.com/eldenring/mods/11319"><img src="https://img.shields.io/badge/Nexus%20Mods-kubilaiswf's%20Minimap-da8e35?logo=nexusmods&logoColor=white" alt="Nexus Mods"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License"></a>
</p>

A minimap for Elden Ring that looks like it belongs in the game. It draws the game's own world map in the top right corner, turns with your camera and shows graces, beacons and map markers around you.

**Download:** [Nexus Mods](https://www.nexusmods.com/eldenring/mods/11319) or [Releases](https://github.com/kubilaiswf/kubilaiswf-minimap/releases/latest) · **Loader:** [me3](https://github.com/garyttierney/me3) · **Bugs and ideas:** [Issues](https://github.com/kubilaiswf/kubilaiswf-minimap/issues)

> **Offline only.** Like every me3 mod, don't take it online.

## Features

**The map**
- The game's own world map, read from your install while the game runs. No game files are shipped.
- Fully revealed, in the overworld, inside legacy dungeons and in the underground.
- Turns with the camera, or stays north up. Your arrow smoothly follows where you face.
- Three zoom levels: 40, 80 and 160 m.

**On the map**
- Graces you've discovered and your numbered beacons. Beacons out of range sit on the rim, pointing the way.
- World map points, with defeated bosses dimmed.
- [Map for Goblins](https://www.nexusmods.com/eldenring/mods/3091) markers, which disappear once looted.
- [Map for Goblins DLL](https://www.nexusmods.com/eldenring/mods/10062) markers with its own icons. Hiding a category or marker in its menu hides it on the minimap too.
- An optional trail of where you've been.

**Out of the way**
- Hides in menus, on the world map, on loading screens and in Roundtable Hold.
- Hides in caves, catacombs and tunnels, which have no map. Optionally during boss fights too.
- Size, position and keys are set in `minimap.ini`.

## Requirements

- [me3](https://github.com/garyttierney/me3) (Mod Engine 3)
- Elden Ring 1.17.1 (exe 2.7.1.0). On any other version the mod turns itself off instead of crashing the game.

## Install

1. Download the zip from [Releases](https://github.com/kubilaiswf/kubilaiswf-minimap/releases/latest) and extract the `kubilaiswf-minimap` folder anywhere.
2. Double click `kubilaiswf-minimap.me3`.

### With other me3 mods

Add the DLL to the other mod's `.me3` profile instead, for example Map for Goblins DLL next to the minimap:

```toml
profileVersion = "v1"

[[supports]]
game = "eldenring"

[[natives]]
path = 'MapForGoblins.dll'

[[natives]]
path = 'C:\path\to\kubilaiswf-minimap\kubilaiswf-minimap.dll'
```

If the profile has an empty `natives = []` line, delete it, me3 can't read a profile that has both. If that line lists other DLLs, keep it and add this one inside the brackets instead:

```toml
natives = [ { path = 'C:\some\other\mod.dll' }, { path = 'C:\path\to\kubilaiswf-minimap\kubilaiswf-minimap.dll' } ]
```

## Controls

| Key | Action |
| --- | --- |
| F7 | Show / hide |
| F8 | Zoom: 40 / 80 / 160 m |

Both keys can be changed in `minimap.ini`. Before 0.4.0 they were F9 and F10, which clash with Map for Goblins DLL.

## Settings

`minimap.ini` sits next to the DLL and is created on first launch. Restart the game after changing it.

| Setting | Default | |
| --- | --- | --- |
| `trail` | `true` | Draw the path you walked |
| `rotate` | `true` | Turn the map with the camera. `false` keeps north up |
| `compass` | `true` | N / E / S / W letters |
| `hide_in_boss_fights` | `false` | Hide while a boss health bar is on screen |
| `hide_in_caves` | `true` | Hide in catacombs, caves and tunnels |
| `size` | `1.0` | 0.5 to 2.5 |
| `margin` | `24` | Distance from the top right corner, in pixels at 1080p |
| `toggle_key` | `F7` | F1-F12, A-Z, 0-9, Insert, Delete, Home, End, PageUp, PageDown, Keypad0-Keypad9, or `none` |
| `zoom_key` | `F8` | Same as above |

Updating from an older version? Delete your `minimap.ini` once so the new settings and their comments show up.

## Troubleshooting

**The minimap doesn't show up.** Check that `minimap.ini` was created next to the DLL. If it wasn't, the DLL never loaded: check the path in your `.me3` profile. If it was, check the game version, the mod only runs on 1.17.1.

**A key does nothing, or does two things.** Another mod uses the same key. Change `toggle_key` or `zoom_key` in `minimap.ini`.

**The map is blank in some places.** Shadow of the Erdtree areas aren't supported yet, and overhauls that move areas around can put you somewhere the world map doesn't cover.

## Known limitations

- No Shadow of the Erdtree map yet, DLC areas get a plain compass.
- Markers of the original Map for Goblins are simple shapes, not the game's icons.
- Only tested with me3.

## How it works

The DLL hooks the game's DirectX 12 renderer through [hudhook](https://github.com/veeenu/hudhook) and reads the game's state on a game task, using the structures from [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs).

The map comes from the menu's world map tiles. A background thread decrypts the archive headers, reads the tiles around you from the archives, decompresses them with the game's own Oodle DLL and decodes the textures. Positions inside legacy dungeons go through the game's `WorldMapLegacyConvParam`, like the world map does. Markers are the game's `WorldMapPointParam` rows, shown by the same rules the world map uses, which is also how map mods' markers appear.

## Building

```
cargo build --release
me3 launch --profile minimap.me3
```

`minimap.me3` loads `target/release/kubilaiswf_minimap.dll`.

Releases are built by GitHub Actions: pushing a `v*` tag builds the zip and publishes it with the tag's message as release notes.

## Credits

- [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs) for the game structures
- [hudhook](https://github.com/veeenu/hudhook) for the overlay
- [soarqin/EROverlay](https://github.com/soarqin/EROverlay) for documenting the beacon and map point logic
- [Map For Goblins - DLL](https://github.com/VirusAlex/ERR-MapForGoblins-DLL) by VirusAlex for its marker icons (MIT, see `package/THIRD-PARTY-NOTICES.txt`)
- [SoulsFormats](https://github.com/soulsmods/SoulsFormatsNEXT), [UXM](https://github.com/Nordgaren/UXM-Selective-Unpack) and [Smithbox](https://github.com/vawser/Smithbox) for file format and map research

## License

MIT, see [LICENSE](LICENSE). Use it however you like, just keep the credit.
