"""SPIKE-143, M3's GLO-30 reference: on each tile (3 x 3 block means of the 1 arc-second grid, about 93 m by 93 cos(lat) m), priority-flood filled D8 channels with over
75 km2 of area, cut at confluences into reaches. Reported: the share of channel length (over 75 km2) in runs of one repeated step direction longer than 2 km, and the
sinuosity (path / chord) of reaches over 2 km (p50 and the length-weighted mean), as m2_m3.py does on the graph. Usage: glo30_m3.py <tif>..."""
import sys, heapq, numpy as np
import rasterio
offs = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
tot_len = tot_st = 0.0
all_sin, all_w = [], []
for p in sys.argv[1:]:
    with rasterio.open(p) as src:
        a = src.read(1).astype(np.float64)
    lat = float(p.split('_N')[1][:2]) + 0.5 if '_N' in p else -(float(p.split('_S')[1][:2]) - 0.5)
    h, w = (a.shape[0] // 3) * 3, (a.shape[1] // 3) * 3
    z = a[:h, :w].reshape(h // 3, 3, w // 3, 3).mean(axis=(1, 3))
    H, W = z.shape
    dy_m = 111320.0 / (a.shape[0] / 3)
    dx_m = 111320.0 * np.cos(np.radians(lat)) / (a.shape[1] / 3)
    # priority flood from the tile edge (epsilon fill)
    filled = z.copy(); done = np.zeros(z.shape, bool); hp = []
    for y in range(H):
        for x in (0, W - 1):
            heapq.heappush(hp, (z[y, x], y, x)); done[y, x] = True
    for x in range(1, W - 1):
        for y in (0, H - 1):
            heapq.heappush(hp, (z[y, x], y, x)); done[y, x] = True
    order = []
    while hp:
        e, y, x = heapq.heappop(hp)
        order.append(y * W + x)
        for ox, oy in offs:
            ny, nx = y + oy, x + ox
            if 0 <= ny < H and 0 <= nx < W and not done[ny, nx]:
                done[ny, nx] = True
                v = max(z[ny, nx], e + 1e-4)
                filled[ny, nx] = v
                heapq.heappush(hp, (v, ny, nx))
    f = filled.ravel()
    # D8 receivers on the filled surface (steepest drop by distance)
    rec = np.full(H * W, -1, np.int64); best = np.zeros(H * W)
    yy, xx = np.divmod(np.arange(H * W), W)
    for ox, oy in offs:
        ny, nx = yy + oy, xx + ox
        ok = (ny >= 0) & (ny < H) & (nx >= 0) & (nx < W)
        q = np.where(ok, ny * W + nx, 0)
        dist = np.hypot(ox * dx_m, oy * dy_m)
        drop = np.where(ok, (f - f[q]) / dist, -np.inf)
        better = drop > best
        rec[better] = q[better]; best[better] = drop[better]
    area = np.full(H * W, dx_m * dy_m)
    for c in reversed(order):  # highest first
        r = rec[c]
        if r >= 0:
            area[r] += area[c]
    ch = area > 75e6
    inflow = np.zeros(H * W, np.int32)
    for c in np.nonzero(ch)[0]:
        r = rec[c]
        if r >= 0 and ch[r]:
            inflow[r] += 1
    # reaches: start at heads (no channel inflow) and confluences (2+), follow receivers to the next confluence or the end
    starts = np.nonzero(ch & ((inflow == 0) | (inflow >= 2)))[0]
    tl = ts = 0.0
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
        seg = np.hypot(ddx * dx_m, ddy * dy_m)
        code = (ddx + 1) * 3 + (ddy + 1)
        rid = np.concatenate([[0], np.cumsum(code[1:] != code[:-1])])
        rl = np.bincount(rid, weights=seg)
        st = rl[rid] > 2000.0
        tl += seg.sum(); ts += seg[st].sum()
        L = seg.sum()
        if L > 2000.0:
            chord = np.hypot((px[-1] - px[0]) * dx_m, (py[-1] - py[0]) * dy_m)
            if chord > 0:
                all_sin.append(L / chord); all_w.append(L)
    tot_len += tl; tot_st += ts
    print(f"{p.split('/')[-1][:40]:40s} channel length {tl / 1000:7.0f} km, in straight runs over 2 km {ts / max(tl, 1) * 100:5.1f} %")
s, wgt = np.array(all_sin), np.array(all_w)
print(f'all tiles: straight-run share {tot_st / tot_len * 100:.1f} %; sinuosity of reaches over 2 km ({len(s)}): p50 {np.median(s):.3f}, length-weighted mean {np.sum(s * wgt) / np.sum(wgt):.3f}')
