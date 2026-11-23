"""SPIKE-143, M1 (pre-registered in results.md): the lattice facet excess of a surface. Kink energy = squared second differences (d_xx, d_yy; d_d1, d_d2 along
the diagonals, divided by 2); per period P (6 = 611 m, 8 = 814 m) the phase profile of d_xx energy against x mod P and of d_yy against y mod P (global L indices);
LS_P = the largest share held by two adjacent phase bins (cyclic), the mean of x and y; X_P = LS_P / (2 / P) - 1. X_diag: d_d2 against (x + y) mod 8 and d_d1 against
(x - y) mod 8, the mean of the two. Counted inside the box on land.
GLO-30: 3 x 3 block means of the 1 arc-second tiles (about 93 m).
Usage: m1_facet.py chain <chain dir> <x0> <y0> <stage>...   |   m1_facet.py glo30 <tif>..."""
import sys, numpy as np


def excess(z, mask, gx0, gy0):
    z = z.astype(np.float64)
    dxx = np.zeros_like(z); dyy = np.zeros_like(z); d1 = np.zeros_like(z); d2 = np.zeros_like(z)
    dxx[:, 1:-1] = z[:, 2:] - 2 * z[:, 1:-1] + z[:, :-2]
    dyy[1:-1, :] = z[2:, :] - 2 * z[1:-1, :] + z[:-2, :]
    d1[1:-1, 1:-1] = (z[2:, 2:] - 2 * z[1:-1, 1:-1] + z[:-2, :-2]) / 2.0
    d2[1:-1, 1:-1] = (z[2:, :-2] - 2 * z[1:-1, 1:-1] + z[:-2, 2:]) / 2.0
    inner = mask.copy(); inner[[0, -1], :] = False; inner[:, [0, -1]] = False
    ys, xs = np.nonzero(inner)
    gx, gy = xs + gx0, ys + gy0
    out = {}

    def ls(e, ph, P):
        h = np.bincount(ph % P, weights=e, minlength=P)
        s = h / max(h.sum(), 1e-30)
        return max(s[i] + s[(i + 1) % P] for i in range(P)), s

    for P in (6, 8):
        lx, sx = ls(dxx[ys, xs] ** 2, gx, P)
        ly, sy = ls(dyy[ys, xs] ** 2, gy, P)
        out[f'X{P}'] = (lx + ly) / 2 / (2 / P) - 1
        out[f'prof{P}'] = (sx + sy) / 2
    l2, _ = ls(d2[ys, xs] ** 2, gx + gy, 8)
    l1, _ = ls(d1[ys, xs] ** 2, gx - gy, 8)
    out['Xdiag'] = (l1 + l2) / 2 / (2 / 8) - 1
    out['energy'] = float(np.mean(dxx[ys, xs] ** 2 + dyy[ys, xs] ** 2))
    return out


def fmt(o):
    return f"X6 {o['X6']:+.3f}  X8 {o['X8']:+.3f}  Xdiag {o['Xdiag']:+.3f}  (mean kink energy {o['energy']:.2f} m2; 814 m profile " + \
        ' '.join(f'{v * 100:.1f}' for v in o['prof8']) + ')'


if __name__ == '__main__':
    if sys.argv[1] == 'chain':
        D, x0, y0 = sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
        n, m = 8160, 1600
        ld = lambda k: np.fromfile(f'{D}/{k}.f32', '<f4').reshape(n, n)
        s0 = ld('s0')
        mask = np.zeros((n, n), bool); mask[m:n-m, m:n-m] = True
        mask &= s0 > 0
        sub = (slice(m - 2, n - m + 2), slice(m - 2, n - m + 2))
        for st in sys.argv[5:]:
            z = ld(st)
            o = excess(z[sub], mask[sub], x0 + m - 2, y0 + m - 2)
            print(f'{st:7s} {fmt(o)}')
    else:
        import rasterio
        for p in sys.argv[2:]:
            with rasterio.open(p) as src:
                a = src.read(1).astype(np.float64)
            # 3 x 3 block means of the 1 arc-second grid (about 93 m; amended before any render: a linear zoom to 101.8 m beats against the source grid and makes
            # a false 8-cell phase, X8 up to +1.4 on six tiles)
            h, w = (a.shape[0] // 3) * 3, (a.shape[1] // 3) * 3
            z = a[:h, :w].reshape(h // 3, 3, w // 3, 3).mean(axis=(1, 3))
            mask = np.ones(z.shape, bool)
            o = excess(z, mask, 0, 0)
            print(f"{p.split('/')[-1][:40]:40s} {fmt(o)}")
