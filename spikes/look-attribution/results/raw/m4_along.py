"""SPIKE-143 / #177a, M4 (registered before the variant runs): kinks along the channel direction. A straight valley along an H1 diagonal has its kinks parallel
to the channel (across it), so its second difference along the channel is near 0; triangle facets cut the channel and give a large one.
Lines: every H1 channel cell (h1-rivers.u8) and its D8 receiver by steepest descent on z_s (distance-weighted), each as a segment between the two H1 cell centres
(L cell 8 i + 3.5). Cells: the L land cells inside the box within 4 L cells of a line, each taking the direction t of its nearest line.
k(p) = z(p + t) - 2 z(p) + z(p - t), bilinear samples at one L cell along t. M4 = the RMS of k over those cells, and its p90 of |k|.
Usage: m4_along.py <chain dir> <stage>..."""
import sys, numpy as np
from scipy import ndimage
D = sys.argv[1]
n, m, nh, L = 8160, 1600, 1020, 8
zs = np.fromfile(f'{D}/zs.f32', '<f4').reshape(nh, nh).astype(np.float64)
riv = np.fromfile(f'{D}/h1-rivers.u8', 'u1').reshape(nh, nh) > 0
offs = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
pad = np.pad(zs, 1, constant_values=np.inf)
best = np.full(zs.shape, -1); bd = np.zeros(zs.shape)
for k, (dx, dy) in enumerate(offs):
    dr = (zs - pad[1 + dy:1 + dy + nh, 1 + dx:1 + dx + nh]) / (np.sqrt(2) if dx and dy else 1.0)
    bt = dr > bd
    best[bt] = k; bd[bt] = dr[bt]
cy, cx = np.nonzero(riv & (best >= 0))
ox = np.array([offs[k][0] for k in best[cy, cx]]); oy = np.array([offs[k][1] for k in best[cy, cx]])
# segments in L coordinates
ax, ay = cx * L + 3.5, cy * L + 3.5
bx, by = ax + ox * L, ay + oy * L
dist = np.full((n, n), np.inf); tx = np.zeros((n, n)); ty = np.zeros((n, n)); uu = np.zeros((n, n))
R = 4
for i in range(len(ax)):
    x0, x1 = int(max(m, min(ax[i], bx[i]) - R)), int(min(n - m - 1, max(ax[i], bx[i]) + R)) + 1
    y0, y1 = int(max(m, min(ay[i], by[i]) - R)), int(min(n - m - 1, max(ay[i], by[i]) + R)) + 1
    if x0 >= x1 or y0 >= y1:
        continue
    yy, xx = np.mgrid[y0:y1, x0:x1]
    dx, dy = bx[i] - ax[i], by[i] - ay[i]
    u = np.clip(((xx - ax[i]) * dx + (yy - ay[i]) * dy) / (dx * dx + dy * dy), 0, 1)
    d = np.hypot(xx - ax[i] - u * dx, yy - ay[i] - u * dy)
    sub = dist[y0:y1, x0:x1]
    better = (d < sub) & (d <= R)
    ln = np.hypot(dx, dy)
    sub[better] = d[better]
    tx[y0:y1, x0:x1][better] = dx / ln; ty[y0:y1, x0:x1][better] = dy / ln; uu[y0:y1, x0:x1][better] = u[better]
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n)
sel = np.isfinite(dist) & (s0 > 0)
py, px = np.nonzero(sel)
t_x, t_y = tx[py, px], ty[py, px]; u_ = uu[py, px]; mid = (u_ > 0.3) & (u_ < 0.7)
print(f'{len(ax)} channel D8 steps; {len(px)} L cells within {R} cells of a line')
for st in sys.argv[2:]:
    z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n)
    f = lambda sx, sy: ndimage.map_coordinates(z, [py + sy, px + sx], order=1)
    for H in (2, 4):
        kh_ = f(H * t_x, H * t_y) - 2 * z[py, px].astype(np.float64) + f(-H * t_x, -H * t_y)
        kc_ = f(-H * t_y, H * t_x) - 2 * z[py, px].astype(np.float64) + f(H * t_y, -H * t_x)
        print(f'{st:7s}   stencil {H} L cells: along RMS {np.sqrt(np.mean(kh_ * kh_)):.3f} m, across RMS {np.sqrt(np.mean(kc_ * kc_)):.3f} m, p90 |along| {np.percentile(np.abs(kh_), 90):.3f} m')
    k = f(t_x, t_y) - 2 * z[py, px].astype(np.float64) + f(-t_x, -t_y)
    kc = f(-t_y, t_x) - 2 * z[py, px].astype(np.float64) + f(t_y, -t_x)
    ra = np.sqrt(np.mean(k * k)) / np.sqrt(np.mean(kc * kc))
    mr = np.mean(np.abs(k[mid])) / np.mean(np.abs(k[~mid]))
    print(f'{st:7s} M4 along-channel kink RMS {np.sqrt(np.mean(k * k)):.3f} m (p90 {np.percentile(np.abs(k), 90):.3f}); across RMS {np.sqrt(np.mean(kc * kc)):.3f} m; along / across {ra:.3f}; mid-segment / ends |k| {mr:.3f}')
