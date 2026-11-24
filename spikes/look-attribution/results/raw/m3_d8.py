"""Lead #184: M3 on a chain surface by glo30_m3.py's own method, so the window's straight runs and sinuosity compare with GLO-30's like for like. The surface's box (the
window less the 1600-cell margin), priority-flood filled from the box's edge (epsilon 1e-4 m), D8 receivers by the steepest drop over distance, channels over 75 km2 cut
at confluences into reaches; the share of channel length in runs of one repeated step direction longer than 2 km, and the sinuosity (path / chord) of reaches over 2 km.
Also, with a curves file (rg_chain's <tag>-trunk-curves.bin), the curves' own sinuosity and straight share (a run is straight while its heading stays within 5 degrees of
the run's start heading, over 2 km). Usage: m3_d8.py <chain dir> <stage> [curves.bin]"""
import sys, struct, numpy as np
D, st = sys.argv[1], sys.argv[2]
n, m, dx = 8160, 1600, 101.802
z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n).astype(np.float64)[m:n - m, m:n - m]
sea = z <= 0.0
H, W = z.shape
import numba, heapq


@numba.njit(cache=True)
def flood_d8(z, sea, dx):
    H, W = z.shape
    filled = z.copy()
    done = np.zeros((H, W), np.bool_)
    hp = [(0.0, 0, 0)]
    hp.pop()
    for y in range(H):
        for x in range(W):
            if y == 0 or x == 0 or y == H - 1 or x == W - 1 or sea[y, x]:
                heapq.heappush(hp, (z[y, x], y, x))
                done[y, x] = True
    order = np.empty(H * W, np.int64)
    k = 0
    offs = ((-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1))
    while len(hp) > 0:
        e, y, x = heapq.heappop(hp)
        order[k] = y * W + x
        k += 1
        for ox, oy in offs:
            ny, nx = y + oy, x + ox
            if 0 <= ny < H and 0 <= nx < W and not done[ny, nx]:
                done[ny, nx] = True
                v = max(z[ny, nx], e + 1e-4)
                filled[ny, nx] = v
                heapq.heappush(hp, (v, ny, nx))
    f = filled.ravel()
    rec = np.full(H * W, -1, np.int64)
    for c in range(H * W):
        y, x = c // W, c % W
        if sea[y, x]:
            continue
        best = 0.0
        for ox, oy in offs:
            ny, nx = y + oy, x + ox
            if 0 <= ny < H and 0 <= nx < W:
                q = ny * W + nx
                d = (f[c] - f[q]) / (dx * np.sqrt(float(ox * ox + oy * oy)))
                if d > best:
                    best = d
                    rec[c] = q
    area = np.full(H * W, dx * dx)
    for i in range(k - 1, -1, -1):
        c = order[i]
        r = rec[c]
        if r >= 0:
            area[r] += area[c]
    return rec, area


rec, area = flood_d8(z, sea, dx)
ch = (area > 75e6) & ~sea.ravel()
inflow = np.zeros(H * W, np.int32)
for c in np.nonzero(ch)[0]:
    r = rec[c]
    if r >= 0 and ch[r]:
        inflow[r] += 1
starts = np.nonzero(ch & ((inflow == 0) | (inflow >= 2)))[0]
tl = ts = 0.0
sins, ws = [], []
for s in starts:
    path = [s]; c = s
    while True:
        r = rec[c]
        if r < 0 or not ch[r]:
            break
        path.append(r)
        if inflow[r] >= 2:
            break
        c = r
    if len(path) < 2:
        continue
    py, px = np.divmod(np.array(path), W)
    ddx, ddy = np.diff(px), np.diff(py)
    seg = np.hypot(ddx * dx, ddy * dx)
    code = (ddx + 1) * 3 + (ddy + 1)
    rid = np.concatenate([[0], np.cumsum(code[1:] != code[:-1])])
    rl = np.bincount(rid, weights=seg)
    stt = rl[rid] > 2000.0
    tl += seg.sum(); ts += seg[stt].sum()
    L = seg.sum()
    if L > 2000.0:
        chord = np.hypot((px[-1] - px[0]) * dx, (py[-1] - py[0]) * dx)
        if chord > 0:
            sins.append(L / chord); ws.append(L)
s, wg = np.array(sins), np.array(ws)
print(f'{st} D8 (GLO-30 method): channel length {tl / 1000:.0f} km, in straight runs over 2 km {ts / max(tl, 1) * 100:.1f} %; sinuosity of reaches over 2 km ({len(s)}): '
      f'p50 {np.median(s):.3f}, length-weighted mean {np.sum(s * wg) / np.sum(wg):.3f}')
if len(sys.argv) > 3:
    b = open(sys.argv[3], 'rb').read()
    assert b[:4] == b'TRC1'
    k = 8; cnt = struct.unpack_from('<I', b, 4)[0]
    csin, cw, clen, cst = [], [], 0.0, 0.0
    for _ in range(cnt):
        _r, npt = struct.unpack_from('<II', b, k); k += 8
        p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
        nr = struct.unpack_from('<I', b, k)[0]; k += 4 + 6 * nr
        inb = (p[:, 0] >= m) & (p[:, 0] < n - m) & (p[:, 1] >= m) & (p[:, 1] < n - m)
        if inb.mean() < 0.5 or npt < 3:
            continue
        d = np.diff(p, axis=0); sl = np.hypot(d[:, 0], d[:, 1]) * dx
        L = sl.sum(); clen += L
        chord = np.hypot(*(p[-1] - p[0])) * dx
        if chord > 2000.0:
            csin.append(L / chord); cw.append(L)
        hd = np.degrees(np.arctan2(d[:, 1], d[:, 0]))
        i = 0
        while i < len(hd):
            j = i; run = 0.0
            while j < len(hd) and abs((hd[j] - hd[i] + 180) % 360 - 180) <= 5.0:
                run += sl[j]; j += 1
            if run > 2000.0:
                cst += run
            i = max(j, i + 1)
    cs, cwg = np.array(csin), np.array(cw)
    print(f'curves: length {clen / 1000:.0f} km, straight (heading within 5 degrees) over 2 km {cst / max(clen, 1) * 100:.1f} %; sinuosity over 2 km ({len(cs)}): '
          f'p50 {np.median(cs):.3f}, length-weighted mean {np.sum(cs * cwg) / np.sum(cwg):.3f}')
