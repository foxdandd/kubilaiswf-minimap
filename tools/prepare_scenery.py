"""Build private scenery sidecars from exported MSB placements and model aliases.

No game objects, artwork or placement data are embedded in the mod source.
Labels classify known objects only; unlabelled models and vague labels are skipped.
"""
import argparse
import json
import math
from pathlib import Path
import re
import struct


def category(label):
    label = label.lower()
    if any(word in label for word in ('dead body', 'corpse', 'painting', 'statue',
                                      'banner', 'pot', 'chair', 'table', 'flower')):
        return None
    if re.search(r'\btree\b', label) and not any(word in label for word in
            ('branch', 'root', 'trunk', 'stump', 'fallen', 'broken', 'illusory')):
        return 0
    if re.search(r'\b(boulder|rock formation|large rock|huge rock)\b', label):
        return 1
    if re.search(r'\b(house|shack|cabin|hut|mausoleum|church building)\b', label):
        return 2
    return None


def convert(layout, aliases):
    match = re.fullmatch(r'm(\d{2})_(\d{2})_(\d{2})_(\d{2})', layout['map_id'])
    if not match:
        raise ValueError('Invalid map ID')
    block = tuple(map(int, match.groups()))
    records = []
    for part in layout['parts']:
        if part['type'] != 'Asset':
            continue
        kind = category(aliases.get(part['model'].lower(), ''))
        if kind is None:
            continue
        x, y, z = part['position']
        if block[0] in (60, 61):
            x += block[1]*256
            z += block[2]*256
        size = max(abs(v) for v in part['scale'])
        yaw = math.radians(part['rotation'][1])
        if not all(math.isfinite(v) for v in (x, y, z, size, yaw)) or size <= 0:
            raise ValueError('Invalid scenery transform')
        radius = max(1., min(7., size*(3. if kind == 0 else 2.)))
        records.append(struct.pack('<5fB3x', x, y, z, radius, yaw, kind))
    if len(records) > 30000:
        raise ValueError('Too many scenery records')
    return b'ERSCEN01'+bytes(block)+struct.pack('<I', len(records))+b''.join(records)


def main(roots, output, aliases_path, navigation_root=None):
    navigation_root = navigation_root or output
    aliases = {v['ID'].lower(): v['Name'] for v in json.loads(
        aliases_path.read_text(encoding='utf-8-sig'))}
    maps = objects = 0
    for root in roots:
        for path in sorted(root.glob('m*/layout.json')):
            layout = json.loads(path.read_text(encoding='utf-8-sig'))
            family = 'outdoor' if layout['map_id'].startswith(('m60', 'm61')) else 'dungeons'
            directory = output/family
            # Do not create coverage for an area with no validated navigation file.
            if not (navigation_root/family/(layout['map_id']+'.nav')).exists():
                continue
            data = convert(layout, aliases)
            directory.mkdir(parents=True, exist_ok=True)
            (directory/(layout['map_id']+'.scenery')).write_bytes(data)
            maps += 1
            objects += struct.unpack_from('<I', data, 12)[0]
    if not maps:
        raise ValueError('No matching navigation layouts')
    print(f'Prepared {maps} private scenery sidecars with {objects} classified placements.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('roots', type=Path, nargs='+')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--aliases', type=Path, required=True)
    parser.add_argument('--navigation-root', type=Path, help='Existing nav data when making a sidecar-only update')
    args = parser.parse_args()
    main(args.roots, args.output, args.aliases, args.navigation_root)
