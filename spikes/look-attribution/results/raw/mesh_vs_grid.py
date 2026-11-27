"""Lead #184g item 1: does the viewer's triangulation make part of the chainsaw? A CPU emulation of the viewer's mesh shading beside the shaded height grid,
on xdiag_synth's straight synthetic valleys (V and its 4-connected staircase floor, wall 0.12, no background), at 45 deg (along the mesh's split diagonal) and
135 deg (across it), alt 10; the sun across each valley's walls (az 45 for the 45-degree valley, az 135 for the 135-degree one; the sheet's az 315 and 135 leave a
45-degree valley's two walls equally lit).
- Mesh path (emulated, not the GPU): the vertex normals by central differences of the heights (planet_generators::normals::lattice_normals' stencil); each cell
  split into the triangles (a, b, c) and (a, c, d), a = (i, j), b = (i + 1, j), c = (i + 1, j + 1), d = (i, j + 1) (planet_generators::mesh::grid_indices); the
  normal interpolated linearly over the triangle and normalised per pixel (compact.rs: `out.normal = texel_normal`, interpolated to the fragment), Lambert.
  The plateau tilt (compact.rs `flat_w`), shadows, the sky and the tone map are left out.
- Control: the same with the other split, (a, b, d) and (b, c, d).
- Shaded grid: the look sheet's form, one shade per cell from the cell's central-difference gradient, drawn nearest.
Also prints the registered Xdiag at 45 and 135 deg (smooth and staircase). 16 px per L cell, 16 x 16 cells around the valley's middle.
Usage: mesh_vs_grid.py <out.png>"""
import os, sys, numpy as np
from PIL import Image, ImageDraw
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import importlib.util
spec = importlib.util.spec_from_file_location('xs', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'xdiag_synth.py'))
src = open(spec.origin, encoding='utf-8').read().split('def xdiag_parts')[0]
xs = {'__file__': spec.origin, '__name__': 'xs'}
exec(compile(src, spec.origin, 'exec'), xs)
from m1_facet import excess  # noqa: E402
OUT = sys.argv[1]
cell, PX, K, ALT = xs['cell'], 16, 16, 10.0
N = xs['N']


def sun(az):
    a, e = np.radians(az), np.radians(ALT)
    return np.array([np.sin(a) * np.cos(e), -np.cos(a) * np.cos(e), np.sin(e)])


def vnormals(z):
    gx = np.zeros_like(z); gy = np.zeros_like(z)
    gx[:, 1:-1] = (z[:, 2:] - z[:, :-2]) / (2 * cell); gy[1:-1, :] = (z[2:, :] - z[:-2, :]) / (2 * cell)
    n = np.stack([-gx, -gy, np.ones_like(z)], -1)
    return n / np.linalg.norm(n, axis=-1, keepdims=True)


def mesh(z, az, split):
    nv = vnormals(z)
    h, w = z.shape[0] - 1, z.shape[1] - 1
    f = (np.arange(PX) + 0.5) / PX
    fy, fx = np.meshgrid(f, f, indexing='ij')
    out = np.zeros((h * PX, w * PX))
    a, b, c, d = nv[:-1, :-1], nv[:-1, 1:], nv[1:, 1:], nv[1:, :-1]
    L = sun(az)
    for py in range(PX):
        for px in range(PX):
            x, y = fx[py, px], fy[py, px]
            if split == 'ac':
                n = a + x * (b - a) + y * (c - b) if x >= y else a + y * (d - a) + x * (c - d)
            else:  # bd split: (a, b, d) for x + y <= 1, (b, c, d) otherwise
                n = a + x * (b - a) + y * (d - a) if x + y <= 1 else c + (1 - x) * (d - c) + (1 - y) * (b - c)
            n = n / np.linalg.norm(n, axis=-1, keepdims=True)
            out[py::PX, px::PX] = np.clip(n @ L, 0, 1)
    return out


def grid(z, az):
    s = np.clip(vnormals(z) @ sun(az), 0, 1)[:-1, :-1]
    return np.repeat(np.repeat(s, PX, 0), PX, 1)


rows, labels = [], []
for ang in (45.0, 135.0):
    for floor in ('smooth', 'stair4'):
        z, zone = xs['surface'](np.radians(ang), 0.37, floor, 'v')
        mask = zone.copy(); mask[:60] = mask[-60:] = False; mask[:, :60] = mask[:, -60:] = False
        xd = excess(z, mask, 0, 0)['Xdiag']
        c0 = N // 2 - K // 2
        sub = z[c0:c0 + K + 1, c0:c0 + K + 1]
        panels = []
        az = 45.0 if ang == 45.0 else 135.0
        panels += [grid(sub, az), mesh(sub, az, 'ac'), mesh(sub, az, 'bd')]
        rows.append(panels); labels.append(f'{ang:.0f} DEG {floor.upper()} SUN AZ {45 if ang == 45.0 else 135}')
        print(f'valley at {ang:5.1f} deg, {floor:6s} floor: registered Xdiag {xd:+.3f}')
cols = ['SHADED GRID', 'MESH, VIEWER SPLIT A-C', 'MESH, CONTROL SPLIT B-D']
P = K * PX
img = Image.new('RGB', (230 + len(cols) * (P + 8), 40 + len(rows) * (P + 8)), (24, 24, 28))
dr = ImageDraw.Draw(img)
dr.text((8, 6), f'MESH (EMULATED: VERTEX NORMALS ON THE VIEWER\'S A-C SPLIT; CONTROL B-D) AGAINST THE SHADED GRID, ALT {ALT:.0f}, {PX} PX = 1 L CELL, NORTH UP', fill=(230, 230, 230))
for k, cl in enumerate(cols):
    dr.text((230 + k * (P + 8), 24), cl, fill=(150, 190, 255))
for r, (lab, ps) in enumerate(zip(labels, rows)):
    y = 40 + r * (P + 8)
    dr.text((8, y + P // 2), lab, fill=(230, 230, 230))
    for k, p in enumerate(ps):
        g = (0.15 + 0.85 * p) * 255
        img.paste(Image.fromarray(np.stack([g * 0.93, g * 0.9, g * 0.84], -1).astype(np.uint8)), (230 + k * (P + 8), y))
img.save(OUT)
