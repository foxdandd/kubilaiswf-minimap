"""Generate private, height-aware outdoor layouts from local navigation exports.
Usage: python tools/prepare_outdoor.py EXPORT_ROOT [EXPORT_ROOT ...] --output PACKAGE
Game geometry stays in ignored .nav files; no game data belongs in Git.
"""
import argparse
from collections import Counter
import csv
import gzip
import json
import math
from pathlib import Path
import re
import struct


def convert(directory):
    match = re.fullmatch(r'm(60|61)_(\d{2})_(\d{2})_(00|10)', directory.name)
    if not match:
        raise ValueError('Unsupported outdoor tile')
    block = tuple(map(int, match.groups()))
    with gzip.open(directory/'navmesh.json.gz', 'rt', encoding='utf-8') as stream:
        navigation = json.load(stream)
    with gzip.open(directory/'placements.json.gz', 'rt', encoding='utf-8') as stream:
        placements = json.load(stream)
    instances = {v['ModelID']: v for v in placements['Navmeshes']
                 if (v['NameID'] & 0xffffffff) < 2_000_000_000 and v['FaceCount'] > 0}
    triangles, edges, seen = [], [], set()
    for mesh in navigation['meshes']:
        if not mesh['faces']:
            continue
        model = int(mesh['name'].rsplit('_', 1)[1].split('.')[0])
        instance = instances[model]
        if model in seen or instance['FaceCount'] != len(mesh['faces']):
            raise ValueError('Duplicate model or NVA face-count mismatch')
        seen.add(model)
        if any(abs(instance['Position'][k]) > 1e-5 or abs(instance['Rotation'][k]) > 1e-5
               or abs(instance['Scale'][k]-1) > 1e-5 for k in 'XYZ'):
            raise ValueError('Nonidentity outdoor NVA transform needs explicit support')
        vertices = []
        for x, y, z in mesh['vertices']:
            # Some navigation polygons extend beyond their 256 m tile into a neighbour.
            if not all(math.isfinite(v) for v in (x, y, z)) or max(abs(x), abs(z)) > 1024:
                raise ValueError(f'{directory.name} model {model}: unexpected tile-local coordinates {(x,y,z)}')
            vertices.append((x+block[1]*256, y, z+block[2]*256))
        for face in mesh['faces']:
            for i in range(1, len(face)-1):
                points = [vertices[j] for j in (face[0], face[i], face[i+1])]
                a, b, c = points
                if abs((b[0]-a[0])*(c[2]-a[2])-(b[2]-a[2])*(c[0]-a[0])) > 1e-7:
                    triangles.append(struct.pack('<9fI', *(v for p in points for v in p), model))
        for edge in mesh['edges']:
            if edge[3] == 0xffffffff:
                edges.append(struct.pack('<6f', *vertices[edge[0]], *vertices[edge[1]]))
    if seen != set(instances):
        raise ValueError('Standard NVA model absent from bundle')
    if not triangles:
        return None
    if max(len(triangles), len(edges)) > 200_000:
        raise ValueError('Outdoor tile exceeds record limit')
    return b'ERWLD001'+bytes(block)+struct.pack('<II', len(triangles), len(edges))+b''.join(triangles+edges)


def main(roots, output):
    destination = output/'outdoor'
    destination.mkdir(parents=True, exist_ok=True)
    catalog, skipped = {}, []
    for root in roots:
        names = {}
        if (root/'maps.csv').exists():
            with (root/'maps.csv').open(encoding='utf-8-sig', newline='') as stream:
                names = {row['map_id']: row['name'] for row in csv.DictReader(stream)}
        for directory in sorted(root.iterdir()):
            if not directory.is_dir() or not (directory/'navmesh.json.gz').exists():
                continue
            if not (directory/'placements.json.gz').exists():
                skipped.append({'map': directory.name, 'reason': 'no NVA placements'})
                continue
            try:
                data = convert(directory)
            except (EOFError, json.JSONDecodeError):
                skipped.append({'map': directory.name, 'reason': 'incomplete navigation export'})
                continue
            if data is None:
                skipped.append({'map': directory.name, 'reason': 'empty navigation tile'})
                continue
            if directory.name in catalog:
                raise ValueError('Duplicate tile across export roots')
            name = names.get(directory.name) or directory.name
            if len(name.encode()) > 512 or any(c in name for c in '\t\r\n'):
                raise ValueError('Invalid tile caption')
            (destination/f'{directory.name}.nav').write_bytes(data)
            catalog[directory.name] = name
    if not catalog:
        raise ValueError('No outdoor layouts generated')
    (destination/'maps.tsv').write_text('map_id\tname\n'+''.join(
        f'{key}\t{catalog[key]}\n' for key in sorted(catalog)), encoding='utf-8')
    print(json.dumps(dict(tiles=len(catalog), base=sum(k.startswith('m60') for k in catalog),
                         dlc=sum(k.startswith('m61') for k in catalog),
                         skipped=dict(Counter(v['reason'] for v in skipped)))))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('roots', type=Path, nargs='+')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    main(args.roots, args.output)
