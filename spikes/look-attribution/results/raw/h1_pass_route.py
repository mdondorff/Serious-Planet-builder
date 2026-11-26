"""Lead #184s: a dry test of option 6 (row3-cols-design-2026-11-25.md): route H1 on L's lowest passes instead of z_s, on a step-1 chain's kept dumps. No chain run.
Two H1 drainage trees by the same priority flood from the sea and the window's edge (ties by cell index), each H1 cell's receiver the cell that reached it:
- (A) on z_s (zs.f32): entering b from a sets b's level to max(level(a), z_s(b));
- (B) on L's passes: b's height is its lowest L cell (S0), and the edge a-b is the lowest crossing between their 8 x 8 L blocks (for side neighbours the lowest
  max(S0) over L pairs across the shared border, 8-connected; for corner neighbours the corner pair); entering b from a sets level max(level(a), pass(a, b)).
Drained area: H1 cells of land (more than half their L cells above the sea), each 0.663 km2; trunks are cells draining 75 km2 or more.
For both trees in L's terms: a cell's spill level is the highest pass on its path to the outlet (B: its flood level; A: the passes along A's own receivers), and its
depth is that level minus its lowest L cell: how far the cell would have to be cut, or flooded, for water to leave by that path. A deep trunk cell (depth over 50 m)
is the H1 form of a deep col.
Compared: trunk cells and receivers (changed cells, km of changed trunk), deep trunk cells under A and B, the step-1 chain's deep cols (row3-cols.csv over 50 m) by
their H1 cell (removed when that cell is no longer a B trunk or its B receiver differs from the chain's own direction there), basin flips (land whose outlet changes),
and an agreement proxy (the share of A's trunk cells that B keeps with the same receiver).
Usage: h1_pass_route.py <chain dir> <trunk-curves.bin> <row3-cols.csv> <out prefix>"""
import sys, struct, heapq, numpy as np, numba
D, CB, CSV, OUT = sys.argv[1:5]
n, L, nh, cell = 8160, 8, 1020, 101.802
A1 = (L * cell) ** 2 / 1e6
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n).astype(np.float64)
zs = np.fromfile(f'{D}/zs.f32', '<f4').reshape(nh, nh).astype(np.float64)
blk = s0.reshape(nh, L, nh, L)
hmin = blk.min(axis=(1, 3))
seafrac = (blk <= 0.0).mean(axis=(1, 3))
sea = seafrac > 0.5
m_h = 1600 // L  # the box margin in H1 cells


@numba.njit
def passes(s0, nh, L):
    # pe[j, i]: east edge (i, j)-(i+1, j); ps: south edge (i, j)-(i, j+1); pse: (i, j)-(i+1, j+1); psw: (i, j)-(i-1, j+1)
    pe = np.full((nh, nh), np.inf); ps = np.full((nh, nh), np.inf); pse = np.full((nh, nh), np.inf); psw = np.full((nh, nh), np.inf)
    for j in range(nh):
        for i in range(nh):
            if i + 1 < nh:
                xa = L * i + L - 1; xb = xa + 1; best = np.inf
                for y in range(L * j, L * j + L):
                    for dy in (-1, 0, 1):
                        yb = y + dy
                        if L * j <= yb < L * j + L:
                            v = max(s0[y, xa], s0[yb, xb])
                            if v < best:
                                best = v
                pe[j, i] = best
            if j + 1 < nh:
                ya = L * j + L - 1; yb = ya + 1; best = np.inf
                for x in range(L * i, L * i + L):
                    for dx in (-1, 0, 1):
                        xb = x + dx
                        if L * i <= xb < L * i + L:
                            v = max(s0[ya, x], s0[yb, xb])
                            if v < best:
                                best = v
                ps[j, i] = best
            if i + 1 < nh and j + 1 < nh:
                pse[j, i] = max(s0[L * j + L - 1, L * i + L - 1], s0[L * j + L, L * i + L])
            if i - 1 >= 0 and j + 1 < nh:
                psw[j, i] = max(s0[L * j + L - 1, L * i], s0[L * j + L, L * i - 1])
    return pe, ps, pse, psw


@numba.njit
def edge(pe, ps, pse, psw, j, i, dj, di):
    if dj == 0:
        return pe[j, min(i, i + di)]
    if di == 0:
        return ps[min(j, j + dj), i]
    if dj == di:
        return pse[min(j, j + dj), min(i, i + di)]
    # dj = -di: the south-west / north-east diagonal
    if dj == 1:
        return psw[j, i]
    return psw[j + dj, i + di]


@numba.njit
def flood(height, use_pass, pe, ps, pse, psw, sea):
    nh = height.shape[0]
    level = np.full((nh, nh), np.inf)
    rec = np.full(nh * nh, -1, np.int64)
    outlet = np.full(nh * nh, -1, np.int64)
    order = np.empty(nh * nh, np.int64); k = 0
    hp = [(0.0, 0)]
    hp.pop()
    for j in range(nh):
        for i in range(nh):
            if sea[j, i] or i == 0 or j == 0 or i == nh - 1 or j == nh - 1:
                c = j * nh + i
                level[j, i] = height[j, i]
                outlet[c] = c
                heapq.heappush(hp, (height[j, i], c))
    done = np.zeros(nh * nh, np.bool_)
    while len(hp) > 0:
        e, c = heapq.heappop(hp)
        if done[c]:
            continue
        done[c] = True
        order[k] = c; k += 1
        j, i = c // nh, c % nh
        for dj in (-1, 0, 1):
            for di in (-1, 0, 1):
                if dj == 0 and di == 0:
                    continue
                jj, ii = j + dj, i + di
                if 0 <= jj < nh and 0 <= ii < nh:
                    q = jj * nh + ii
                    if done[q]:
                        continue
                    v = max(e, edge(pe, ps, pse, psw, j, i, dj, di)) if use_pass else max(e, height[jj, ii])
                    if v < level[jj, ii]:
                        level[jj, ii] = v
                        rec[q] = c
                        outlet[q] = outlet[c]
                        heapq.heappush(hp, (v, q))
    return level, rec, outlet, order[:k]


@numba.njit
def l_levels(rec, order, pe, ps, pse, psw, hmin, nh):
    # in L's terms: a cell's spill level along its own receivers (the highest pass to the outlet)
    lev = np.full(nh * nh, np.inf)
    for t in range(len(order)):
        c = order[t]
        r = rec[c]
        if r < 0:
            lev[c] = hmin[c // nh, c % nh]
        else:
            j, i = c // nh, c % nh
            rj, ri = r // nh, r % nh
            lev[c] = max(lev[r], edge(pe, ps, pse, psw, j, i, rj - j, ri - i))
    return lev


pe, ps, pse, psw = passes(s0, nh, L)
res = {}
for name, h, use in (('A (z_s)', zs, False), ('B (L passes)', hmin, True)):
    level, rec, outlet, order = flood(h, use, pe, ps, pse, psw, sea)
    area = np.where(sea.ravel(), 0.0, A1)
    for t in range(len(order) - 1, -1, -1):
        c = order[t]
        if rec[c] >= 0:
            area[rec[c]] += area[c]
    lev = l_levels(rec, order, pe, ps, pse, psw, hmin, nh)
    depth = lev - hmin.ravel()
    res[name] = (rec, outlet, area, depth)
ga = np.zeros((nh, nh), bool); ga[m_h:nh - m_h, m_h:nh - m_h] = True; ga = ga.ravel() & ~sea.ravel()
(ra, oa, aa, da), (rb, ob, ab, db) = res['A (z_s)'], res['B (L passes)']
ta = (aa >= 75.0) & ga; tb = (ab >= 75.0) & ga
riv = np.fromfile(f'{D}/h1-rivers.u8', 'u1').reshape(nh, nh).ravel() > 0
name = D.rstrip('/').split('/')[-1]
print(f'{name}: land H1 cells in the box {ga.sum()}; trunk cells (>= 75 km2): A {ta.sum()}, B {tb.sum()}; the chain\'s own H1 channel cells there {(riv & ga).sum()}, '
      f'of them A trunks {100 * np.mean(ta[riv & ga]):.1f} %')
both = ta & tb
same = both & (ra == rb)
print(f'  agreement proxy: of A\'s trunk cells, B keeps {100 * both.sum() / ta.sum():.1f} % as trunks and {100 * same.sum() / ta.sum():.1f} % with the same receiver')
chg = (ta | tb) & ~((ta & tb) & (ra == rb))
lenkm = 0.0
for c in np.nonzero(chg)[0]:
    r = rb[c] if tb[c] else ra[c]
    if r >= 0:
        dj, di = r // nh - c // nh, r % nh - c % nh
        lenkm += L * cell / 1000 * (1.4142 if dj and di else 1.0)
print(f'  changed trunk cells (in one tree only, or a different receiver): {chg.sum()} ({lenkm:.0f} km of H1 trunk)')
for t in (10, 50):
    print(f'  deep trunk cells (depth over {t} m, in L terms): A {int((ta & (da > t)).sum())}, B {int((tb & (db > t)).sum())}; '
          f'B\'s deep cells that are also A\'s {int((ta & tb & (da > t) & (db > t)).sum())}')
flip = ga & (oa != ob)
print(f'  basin flips: {flip.sum()} land H1 cells ({100 * flip.sum() / ga.sum():.1f} % of land, {flip.sum() * A1:.0f} km2) drain to another outlet under B')
far = flip & (np.hypot(oa // nh - ob // nh, oa % nh - ob % nh) > 10)
print(f'  of them, outlets more than 10 H1 cells (8 km) apart: {far.sum()} cells ({100 * far.sum() / ga.sum():.1f} % of land, {far.sum() * A1:.0f} km2); of the trunk cells {100 * np.mean(far[ta | tb]):.1f} %')
# the chain's deep cols by H1 cell
cols = np.loadtxt(CSV, delimiter=',', skiprows=1)
deep = cols[cols[:, 2] > 50]
hc = (deep[:, 4] // L).astype(int) * nh + (deep[:, 3] // L).astype(int)
# the chain's own direction at the col: the curve's next H1 cell downstream of the col centre
b = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; ras = {}
for _ in range(cnt):
    r, npt = struct.unpack_from('<II', b, k); k += 8 + 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4
    ras[r] = np.frombuffer(b, np.dtype([('c', '<u4'), ('o', 'u1', 2)]), nr, k)['c'].astype(np.int64); k += 6 * nr
removed = 0; nottrunk = 0; turned = 0; rows = []
for (reach, run, dep, x, y), h in zip(deep, hc):
    rc = ras[int(reach)]; cc = int(y) * n + int(x)
    idx = np.nonzero(rc == cc)[0]
    nxt = -1
    if len(idx):
        for q in rc[idx[0]:]:
            hq = (q // n // L) * nh + (q % n) // L
            if hq != h:
                nxt = hq; break
    gone = (not tb[h]) or (nxt >= 0 and rb[h] != nxt)
    removed += gone; nottrunk += (not tb[h]); turned += (tb[h] and nxt >= 0 and rb[h] != nxt)
    rows.append((int(reach), int(x), int(y), h % nh, h // nh, dep, int(gone), float(db[h])))
print(f'  the chain\'s cols over 50 m: {len(deep)} in {len(np.unique(hc))} H1 cells; removed under B {removed} ({nottrunk} no longer a trunk, {turned} drain another way), remaining {len(deep) - removed}')
np.savetxt(f'{OUT}-deep-cols-h1.csv', np.array(rows), fmt=['%d', '%d', '%d', '%d', '%d', '%.1f', '%d', '%.1f'], delimiter=',', comments='',
           header='reach,x,y (window L),h1_i,h1_j,deepest_cut_m,removed_under_B,B_depth_m_at_the_H1_cell')
