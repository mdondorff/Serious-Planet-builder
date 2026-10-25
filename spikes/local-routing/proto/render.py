"""SPIKE-42 renders: hillshade of the chunk core with the coarse path (B) and the local routes (A), and the ground/bed profiles along them.
usage: python -I render.py <out_dir_of_router> <tag> <images_dir> [cell_m]"""
import sys
import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

out, tag, img = sys.argv[1], sys.argv[2], sys.argv[3]
cell = float(sys.argv[4]) if len(sys.argv) > 4 else 8.04
per = 608
z = np.fromfile(f"{out}/{tag}_ground.f32", dtype="<f4").reshape(per, per).astype(float)
paths = {}
for line in open(f"{out}/{tag}_paths.txt"):
    name, rest = line.split(":", 1)
    paths[name] = np.array([int(x) for x in rest.split()])

gy, gx = np.gradient(z, cell)
az, alt = np.radians(315), np.radians(40)
slope = np.arctan(np.hypot(gx, gy))
aspect = np.arctan2(gy, -gx)
shade = np.sin(alt) * np.cos(slope) + np.cos(alt) * np.sin(slope) * np.cos(az - aspect)

show = [("B coarse path", "tab:red"), ("A W=1000 lam=0.02", "tab:cyan"), ("A2sym G=500 W=1000", "tab:green")]
fig, ax = plt.subplots(1, 2, figsize=(15, 7.2), gridspec_kw={"width_ratios": [1, 1]})
ax[0].imshow(shade, cmap="gray", origin="upper", extent=[0, per * cell / 1000, per * cell / 1000, 0])
for name, col in show:
    if name in paths:
        p = paths[name]
        ax[0].plot((p % per) * cell / 1000, (p // per) * cell / 1000, color=col, lw=1.4, label=name)
        ax[0].plot((p[0] % per) * cell / 1000, (p[0] // per) * cell / 1000, "o", color=col, ms=4)
ax[0].set_xlabel("km (i)")
ax[0].set_ylabel("km (j)")
ax[0].set_title(f"{tag}: hillshade of the core, ground range {z.min():.0f} to {z.max():.0f} m")
ax[0].legend(loc="lower left", fontsize=8)


def cum(p):
    d = np.hypot(np.diff(p % per), np.diff(p // per)) * cell
    return np.concatenate([[0], np.cumsum(d)])


for name, col in show:
    if name not in paths:
        continue
    p = paths[name]
    g = z.flat[p]
    bed = np.minimum.accumulate(g)
    c = cum(p) / 1000
    ax[1].plot(c, g, color=col, lw=1.0, alpha=0.8, label=f"{name}: ground")
    ax[1].plot(c, bed, color=col, lw=1.6, ls="--", label=f"{name}: running-minimum bed")
ax[1].set_xlabel("distance along the path, km")
ax[1].set_ylabel("height, m")
ax[1].set_title("ground and running-minimum bed along each path")
ax[1].legend(fontsize=7)
plt.tight_layout()
plt.savefig(f"{img}/{tag}.png", dpi=75)
print("wrote", f"{img}/{tag}.png")
