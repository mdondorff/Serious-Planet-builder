"""SPIKE-142 (#173): the hump trenches of a chain (S_KH -> final cut over 10 m through an S_KH hump over 10 m, inside the box; humps164b.py's definition) and
where their humps come from.
Per trench: the crest (the path cell highest on S_KH), its global L position modulo 6 (611 m) and 8 (814 m) cells against reference sets, its height above the
1 km (11 x 11 cell) mean of S0, and the hump measured the same way on each layer: B (the base at L), r = S0 - B (K5's regional delta), S0, S_KH.
Usage: lattice173.py <chain dir> <x0> <y0> <out prefix>"""
import sys, numpy as np
from scipy import ndimage
D, x0, y0, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
n, m = 8160, 1600
ld = lambda k: np.fromfile(f'{D}/{k}.f32', '<f4').reshape(n, n).astype(np.float64)
kh, fin = ld('kh'), ld('final')
cut = kh - fin
box = np.zeros((n, n), bool); box[m:n-m, m:n-m] = True
low = (cut > 0.25) & box
del fin
lab, k = ndimage.label(low, structure=np.ones((3, 3)))
nb = ndimage.convolve(low.astype(np.int32), np.ones((3, 3), np.int32), mode='constant') - low
ends = low & (nb <= 1)
idx = np.arange(1, k + 1)
maxcut = ndimage.maximum(cut, lab, idx)
del cut


def hump(z):
    top = ndimage.maximum(z, lab, idx)
    e = ndimage.maximum(np.where(ends, z, -np.inf), lab, idx)
    noend = ~np.isfinite(e)
    e[noend] = ndimage.minimum(z, lab, idx)[noend]
    return top - e


h_kh = hump(kh)
tr = (maxcut > 10) & (h_kh > 10)
ti = idx[tr]
print(f'{k} cut paths inside the box; {tr.sum()} trenches over 10 m through an S_KH hump over 10 m')
crest = np.array(ndimage.maximum_position(kh, lab, ti))  # (row, col)
s0 = ld('s0')
mean1k = ndimage.uniform_filter(s0, size=11, mode='nearest')
above = s0[crest[:, 0], crest[:, 1]] - mean1k[crest[:, 0], crest[:, 1]]
b = ld('base')
r = s0 - b
hum = {'S_KH': h_kh[tr], 'S0': hump(s0)[tr], 'B': hump(b)[tr], 'r': hump(r)[tr]}
del r
# reference sets: every land cell of the box (uniform), and the local maxima of S0 (3 x 3) inside the box, which carry any lattice the surface has
land = box & (s0 > 0)
lmax = box & (s0 == ndimage.maximum_filter(s0, size=3)) & land
gx, gy = crest[:, 1] + x0, crest[:, 0] + y0


def phase_table(name, px, py, P):
    h = np.zeros((P, P)); np.add.at(h, (py % P, px % P), 1)
    return h / h.sum()


def chi(p, q, N):  # chi-square of N counts with shares p against expected shares q
    return float(N * np.sum((p - q) ** 2 / np.maximum(q, 1e-12)))


ry, rx = np.nonzero(lmax)
ly, lx = np.nonzero(land[::7, ::7]); ly, lx = ly * 7, lx * 7
lines = []
for P, lab_p in ((6, '611 m (6 cells)'), (8, '814 m (8 cells)')):
    pt = phase_table('trench', gx, gy, P)
    pm = phase_table('lmax', rx + x0, ry + y0, P)
    pl = phase_table('land', lx + x0, ly + y0, P)
    N = len(gx)
    lines.append(f'phase modulo {lab_p}: trench crests against land chi2 {chi(pt, pl, N):.1f} (df {P*P-1}), against S0 local maxima {chi(pt, pm, N):.1f}; '
                 f'S0 local maxima against land chi2 {chi(pm, pl, len(rx)):.1f} over {len(rx)} maxima; trench max bin share {pt.max():.3f} (uniform {1/P/P:.3f}), '
                 f'local maxima max bin share {pm.max():.3f}')
    np.savetxt(f'{out}-phase{P}-trench.txt', pt * 100, fmt='%6.2f')
    np.savetxt(f'{out}-phase{P}-lmax.txt', pm * 100, fmt='%6.2f')
    # axis phase (x and y separately), shares in %
    for ax, v, vm in (('x', gx, rx + x0), ('y', gy, ry + y0)):
        a = np.bincount(v % P, minlength=P) / len(v) * 100
        am = np.bincount(vm % P, minlength=P) / len(vm) * 100
        lines.append(f'  {ax} mod {P}: trench crests ' + ' '.join(f'{x:5.1f}' for x in a) + ' | S0 local maxima ' + ' '.join(f'{x:5.1f}' for x in am))
for s in lines:
    print(s)
q = lambda v: f'p10 {np.percentile(v, 10):.1f} p50 {np.percentile(v, 50):.1f} p90 {np.percentile(v, 90):.1f} m'
print(f'crest height above the 1 km mean of S0: {q(above)}; under 5 m {np.mean(above < 5) * 100:.1f} %, over 20 m {np.mean(above > 20) * 100:.1f} %')
for name, v in hum.items():
    print(f'hump on {name:4s}: {q(v)}; over 10 m {np.mean(v > 10) * 100:.1f} %, over 5 m {np.mean(v > 5) * 100:.1f} %, under 2 m {np.mean(v < 2) * 100:.1f} %')
# which layer makes the hump: the share of the S_KH hump carried by B, by r and by the L hand-over (S_KH - S0)
both = (hum['B'] > 10) & (hum['r'] > 10)
print(f'humps over 10 m on B {np.sum(hum["B"] > 10)}, on r {np.sum(hum["r"] > 10)}, on both {both.sum()}, on S0 {np.sum(hum["S0"] > 10)}, of {tr.sum()} (S_KH)')
np.savez_compressed(f'{out}-trenches.npz', gx=gx, gy=gy, above=above, maxcut=maxcut[tr], **{f'h_{k_}': v for k_, v in hum.items()})
