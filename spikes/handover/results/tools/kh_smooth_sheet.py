"""SPIKE-136 addendum 2 (spikes #138): one sheet per window. #137's crop of the region box (the 512 x 512 L-cell block with the largest p50 1 km relief), every L cell a
pixel, hillshade at sun altitude 10 degrees from the north-west, no vertical exaggeration: C1(z_s) + r (S0), h2 + r (the triangulation), h3 + r (the skeleton), h4 + r
(Clough-Tocher). Analysis render (CPU), not the production path.
Usage: kh_smooth_sheet.py <dir> <tag> <window name> <out.jpg> [--h4c] (--h4c: panels C1, h2, h4, h4c, spikes #139)
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
    h4c = '--h4c' in sys.argv[5:]
    f = glob.glob(f'{d}/{tag}-crop-s0-*.f32')[0]
    crop = int(f.rsplit('-', 1)[1].split('.')[0])
    ls = LightSource(azdeg=315, altdeg=10)
    fig, axes = plt.subplots(2, 2, figsize=(14, 14.6), dpi=100)
    panels = [('s0', 'C1(z_s) + r (S0, today)'), ('h2r', 'h2 + r: the triangulation (#137)'), ('h3r', 'h3 + r: the skeleton (C1 lowered along each D8 diagonal step)'),
              ('h4r', 'h4 + r: Clough-Tocher on h2\'s split')]
    if h4c:
        panels = [('s0', 'C1(z_s) + r (S0, today)'), ('h2r', 'h2 + r: the triangulation (#137)'), ('h4r', "h4 + r: Clough-Tocher on h2's split (#138)"),
                  ('h4cr', "h4c + r: h4 clamped to its quad's corners")]
    for ax, (key, title) in zip(axes.flat, panels):
        z = np.fromfile(f'{d}/{tag}-crop-{key}-{crop}.f32', '<f4').reshape(crop, crop).astype(float)
        sh = ls.hillshade(np.maximum(z, 0.0), vert_exag=1.0, dx=L_M, dy=L_M)
        rgb = np.dstack([sh] * 3)
        rgb[z <= 0] = [0.95, 0.93, 0.86]
        ax.imshow(rgb, interpolation='nearest', vmin=0, vmax=1)
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
