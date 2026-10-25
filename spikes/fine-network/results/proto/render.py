"""SPIKE-43 renders: hillshade of the centre chunk core with the channel mask (threshold 0.3 km2) of a chunk solved with halo H against the reference.
green: channel in both; red: reference only (missed); blue: chunk only (false).
usage: python -I render.py <mask_dir> <images_dir> [cell_m]"""
import sys
import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

d, img = sys.argv[1], sys.argv[2]
cell = float(sys.argv[3]) if len(sys.argv) > 3 else 8.037
per = 608
rows = [("low", [0, 100, 375]), ("hillsA", [0, 375, 600])]
fig, ax = plt.subplots(2, 3, figsize=(15, 10))
for r, (site, hs) in enumerate(rows):
    z = np.fromfile(f"{d}/{site}_ground.f32", dtype="<f4").reshape(per, per).astype(float)
    ref = np.fromfile(f"{d}/{site}_ref.u8", dtype=np.uint8).reshape(per, per) > 0
    gy, gx = np.gradient(z, cell)
    az, alt = np.radians(315), np.radians(40)
    slope = np.arctan(np.hypot(gx, gy))
    aspect = np.arctan2(gy, -gx)
    shade = np.sin(alt) * np.cos(slope) + np.cos(alt) * np.sin(slope) * np.cos(az - aspect)
    for c, h in enumerate(hs):
        m = np.fromfile(f"{d}/{site}_h{h}.u8", dtype=np.uint8).reshape(per, per) > 0
        rgb = np.dstack([shade] * 3)
        rgb = 0.35 + 0.65 * (rgb - rgb.min()) / (np.ptp(rgb) + 1e-9)
        for mask, col in [(m & ref, (0, 0.8, 0)), (ref & ~m, (0.95, 0, 0)), (m & ~ref, (0, 0.3, 1))]:
            rgb[mask] = col
        ax[r, c].imshow(rgb, origin="upper", extent=[0, per * cell / 1000, per * cell / 1000, 0], interpolation="nearest")
        ax[r, c].set_title(f"{site}, halo H = {h} cells: miss {100 * (ref & ~m).sum() / ref.sum():.1f} %, false {100 * (m & ~ref).sum() / ref.sum():.1f} %", fontsize=9)
        ax[r, c].set_xlabel("km")
plt.tight_layout()
plt.savefig(f"{img}/halo_masks.png", dpi=60)
print("wrote", f"{img}/halo_masks.png")
