"""Generate a private Tombsward test sidecar from local navigation exports.
Usage: python tools/prepare_dungeon.py NAVIGATION_ROOT OUTPUT_DIRECTORY
The generated sidecar contains game-derived geometry. Do not add it to Git.
"""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import struct


def prepare(root, output):
    directory = root / 'm30_00_00_00'
    with gzip.open(directory / 'navmesh.json.gz', 'rt', encoding='utf-8') as stream:
        navigation = json.load(stream)
    with gzip.open(directory / 'placements.json.gz', 'rt', encoding='utf-8') as stream:
        placements = json.load(stream)
    models = {int(m['name'].rsplit('_', 1)[-1].split('.')[0]): m for m in navigation['meshes']}
    instances = {n['NameID']: n for n in placements['Navmeshes'] if (n['NameID'] & 0xffffffff) < 2_000_000_000}
    linked_edges = {(c['MainNameID'], link['EdgeIndex']) for c in placements['Connectors']
                    for link in c['NavMeshConnections']}
    triangles, boundaries = [], []
    for section, instance in instances.items():
        if any(abs(instance['Position'][k]) > 1e-5 or abs(instance['Rotation'][k]) > 1e-5
               or abs(instance['Scale'][k]-1) > 1e-5 for k in 'XYZ'):
            raise ValueError('This pilot requires identity NVA instance transforms')
        mesh = models[instance['ModelID']]
        if len(mesh['faces']) != instance['FaceCount']:
            raise ValueError('NVA and navmesh polygon counts differ')
        for face in mesh['faces']:
            points = [mesh['vertices'][i] for i in face]
            for i in range(1, len(points)-1):
                triangle = [points[0], points[i], points[i+1]]
                a,b,c = triangle
                area = (b[0]-a[0])*(c[2]-a[2])-(b[2]-a[2])*(c[0]-a[0])
                if abs(area) > 1e-7:
                    triangles.append((triangle, instance['ModelID']))
        for i, edge in enumerate(mesh['edges']):
            if edge[3] == 0xffffffff and (section, i) not in linked_edges:
                boundaries.append([mesh['vertices'][edge[0]], mesh['vertices'][edge[1]]])
    data = bytearray(b'ERDNG001' + bytes([30,0,0,0]) + struct.pack('<II', len(triangles), len(boundaries)))
    for points, section in triangles:
        values = [v for p in points for v in p]
        if not all(math.isfinite(x) for x in values): raise ValueError('Invalid coordinate')
        data.extend(struct.pack('<9fI', *values, section))
    for points in boundaries:
        data.extend(struct.pack('<6f', *(v for p in points for v in p)))
    output.mkdir(parents=True, exist_ok=True)
    path = output / 'tombsward.nav'
    path.write_bytes(data)
    info = dict(map_id='m30_00_00_00', triangles=len(triangles), boundary_segments=len(boundaries),
                bytes=len(data), sha256=hashlib.sha256(data).hexdigest(),
                source_navmesh_sha256=hashlib.sha256((directory/'navmesh.json.gz').read_bytes()).hexdigest(),
                limitations=['AI walkable surfaces, not physical walls or verified room boundaries.',
                             'Vanilla Tombsward Catacombs only; live alignment requires gameplay verification.'])
    (output/'dungeon-data.json').write_text(json.dumps(info,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(info,indent=2))


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    args=parser.parse_args()
    prepare(args.root,args.output)
