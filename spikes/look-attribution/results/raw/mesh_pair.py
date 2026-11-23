"""SPIKE-143 step 4: the viewer's still (its mesh, lit path, straight down, north up) beside the shaded height grid of the same crop, final surface and S0, at a
10-degree sun from 270 degrees. Usage: mesh_pair.py <viewer png> <chain dir> <crop x> <crop y> <size> <out.png>"""
import sys, numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource
from PIL import Image
vp, D, x, y, C, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]), sys.argv[6]
n = 8160
ls = LightSource(azdeg=270, altdeg=10)
fig, axs = plt.subplots(1, 4, figsize=(16, 4.4), dpi=90)
im = Image.open(vp).convert('RGB').resize((600, 600))
axs[0].imshow(im); axs[0].set_title("viewer (planet_turned, flight store g1f15; mesh, lit path)", fontsize=8)
for ax, (st, lab) in zip(axs[1:], [('final', 'final, shaded height grid'), ('kh', 'S_KH, shaded height grid'), ('s0', 'S0, shaded height grid')]):
    z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n)[y:y + C, x:x + C].astype(np.float64)[::-1]
    ax.imshow(ls.hillshade(z, vert_exag=1.0, dx=101.802, dy=101.802), cmap='gray', vmin=0, vmax=1, interpolation='bilinear')
    ax.set_title(lab + ', sun 10 deg from 270 deg', fontsize=8)
for ax in axs:
    ax.set_xticks([]); ax.set_yticks([])
fig.suptitle(f'SPIKE-143 R1h river crop ({x},{y}), {C} x {C} L cells (12.2 km), north up: the viewer\'s mesh against the height grid', fontsize=9)
fig.tight_layout(rect=(0, 0, 1, 0.93))
fig.savefig(out)
