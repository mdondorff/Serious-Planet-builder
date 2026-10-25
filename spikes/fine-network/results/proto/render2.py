"""SPIKE-43 step 2 renders on the A-net's solved ground. Columns: the unbounded fill chunk at H 200 against the fill reference (green both, red missed, blue false);
the fill reference's channels (green) so the straight runs show; the bounded scheme's chunk at H 375 against the bounded reference (no red or blue where it is local by construction).
usage: python -I render2.py <mask_dir> <images_dir> [cell_m]"""
import sys
import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

d, img = sys.argv[1], sys.argv[2]
cell = float(sys.argv[3]) if len(sys.argv) > 3 else 8.037
per = 608


def ld(site, name):
    return np.fromfile(f"{d}/{site}_{name}.u8", dtype=np.uint8).reshape(per, per) > 0


fig, ax = plt.subplots(2, 3, figsize=(15, 10))
for r, site in enumerate(["hillsA", "low"]):
    z = np.fromfile(f"{d}/{site}_sg.f32", dtype="<f4").reshape(per, per).astype(float)
    gy, gx = np.gradient(z, cell)
    az, alt = np.radians(315), np.radians(40)
    slope = np.arctan(np.hypot(gx, gy))
    aspect = np.arctan2(gy, -gx)
    shade = np.sin(alt) * np.cos(slope) + np.cos(alt) * np.sin(slope) * np.cos(az - aspect)
    shade = 0.35 + 0.65 * (shade - shade.min()) / (np.ptp(shade) + 1e-9)
    fill, fill200, bnd, bnd375 = ld(site, "sg_fill"), ld(site, "sg_fill200"), ld(site, "sg_bounded"), ld(site, "sg_bounded375")
    panels = [
        (fill200, fill, f"{site}: fill chunk H 200 vs fill reference"),
        (fill, fill, f"{site}: fill reference (flats routed straight)"),
        (bnd375, bnd, f"{site}: bounded chunk H 375 vs bounded reference"),
    ]
    for c, (m, ref, title) in enumerate(panels):
        rgb = np.dstack([shade] * 3)
        for mask, col in [(m & ref, (0, 0.8, 0)), (ref & ~m, (0.95, 0, 0)), (m & ~ref, (0, 0.3, 1))]:
            rgb[mask] = col
        ax[r, c].imshow(rgb, origin="upper", extent=[0, per * cell / 1000, per * cell / 1000, 0], interpolation="nearest")
        ax[r, c].set_title(f"{title}: miss {100 * (ref & ~m).sum() / ref.sum():.1f} %, false {100 * (m & ~ref).sum() / ref.sum():.1f} %", fontsize=8)
        ax[r, c].set_xlabel("km")
plt.tight_layout()
plt.savefig(f"{img}/bounded_masks.png", dpi=60)
print("wrote", f"{img}/bounded_masks.png")
