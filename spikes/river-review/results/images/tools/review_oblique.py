"""River review (lead #118): low-sun obliques of the start surfaces (no bake) at a stills pose, by ray marching the probe's full-resolution dump.
Camera as `planet test-render --scene capture-stills` (stills.rs pose_camera, Camera::at_surface_point; fov 60 deg vertical), the sun at the pose's local solar
time (declination 0), Lambert shading with cast shadows, distance haze, river cells blue. R1h window only (face 1, seed 14, i 8056-8675, j 760-1379: the E1 region) and its margin.
Usage: review_oblique.py <poses.toml> <tag-suffix|''> <label> <out.png> [width height]
"""
import sys, math
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

T = 'C:/DEV/Serious-Planet-Builder/.claude/worktrees/sonnet-spikes-36fc88/target/river-review/'
BASE = 'f1-8056-760-none'
R = 6_371_000.0
NF = 98_304  # L cells a face edge
X0, Y0, N = (8056 - 200) * 8, (760 - 200) * 8, 8160  # the whole window's first L cell on face 1 and its size (PLANET_PICS=window)
Q = math.pi / 4


def parse_toml(path):
    """The two tables of a poses file ([place] and [[pose]], plain key = number or string lines)."""
    cfg, cur = {'place': {}, 'pose': []}, None
    for line in open(path, encoding='utf-8'):
        line = line.split('#')[0].strip()
        if not line:
            continue
        if line == '[place]':
            cur = cfg['place']
        elif line == '[[pose]]':
            cur = {}; cfg['pose'].append(cur)
        else:
            k, v = [a.strip() for a in line.split('=', 1)]
            cur[k] = v.strip('"') if v.startswith('"') else float(v)
    return cfg


def ll_vec(lat, lon):
    return np.array([math.cos(lat) * math.cos(lon), math.cos(lat) * math.sin(lon), math.sin(lat)])


def cell_of(p):
    """Planet-fixed points (..., 3) on face 1 (-X: normal (-1,0,0), U (0,-1,0), V (0,0,1)) to fractional core cell coordinates (x, y)."""
    w = -p[..., 0]
    u, v = -p[..., 1] / w, p[..., 2] / w
    s, t = np.arctan(u) / Q, np.arctan(v) / Q
    return (s + 1) / 2 * NF - 0.5 - X0, (t + 1) / 2 * NF - 0.5 - Y0


def bilinear(z, x, y):
    x = np.clip(x, 0, N - 1.001); y = np.clip(y, 0, N - 1.001)
    ix, iy = np.floor(x).astype(np.int64), np.floor(y).astype(np.int64)
    fx, fy = x - ix, y - iy
    a, b = z[iy, ix], z[iy, ix + 1]
    c, d = z[iy + 1, ix], z[iy + 1, ix + 1]
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy


def ground(z, p):
    x, y = cell_of(p)
    return bilinear(z, x, y)


def main():
    poses, suf, label, out = sys.argv[1:5]
    W, H = (int(sys.argv[5]), int(sys.argv[6])) if len(sys.argv) > 6 else (960, 540)
    cfg = parse_toml(poses)
    lat, lon = math.radians(cfg['place']['lat']), math.radians(cfg['place']['lon'])
    pose = cfg['pose'][0]
    z = np.fromfile(T + BASE + suf + '-full-8160x8160.f32', dtype='<f4').reshape(N, N).astype(np.float64)
    m = np.fromfile(T + BASE + suf + '-chan-8160x8160.u8', dtype=np.uint8).reshape(N, N)
    A = ll_vec(lat, lon)
    north = np.array([-math.sin(lat) * math.cos(lon), -math.sin(lat) * math.sin(lon), math.cos(lat)])
    east = np.array([-math.sin(lon), math.cos(lon), 0.0])
    hd = math.radians(pose['heading_deg'])
    head = north * math.cos(hd) + east * math.sin(hd)
    th = pose['back_m'] / R
    eye_dir = A * math.cos(th) - head * math.sin(th)
    eye_dir /= np.linalg.norm(eye_dir)
    eye_alt = max(float(ground(z, eye_dir * R)), 0.0) + pose['altitude_m']
    eye = eye_dir * (R + eye_alt)
    up = eye_dir
    ahead = A - eye_dir * A.dot(eye_dir); ahead /= np.linalg.norm(ahead)
    pr = math.radians(pose['pitch_deg'])
    fwd = ahead * math.cos(pr) - up * math.sin(pr)
    right = np.cross(fwd, up); right /= np.linalg.norm(right)
    cup = np.cross(right, fwd)
    fy = math.tan(math.radians(60.0) / 2)
    fx = fy * W / H
    jj, ii = np.meshgrid(np.arange(W), np.arange(H))
    sx = (2 * (jj + 0.5) / W - 1) * fx
    sy = (1 - 2 * (ii + 0.5) / H) * fy
    D = fwd[None, None, :] + sx[..., None] * right[None, None, :] + sy[..., None] * cup[None, None, :]
    D /= np.linalg.norm(D, axis=2, keepdims=True)
    D = D.reshape(-1, 3)
    n = D.shape[0]
    t = np.full(n, 1.0)
    hit = np.zeros(n, bool)
    alive = np.ones(n, bool)
    tmax = 40_000.0
    prev_t = np.zeros(n)
    while alive.any():
        idx = np.nonzero(alive)[0]
        p = eye[None, :] + D[idx] * t[idx, None]
        r = np.linalg.norm(p, axis=1)
        gz = np.maximum(ground(z, p / r[:, None] * R), 0.0)
        below = r - R <= gz
        # refine the hit by bisection between the previous and this step
        bi = idx[below]
        if bi.size:
            lo, hi = prev_t[bi], t[bi]
            for _ in range(12):
                mid = 0.5 * (lo + hi)
                pm = eye[None, :] + D[bi] * mid[:, None]
                rm = np.linalg.norm(pm, axis=1)
                inside = rm - R <= np.maximum(ground(z, pm / rm[:, None] * R), 0.0)
                hi = np.where(inside, mid, hi); lo = np.where(inside, lo, mid)
            t[bi] = hi
            hit[bi] = True
            alive[bi] = False
        rest = idx[~below]
        prev_t[rest] = t[rest]
        t[rest] += np.maximum(2.0, t[rest] * 0.004)
        alive[rest[t[rest] > tmax]] = False
    # shading
    ph = eye[None, :] + D * t[:, None]
    pr_ = ph / np.linalg.norm(ph, axis=1, keepdims=True) * R
    x, y = cell_of(pr_)
    e = 0.5
    zx = (bilinear(z, x + e, y) - bilinear(z, x - e, y)) / (2 * e * 101.8)
    zy = (bilinear(z, x, y + e) - bilinear(z, x, y - e)) / (2 * e * 101.8)
    # local tangent axes along the cell axes: +x ~ face U (0,-1,0) projected, +y ~ V (0,0,1) projected
    upv = pr_ / R
    ux = np.array([0.0, -1.0, 0.0])[None, :] - upv * (-upv[:, 1:2])
    ux /= np.linalg.norm(ux, axis=1, keepdims=True)
    uy = np.array([0.0, 0.0, 1.0])[None, :] - upv * upv[:, 2:3]
    uy /= np.linalg.norm(uy, axis=1, keepdims=True)
    nrm = upv - zx[:, None] * ux - zy[:, None] * uy
    nrm /= np.linalg.norm(nrm, axis=1, keepdims=True)
    hang = math.radians((pose['sun_local_hours'] - 12.0) * 15.0)
    dec = math.radians(pose.get('sun_declination_deg', 0.0))
    sun = ll_vec(dec, lon - hang)
    lam = np.clip((nrm * sun[None, :]).sum(1), 0, 1)
    # cast shadows: march toward the sun from the hit point
    sh = np.zeros(n, bool)
    cand = np.nonzero(hit & (lam > 0))[0]
    ts = np.full(cand.size, 20.0)
    act = np.ones(cand.size, bool)
    base_p = ph[cand] + upv[cand] * 0.5
    while act.any():
        k = np.nonzero(act)[0]
        p = base_p[k] + sun[None, :] * ts[k, None]
        r = np.linalg.norm(p, axis=1)
        gz = np.maximum(ground(z, p / r[:, None] * R), 0.0)
        blocked = r - R <= gz
        sh[cand[k[blocked]]] = True
        act[k[blocked]] = False
        ts[k] += np.maximum(10.0, ts[k] * 0.01)
        act[k[ts[k] > 30_000.0]] = False
    light = np.where(sh, 0.0, lam)
    sky_l = 0.5 + 0.5 * (nrm * upv).sum(1)  # sky light: how much a face looks up
    c = 0.30 * sky_l + 0.70 * light
    gzh = ground(z, pr_)
    xi, yi = np.clip(np.round(x).astype(int), 0, N - 1), np.clip(np.round(y).astype(int), 0, N - 1)
    base_col = np.stack([0.62 * c, 0.58 * c, 0.50 * c], 1)
    sea = gzh <= 0.0
    base_col[sea] = np.stack([0.20 + 0.3 * c[sea], 0.32 + 0.3 * c[sea], 0.50 + 0.3 * c[sea]], 1)
    riv = m[yi, xi].astype(bool) & ~sea
    base_col[riv] = np.stack([0.05 + 0.1 * c[riv], 0.25 + 0.2 * c[riv], 0.85 * (0.5 + 0.5 * c[riv])], 1)
    haze = 1 - np.exp(-t / 25_000.0)
    sky = np.array([0.72, 0.80, 0.90])
    col = base_col * (1 - haze[:, None]) + sky[None, :] * haze[:, None]
    col[~hit] = sky
    img = np.clip(col.reshape(H, W, 3), 0, 1)
    fig = plt.figure(figsize=(W / 100, H / 100 + 0.45), dpi=100)
    ax = fig.add_axes([0, 0, 1, H / (H + 45)])
    ax.imshow(img, interpolation='nearest'); ax.axis('off')
    fig.text(0.005, 0.995, ('%s | %s: %.0f m above ground, heading %.1f, %.1f deg down, sun %.1f h local |\nSTART SURFACE, NO BAKE (probe dump at 101.8 m, bilinear; '
             'Lambert + sky light + cast shadows, no materials, no detail; river cells blue)') % (label, pose['name'], pose['altitude_m'], pose['heading_deg'], pose['pitch_deg'], pose['sun_local_hours']),
             fontsize=6.5, va='top')
    fig.savefig(out, dpi=100)
    print('wrote', out, 'eye alt %.0f m, hit %.1f %%' % (eye_alt, 100 * hit.mean()))


main()
