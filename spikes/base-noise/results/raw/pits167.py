"""SPIKE-140 item 4: closed depressions of the base against those of S0 on a window (L grid, f32 dumps). A depression cell: the priority fill (outlets: the sea and the window's
border) raises it by more than 0.5 m; a basin: a connected set of such cells of at least 16 cells (the gate's basin rule, BUILD-055). Reports the land share in basins on each surface,
the share of S0's basin area that lies in a base basin, and depth percentiles. Usage: python -I pits167.py <chain dir> <label>"""
import sys
import numpy as np
from skimage.morphology import reconstruction
from scipy.ndimage import label as cc

d, name = sys.argv[1], sys.argv[2]
N = 8160
box = (slice(1600, 6560), slice(1600, 6560))


def basins(z):
    seed = np.full_like(z, z.max() + 1.0)
    edge = np.zeros(z.shape, bool)
    edge[0, :] = edge[-1, :] = edge[:, 0] = edge[:, -1] = True
    out = edge | (z <= 0)
    seed[out] = z[out]
    filled = reconstruction(seed, z, method="erosion")
    depth = filled - z
    wet = depth > 0.5
    lab, n = cc(wet, structure=np.ones((3, 3)))
    sizes = np.bincount(lab.ravel())
    big = sizes >= 16
    big[0] = False
    return big[lab], depth


for f in ["base", "s0"]:
    z = np.fromfile(f"{d}/{f}.f32", dtype="<f4").reshape(N, N).astype(np.float64)
    m, dep = basins(z)
    land = z[box] > 0
    globals()[f] = (m[box] & land, dep[box], land)
    mb = m[box] & land
    print(f"{name} {f}: {100 * mb.sum() / land.sum():.2f} % of the box's land in closed basins (>0.5 m, >=16 cells); depth p50 {np.median(dep[box][mb]) if mb.any() else 0:.1f} m p95 {np.percentile(dep[box][mb], 95) if mb.any() else 0:.1f} m")
    del z, m, dep
bm, sm = base[0], s0[0]
both = (bm & sm).sum()
print(f"{name}: {100 * both / max(sm.sum(), 1):.1f} % of S0's basin area lies in a base basin; {100 * both / max(bm.sum(), 1):.1f} % of the base's basin area stays a basin in S0")
