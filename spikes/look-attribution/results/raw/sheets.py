"""SPIKE-143: low-sun shaded relief per stage on the pre-chosen crops (crops.py): each stage's own height grid, sun 10 degrees high from azimuth A (degrees
clockwise from north; north up), matplotlib's LightSource hillshade at 101.8 m cells, no exaggeration; one sheet per window and azimuth: rows the crops (river, hill,
rim), columns the stages. Usage: sheets.py <chain dir> <crops.json> <window name> <azimuth> <out.png>"""
import sys, json, numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource
D, cj, name, az, out = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4]), sys.argv[5]
n = 8160
crops = json.load(open(cj)); C = crops['size']
stages = [('base', 'B (base at L)'), ('s0', 'S0'), ('kh', 'S_KH'), ('ks0', 'KS0'), ('ks1c', 'KS1 + connect'), ('band6c', 'band6 + connect'), ('kv', 'KV'), ('final', 'final (KC)')]
rows = [('river', 'river reach'), ('hill', 'hill flank'), ('rim', 'kept-lake rim')]
ls = LightSource(azdeg=az, altdeg=10)
fig, axs = plt.subplots(len(rows), len(stages), figsize=(len(stages) * 2.0, len(rows) * 2.15), dpi=110)
for c, (st, label) in enumerate(stages):
    z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n)
    for r, (key, rl) in enumerate(rows):
        x, y = crops[key]
        a = z[y:y + C, x:x + C].astype(np.float64)[::-1]  # north up (rows grow northward)
        img = ls.hillshade(a, vert_exag=1.0, dx=101.802, dy=101.802)
        ax = axs[r, c]
        ax.imshow(img, cmap='gray', vmin=0, vmax=1, interpolation='nearest')
        ax.set_xticks([]); ax.set_yticks([])
        if r == 0:
            ax.set_title(label, fontsize=8)
        if c == 0:
            ax.set_ylabel(f'{rl}\n({x},{y})', fontsize=7)
fig.suptitle(f'SPIKE-143 {name}: shaded relief of each stage\'s own height grid, sun 10 deg from {az:.0f} deg; crops of 120 x 120 L cells (12.2 km), north up', fontsize=9)
fig.tight_layout(rect=(0, 0, 1, 0.96))
fig.savefig(out)
