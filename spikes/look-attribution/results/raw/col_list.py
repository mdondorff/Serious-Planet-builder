"""Lead #184q: row 3's col worklist on a step-1 chain against its defaults: runs of curve raster cells (in the box) that KH cuts deeper than max(the defaults' KH lowering,
10 m), both measured as S0 - S_KH; per run its length, the deepest final cut (S0 - final) and its centre. Writes <out>.csv and prints the counts by depth.
Usage: col_list.py <chain dir> <defaults chain dir> <trunk-curves.bin> <out.csv>"""
import sys, struct, numpy as np
D, DD, CB, OUT = sys.argv[1:5]
n, m, cell = 8160, 1600, 101.802
ld = lambda d, s: np.fromfile(f'{d}/{s}.f32', '<f4').reshape(n, n)
s0 = ld(D, 's0'); khm = s0 - ld(D, 'kh'); khd = s0 - ld(DD, 'kh'); fin = s0 - ld(D, 'final')
b = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; cols = []
for _ in range(cnt):
    r, npt = struct.unpack_from('<II', b, k); k += 8 + 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4
    c = np.frombuffer(b, np.dtype([('c', '<u4'), ('o', 'u1', 2)]), nr, k)['c'].astype(np.int64); k += 6 * nr
    x, y = c % n, c // n; inb = (x >= m) & (x < n - m) & (y >= m) & (y < n - m)
    h = inb & (khm[y, x] > np.maximum(khd[y, x], 10.0) + 1e-6)
    i = 0
    while i < len(h):
        if h[i]:
            j = i
            while j < len(h) and h[j]:
                j += 1
            cols.append((r, j - i, float(fin[y[i:j], x[i:j]].max()), int(x[(i + j) // 2]), int(y[(i + j) // 2]))); i = j
        else:
            i += 1
cols = np.array(cols); L = cols[:, 1]; dep = cols[:, 2]
print(f'{D.rstrip("/").split("/")[-1]}: {len(cols)} cols on {len(np.unique(cols[:, 0]))} reaches, {int(L.sum())} raster cells ({L.sum() * cell / 1000:.0f} km)')
for t in (10, 25, 50, 100, 150):
    s = dep > t
    print(f'  deepest final cut over {t:3d} m: {s.sum():5d} cols on {len(np.unique(cols[s, 0])):4d} reaches ({int(L[s].sum())} raster cells)')
np.savetxt(OUT, cols, fmt=['%d', '%d', '%.1f', '%d', '%d'], delimiter=',', comments='',
           header='reach,run_cells,deepest_cut_m,x,y (window L; curve raster runs KH cuts deeper than max(the defaults KH lowering, 10 m), S0 - S_KH)')
