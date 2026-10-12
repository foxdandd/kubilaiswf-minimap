# kubilaiswf's Minimap

A minimap for Elden Ring that looks like it belongs in the game. Loaded with [me3](https://github.com/garyttierney/me3).

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

To use it with another me3 mod, add this to that mod's `.me3` file. If the file has a `natives = []` line, delete that line.

```toml
[[natives]]
path = 'C:\path\to\kubilaiswf-minimap\kubilaiswf-minimap.dll'
```

## Controls

- F9 show / hide
- F10 zoom (40 / 80 / 160 m)

## Settings

`minimap.ini` next to the dll, created on first launch. Restart the game after changing it.

```ini
trail = true
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
