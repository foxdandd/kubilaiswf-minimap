# kubilaiswf's Minimap

A minimap for Elden Ring that looks like it belongs in the game. Loaded with [me3](https://github.com/garyttierney/me3).

Experimental local development: an optional Tombsward Catacombs navigation-layout
mode is documented in [TESTING.md](TESTING.md). Generate the dungeon data locally;
it is not embedded in the DLL or committed to this source repository.

- the game's own map, read from your install at runtime (no game files are shipped)
- fully revealed, works in the overworld, legacy dungeons and underground
- rotates with the camera, the arrow follows where you face
- discovered graces, numbered beacons (1-5) and world map points
- [Map for Goblins](https://www.nexusmods.com/eldenring/mods/3091) markers show up too and disappear once looted
- optional trail of where you've been
- hides in menus, the world map and loading screens

## Requirements

- me3
- Elden Ring 1.17.1 (exe 2.7.1.0). On any other version the mod turns itself off.

## Install

1. Download the zip from [Releases](https://github.com/kubilaiswf/kubilaiswf-minimap/releases) and extract the `kubilaiswf-minimap` folder anywhere.
2. Double click `kubilaiswf-minimap.me3`.

To use it with another me3 mod, add this to that mod's `.me3` file. If the file has an empty `natives = []` line, delete that line.

```toml
[[natives]]
path = 'C:\path\to\kubilaiswf-minimap\kubilaiswf-minimap.dll'
```

If that line lists other DLLs, keep it and add this one inside the brackets instead:

```toml
natives = [ { path = 'C:\some\other\mod.dll' }, { path = 'C:\path\to\kubilaiswf-minimap\kubilaiswf-minimap.dll' } ]
```

## Controls

- F9 show / hide
- F10 zoom (40 / 80 / 160 m)

Both keys can be changed in `minimap.ini`.

## Settings

`minimap.ini` next to the dll, created on first launch. Restart the game after changing it.

```ini
trail = true                 ; path you walked
rotate = true                ; false keeps north up
compass = true               ; N / E / S / W letters
hide_in_boss_fights = false  ; hide while a boss health bar is up
hide_in_caves = true         ; hide in catacombs, caves and tunnels
size = 1.0                   ; 0.5 to 2.5
margin = 24                  ; distance from the top right corner, pixels at 1080p
toggle_key = F9              ; F1-F12, A-Z, 0-9, PageUp, Keypad0... or none
zoom_key = F10
```

## Known limitations

- no Shadow of the Erdtree map yet, DLC areas get a plain compass
- Map for Goblins icons are simple shapes, not the game's icons
- offline only

## Building

```
cargo build --release
me3 launch --profile minimap.me3
```

`minimap.me3` loads `target/release/kubilaiswf_minimap.dll`.

## Credits

- [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs) for the game structures
- [hudhook](https://github.com/veeenu/hudhook) for the overlay
- [soarqin/EROverlay](https://github.com/soarqin/EROverlay) for documenting the beacon and map point logic
- [SoulsFormats](https://github.com/soulsmods/SoulsFormatsNEXT), [UXM](https://github.com/Nordgaren/UXM-Selective-Unpack) and [Smithbox](https://github.com/vawser/Smithbox) for file format and map research

## License

MIT, see [LICENSE](LICENSE). Use it however you like, just keep the credit.
