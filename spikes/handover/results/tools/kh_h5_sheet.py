"""SPIKE-136 addendum 4 (spikes #144b): one sheet per window. #137's crop of the region box (the 512 x 512 L-cell block with the largest p50 1 km relief), every L cell a
pixel, hillshade at sun altitude 10 degrees from the north-west, no vertical exaggeration: C1(z_s) + r (S0), h4c + r, h5 + r, h4m + r, h4b + r, and h5 + r with the
814 m channel steps (D8 area >= 75 km2) that define h5's zone. Analysis render (CPU), not the production path.
Usage: kh_h5_sheet.py <dir> <tag> <window name> <out.jpg>
"""
import sys
import glob
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource

L_M = 101.75


def main():
    d, tag, name, out = sys.argv[1:5]
    f = glob.glob(f'{d}/{tag}-crop-s0-*.f32')[0]
    crop = int(f.rsplit('-', 1)[1].split('.')[0])
    ls = LightSource(azdeg=315, altdeg=10)
    fig, axes = plt.subplots(2, 3, figsize=(21, 14.6), dpi=100)
    panels = [('s0', 'C1(z_s) + r (S0, today)', False), ('h4cr', "h4c + r: h4 clamped to its quad's corners", False),
              ('h5r', 'h5 + r: h4c on the 75 km2 channel zone, C1 elsewhere', False), ('h4mr', 'h4m + r: h4 clamped to its macro triangle', False),
              ('h4br', 'h4b + r: h4c within 2 L cells of D8 diagonals, C1 beyond 4', False), ('h5r', 'h5 + r with the 814 m channel steps (>= 75 km2)', True)]
    rivers = np.loadtxt(f'{d}/{tag}-crop-rivers-{crop}.txt', ndmin=2) if glob.glob(f'{d}/{tag}-crop-rivers-{crop}.txt') else np.zeros((0, 5))
    for ax, (key, title, lines) in zip(axes.flat, panels):
        z = np.fromfile(f'{d}/{tag}-crop-{key}-{crop}.f32', '<f4').reshape(crop, crop).astype(float)
        sh = ls.hillshade(np.maximum(z, 0.0), vert_exag=1.0, dx=L_M, dy=L_M)
        rgb = np.dstack([sh] * 3)
        rgb[z <= 0] = [0.95, 0.93, 0.86]
        ax.imshow(rgb, interpolation='nearest', vmin=0, vmax=1)
        if lines and len(rivers):
            for x1, y1, x2, y2, a in rivers:
                ax.plot([x1, x2], [y1, y2], color='#1f5fbf', lw=0.6 + 0.6 * np.log10(max(a, 75.0) / 75.0), solid_capstyle='round')
            ax.set_xlim(-0.5, crop - 0.5)
            ax.set_ylim(crop - 0.5, -0.5)
        ax.set_xticks([])
        ax.set_yticks([])
        bar = 10_000 / L_M
        x0, y0 = crop * 0.05, crop * 0.95
        ax.plot([x0, x0 + bar], [y0, y0], color='black', lw=4)
        ax.text(x0 + bar / 2, y0 - crop * 0.015, '10 km', ha='center', va='bottom', fontsize=9, bbox=dict(facecolor='white', alpha=0.7, lw=0))
        ax.set_title(title, fontsize=11)
    fig.suptitle(f'{name}: {crop * L_M / 1000:.0f} km crop, 1 px an L cell (101.8 m), sun altitude 10 deg from the NW, no vertical exaggeration.\nAnalysis render, not the production path',
                 fontsize=12)
    fig.tight_layout(rect=(0, 0, 1, 0.95), h_pad=2.5)
    fig.savefig(out, dpi=100, pil_kwargs={'quality': 85})
    print('wrote', out)


if __name__ == '__main__':
    main()
