"""SPIKE-135: one sheet per window. A low-sun hillshade of S0 over the window's region box (block means of 4 x 4 L cells, 407 m a pixel) with S0's breach cells
(lowered over 1 m by the complete breach along the routing) coloured by the first rung that has a breach there: base (z_b at 814 m), K5 (z_s at 814 m, not z_b),
interpolation (C1(z_s) at L, not z_s at 814 m), band (S0 only). Dark: the source-fixed hand-over (L5) keeps a breach within 4 L cells; light: it has none there.
A pixel shows the class of the block's deepest breach cell. Analysis render (CPU), not the production path.
Usage: repair_sheet.py <out dir> <tag> <title> <out.jpg>
"""
import sys
import glob
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource
from matplotlib.patches import Patch

CLASSES = [(1, 'base', '#6a3d9a', '#cab2d6'), (2, 'K5', '#d95f02', '#fdbf6f'), (3, 'interpolation', '#1f78b4', '#a6cee3'), (4, 'band', '#e31a1c', '#fb9a99')]


def main():
    d, tag, title, out = sys.argv[1:5]
    f = glob.glob(f'{d}/{tag}-class-*.u8')[0]
    cw, ch = map(int, f.rsplit('-', 1)[1].split('.')[0].split('x'))
    cls = np.fromfile(f, np.uint8).reshape(ch, cw)
    z = np.fromfile(f'{d}/{tag}-s0-{cw}x{ch}.f32', '<f4').reshape(ch, cw).astype(float)
    px = 4 * 101.75
    shade = LightSource(azdeg=315, altdeg=20).hillshade(np.maximum(z, 0.0), vert_exag=2.0, dx=px, dy=px)
    rgb = np.dstack([shade] * 3) * 0.85 + 0.15
    rgb[z <= 0] = [0.95, 0.93, 0.86]
    counts = []
    for k, name, dark, light in CLASSES:
        for fixed, col in ((False, dark), (True, light)):
            m = ((cls & 0x0f) == k) & (((cls & 0x10) != 0) == fixed)
            rgb[m] = matplotlib.colors.to_rgb(col)
            counts.append((name, fixed, int(m.sum())))
    fig, ax = plt.subplots(figsize=(10.4, 10.4), dpi=120)
    ax.imshow(rgb, interpolation='nearest')
    ax.set_xticks([])
    ax.set_yticks([])
    # scale bar: 20 km
    bar = 20_000 / px
    x0, y0 = cw * 0.05, ch * 0.95
    ax.plot([x0, x0 + bar], [y0, y0], color='black', lw=4)
    ax.text(x0 + bar / 2, y0 - ch * 0.015, '20 km', ha='center', va='bottom', fontsize=10, color='black', bbox=dict(facecolor='white', alpha=0.7, lw=0))
    handles = []
    for k, name, dark, light in CLASSES:
        nd = sum(c for n, fx, c in counts if n == name and not fx)
        nl = sum(c for n, fx, c in counts if n == name and fx)
        handles.append(Patch(color=dark, label=f'{name}: the hand-over keeps a breach ({nd} px)'))
        handles.append(Patch(color=light, label=f'{name}: the hand-over has none near ({nl} px)'))
    ax.legend(handles=handles, loc='upper right', fontsize=8, framealpha=0.85)
    ax.set_title(title, fontsize=11)
    fig.tight_layout()
    fig.savefig(out, dpi=120, pil_kwargs={'quality': 82})
    print('wrote', out)


if __name__ == '__main__':
    main()
