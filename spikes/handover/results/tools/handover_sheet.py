"""SPIKE-136: one sheet per window. Two panels over the window's region box (4 x 4 L cells, 407 m a pixel): h1 + r (SPIKE-116's 4-neighbour breach, bilinear, raw band)
and h2 + r (the flow-diagonal triangulation, raw band). On a low-sun hillshade of S0, each panel colours the surface's breach cells (lowered over 1 m by the complete breach
along the routing) by S0's class at the cell (SPIKE-135: base, K5, interpolation, band; black: a cell S0 does not breach). In h1's panel, magenta: the blocks where h1's
own 814 m breach, read at L (bilinear of the lowering), cuts over 50 m (h2 lowers nothing at 814 m). A pixel shows the class of the block's deepest breach cell. Analysis render (CPU), not the production path.
Usage: handover_sheet.py <dir> <tag> <title> <out.jpg>
"""
import sys
import glob
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource, to_rgb
from matplotlib.patches import Patch

CLASSES = [(1, 'base', '#6a3d9a'), (2, 'K5', '#d95f02'), (3, 'interpolation', '#1f78b4'), (4, 'band', '#e31a1c'), (5, 'not breached on S0', '#000000')]


def panel(ax, d, tag, key, rgb0, cw, ch, title, cut=None):
    cls = np.fromfile(f'{d}/{tag}-{key}-class-{cw}x{ch}.u8', np.uint8).reshape(ch, cw)
    rgb = rgb0.copy()
    counts = {}
    for k, name, col in CLASSES:
        m = cls == k
        rgb[m] = to_rgb(col)
        counts[name] = int(m.sum())
    if cut is not None:
        m50 = cut > 50.0
        rgb[m50] = to_rgb('#ff00ff')
    ax.imshow(rgb, interpolation='nearest')
    ax.set_xticks([])
    ax.set_yticks([])
    px = 4 * 101.75
    bar = 20_000 / px
    x0, y0 = cw * 0.05, ch * 0.95
    ax.plot([x0, x0 + bar], [y0, y0], color='black', lw=4)
    ax.text(x0 + bar / 2, y0 - ch * 0.015, '20 km', ha='center', va='bottom', fontsize=9, bbox=dict(facecolor='white', alpha=0.7, lw=0))
    handles = [Patch(color=col, label=f'breach on S0 class {name} ({counts[name]} px)') if k < 5 else Patch(color=col, label=f'breach where S0 has none ({counts[name]} px)') for k, name, col in CLASSES]
    if cut is not None:
        handles.append(Patch(color='#ff00ff', label=f"h1's 814 m breach cuts over 50 m at L ({int(m50.sum())} px)"))
    ax.legend(handles=handles, loc='upper right', fontsize=7, framealpha=0.85)
    ax.set_title(title, fontsize=10)


def main():
    d, tag, title, out = sys.argv[1:5]
    f = glob.glob(f'{d}/{tag}-s0-*.f32')[0]
    cw, ch = map(int, f.rsplit('-', 1)[1].split('.')[0].split('x'))
    z = np.fromfile(f, '<f4').reshape(ch, cw).astype(float)
    px = 4 * 101.75
    shade = LightSource(azdeg=315, altdeg=20).hillshade(np.maximum(z, 0.0), vert_exag=2.0, dx=px, dy=px)
    rgb0 = np.dstack([shade] * 3) * 0.85 + 0.15
    rgb0[z <= 0] = [0.95, 0.93, 0.86]
    fig, axes = plt.subplots(1, 2, figsize=(19.2, 10.2), dpi=100)
    # h1's 814 m lowering read bilinearly at the L cells of the box (cell centres 8i + 3.5), the block maximum of 4 x 4
    f = glob.glob(f'{d}/{tag}-h1low-*.f32')[0]
    rw, rh = map(int, f.rsplit('-', 1)[1].split('.')[0].split('x'))
    low = np.fromfile(f, '<f4').reshape(rh, rw).astype(float)
    m = (rw - cw * 4 // 8) // 2
    x = np.arange(m * 8, m * 8 + cw * 4)
    fx = np.clip((x - 3.5) / 8.0, 0, rw - 1.000001)
    i0 = np.floor(fx).astype(int)
    u = fx - i0
    rows = low[:, i0] * (1 - u) + low[:, np.minimum(i0 + 1, rw - 1)] * u
    y = np.arange(m * 8, m * 8 + ch * 4)
    fy = np.clip((y - 3.5) / 8.0, 0, rh - 1.000001)
    j0 = np.floor(fy).astype(int)
    v = (fy - j0)[:, None]
    at_l = rows[j0, :] * (1 - v) + rows[np.minimum(j0 + 1, rh - 1), :] * v
    cut = at_l.reshape(ch, 4, cw, 4).max(axis=(1, 3))
    panel(axes[0], d, tag, 'h1r', rgb0, cw, ch, 'h1 + r: z_s breached over 4 neighbours at 814 m, bilinear, raw band', cut)
    panel(axes[1], d, tag, 'h2r', rgb0, cw, ch, 'h2 + r: flow-diagonal triangulation of z_s, no lowering, raw band')
    fig.suptitle(title, fontsize=12)
    fig.tight_layout(rect=(0, 0, 1, 0.96))
    fig.savefig(out, dpi=100, pil_kwargs={'quality': 72})
    print('wrote', out)


if __name__ == '__main__':
    main()
