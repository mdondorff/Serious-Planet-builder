"""Lead #184f check 1: does the registered in-zone Xdiag (m1_facet.excess) read a smooth, lattice-aligned valley as facets? Synthetic L surfaces (101.802 m cells),
one straight valley through the domain's centre at an angle, the zone the cells within 12 L cells of the valley line (KH's zone: the H1 channel cells and their 8
neighbours, 3 H1 cells = 24 L cells wide). Profiles: V (KH's `v` profile, target = bed + wall slope x distance) and round (the hyperbola, 1 L cell floor radius).
Floors: `smooth` (distance to the true line, a sub-cell offset per row), `stair4` (distance to the line's 4-connected raster cells: a lattice-locked staircase floor,
the chainsaw's mechanism), `stair8` (the 8-connected raster). Reported per angle and floor: Xdiag as registered, its energy-weighted form, and d1's share of the diagonal kink energy. Usage: xdiag_synth.py"""
import os, sys, numpy as np
from scipy import ndimage
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from m1_facet import excess  # noqa: E402  (the registered definition)

N, cell, wall, grade, zone_r = 1400, 101.802, 0.12, 0.004, 12.0
rng = np.random.default_rng(184)
yy, xx = np.mgrid[0:N, 0:N].astype(np.float64)


def raster(theta, off, four):
    """The line's cells: a 4-connected (or 8-connected) walk along the line, the cell under the line at each half-cell step, gaps filled by one axis step."""
    c, s = np.cos(theta), np.sin(theta)
    cx, cy = N / 2 + off * -s, N / 2 + off * c
    t = np.arange(-N, N, 0.25)
    px, py = np.floor(cx + t * c + 0.5).astype(int), np.floor(cy + t * s + 0.5).astype(int)
    cells = [(px[0], py[0])]
    for x, y in zip(px[1:], py[1:]):
        lx, ly = cells[-1]
        if (x, y) == (lx, ly):
            continue
        if four and x != lx and y != ly:
            # the axis step whose cell lies nearer the line
            a, b = (x, ly), (lx, y)
            da = abs(-(a[0] - cx) * s + (a[1] - cy) * c); db = abs(-(b[0] - cx) * s + (b[1] - cy) * c)
            cells.append(a if da <= db else b)
        cells.append((x, y))
    m = np.zeros((N, N), bool)
    for x, y in cells:
        if 0 <= x < N and 0 <= y < N:
            m[y, x] = True
    return m


def surface(theta, off, floor, profile):
    c, s = np.cos(theta), np.sin(theta)
    cx, cy = N / 2 + off * -s, N / 2 + off * c
    along = (xx - cx) * c + (yy - cy) * s
    dl = np.abs(-(xx - cx) * s + (yy - cy) * c)  # distance to the true line, L cells
    if floor == 'smooth':
        d = dl
    else:
        d = ndimage.distance_transform_edt(~raster(theta, off, floor == 'stair4'))
    dm = d * cell
    reach = dm if profile == 'v' else np.sqrt(dm * dm + cell * cell) - cell
    z = 200.0 + grade * along * cell + wall * reach
    return z, dl <= zone_r


def xdiag_parts(z, mask):
    """Xdiag as registered, plus the energy-weighted form (each diagonal's phase share weighted by its kink energy; the registered form weights them equally)."""
    d1 = np.zeros_like(z); d2 = np.zeros_like(z)
    d1[1:-1, 1:-1] = (z[2:, 2:] - 2 * z[1:-1, 1:-1] + z[:-2, :-2]) / 2.0
    d2[1:-1, 1:-1] = (z[2:, :-2] - 2 * z[1:-1, 1:-1] + z[:-2, 2:]) / 2.0
    ys, xs = np.nonzero(mask)
    def ls(e, ph):
        h = np.bincount(ph % 8, weights=e, minlength=8); s = h / max(h.sum(), 1e-30)
        return max(s[i] + s[(i + 1) % 8] for i in range(8)), h.sum()
    l1, e1 = ls(d1[ys, xs] ** 2, xs - ys + 8 * N); l2, e2 = ls(d2[ys, xs] ** 2, xs + ys)
    return excess(z, mask, 0, 0)['Xdiag'], (e1 * l1 + e2 * l2) / (e1 + e2) / 0.25 - 1, e1 / (e1 + e2)


# the backgrounds: none, and R1h's own S0 (the defaults' chain dump, 1400 x 1400 patches inside the box; the patch's phase origin is a multiple of 8 so the global
# phase is kept), the valley added on top
S0 = 'C:/DEV/SeriousPlanetData/tmp/g1f6/spikes/s184/r1h-def-chain/s0.f32'
s0 = np.fromfile(S0, '<f4').reshape(8160, 8160).astype(np.float64) if os.path.exists(S0) else None
patches = [(1600, 1600), (2000, 3600), (2600, 2400), (1800, 5000)]  # land shares 0.99-1.00
angles = [0.0, 22.5, 45.0, 67.5]
lines = [f'Synthetic valleys, N {N}, wall {wall}, grade {grade}, zone |d| <= {zone_r} L cells; per cell: Xdiag registered / energy-weighted / d1 energy share, '
         'mean over 8 sub-cell offsets (smooth background) or 4 S0 patches x 2 offsets']
for bg in ('none', 's0'):
    if bg == 's0' and s0 is None:
        continue
    for profile in ('v', 'round'):
        for floor in ('smooth', 'stair4', 'stair8'):
            row = []
            for a in angles:
                r = []
                for k, off in enumerate(rng.uniform(0, 1, 8)):
                    z, zone = surface(np.radians(a), off, floor, profile)
                    mask = zone.copy(); mask[:60, :] = mask[-60:, :] = False; mask[:, :60] = mask[:, -60:] = False
                    if bg == 's0':
                        py, px = patches[k % 4]
                        b = s0[py:py + N, px:px + N]
                        z = z + b; mask &= b > 0
                    r.append(xdiag_parts(z, mask))
                r = np.array(r)
                row.append(f'{a:4.1f} deg {r[:, 0].mean():+.3f} / {r[:, 1].mean():+.3f} / {r[:, 2].mean():.2f}')
            lines.append(f'{bg:4s} {profile:5s} {floor:6s}: ' + '   '.join(row))
    if s0 is not None and bg == 's0':
        r = []
        for (py, px) in patches:
            b = s0[py:py + N, px:px + N]
            yy2, xx2 = np.mgrid[0:N, 0:N]
            mask = (b > 0) & (yy2 >= 60) & (yy2 < N - 60) & (xx2 >= 60) & (xx2 < N - 60)
            r.append(xdiag_parts(b, mask))
        r = np.array(r)
        lines.append(f'S0 patches alone (whole patch, no zone): {r[:, 0].mean():+.3f} / {r[:, 1].mean():+.3f} / {r[:, 2].mean():.2f}')
print(chr(10).join(lines))
