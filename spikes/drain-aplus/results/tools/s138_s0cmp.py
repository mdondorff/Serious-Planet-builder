import sys
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource
N, C, L = 8160, 512, 101.75
cx, cy = 1600, 5696
fig, axes = plt.subplots(1, 3, figsize=(21, 7.6), dpi=100)
ls = LightSource(azdeg=315, altdeg=10)
for ax, (p, t) in zip(axes, [(sys.argv[1], "S0 today: C1(z_s) + r"), (sys.argv[2], "S0 on the new base: C1(z_s') + r (sel-post A, no band drain, no KS)"), (sys.argv[3], "z_s' - z_s at 814 m (crop), m")]):
    if p.endswith('zs'):
        a = np.fromfile(sys.argv[4], '<f4').reshape(1020, 1020); b = np.fromfile(sys.argv[5], '<f4').reshape(1020, 1020)
        d = (b - a)[cy // 8:(cy + C) // 8, cx // 8:(cx + C) // 8]
        im = ax.imshow(d, cmap='RdBu_r', vmin=-20, vmax=20, interpolation='nearest'); plt.colorbar(im, ax=ax, fraction=0.046)
    else:
        z = np.array(np.memmap(p, dtype='<f4', mode='r', shape=(N, N))[cy:cy + C, cx:cx + C], dtype=float)
        sh = ls.hillshade(np.maximum(z, 0), vert_exag=1, dx=L, dy=L); ax.imshow(sh, cmap='gray', vmin=0, vmax=1, interpolation='nearest')
    ax.set_title(t, fontsize=10); ax.set_xticks([]); ax.set_yticks([])
fig.suptitle('F1: where the diagonal texture comes from (52 km crop, 10-degree sun). Analysis render', fontsize=12)
fig.tight_layout(rect=(0, 0, 1, 0.94)); fig.savefig(sys.argv[6], dpi=100, pil_kwargs={'quality': 85}); print('ok')
