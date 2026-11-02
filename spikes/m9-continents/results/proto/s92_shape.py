"""SPIKE-92 round 2: S9 compactness and S10 elongation per land mass of at least 1 Mkm2 (1440x720 mask, row 0 north).
S9: 4 pi A / P^2, A from the 0.25 degree cells, P the contour length (marching squares at 0.5) of the mass's land fraction on a 1 degree grid (about 111 km), E-W
steps scaled by cos(latitude); a disc of the same method scores about 1 (printed by `disc`). S10: sqrt of the eigenvalue ratio of the area-weighted second moment in the tangent plane at the
mass's mean direction. S9 is summarized as the area-weighted median over the masses, S10 as the plain median."""
import json, sys
import numpy as np
from s92_score import wrap_label, cell, H, W, lat, R
lon = -180 + (np.arange(W) + 0.5) * 0.25
cl, sl = np.cos(np.radians(lat))[:, None], np.sin(np.radians(lat))[:, None]
X = cl * np.cos(np.radians(lon))[None, :]; Y = cl * np.sin(np.radians(lon))[None, :]; Z = sl * np.ones((1, W))
D = R * np.radians(1.0)
lat1 = 90 - (np.arange(180) + 0.5)

def perimeter(m):
    """Contour length (marching squares at 0.5) of the mass's 1-degree land fraction, the grid padded by wrap in longitude."""
    from skimage.measure import find_contours
    f = m.reshape(180, 4, 360, 4).mean(axis=(1, 3))
    f = np.pad(f, ((1, 1), (0, 0)))  # sea rows beyond the poles close the contours
    j = np.argmin(f.sum(axis=0)); f = np.roll(f, -j, axis=1)  # cut at the emptiest meridian
    f = np.pad(f, ((0, 0), (1, 1)))
    p = 0.0
    for c in find_contours(f, 0.5):
        y, x = c[:, 0] - 1, c[:, 1]
        la = np.radians(90 - (0.5 * (y[1:] + y[:-1]) + 0.5))
        p += float(np.sum(D * np.hypot(np.diff(y), np.diff(x) * np.cos(la))))
    return p

def wmedian(v, w):
    o = np.argsort(v); v, w = np.asarray(v)[o], np.asarray(w)[o]
    return float(v[np.searchsorted(np.cumsum(w), 0.5 * w.sum())])

def shape(land):
    lab = wrap_label(land); out = []
    for r in np.unique(lab):
        if not r: continue
        m = lab == r; a = float(cell[m].sum())
        if a < 1e6: continue
        p = perimeter(m)
        w = cell[m]; v = np.stack([X[m], Y[m], Z[m]], 1)
        n = (v * w[:, None]).sum(0); n /= np.linalg.norm(n)
        e1 = np.cross(n, [0, 0, 1.0] if abs(n[2]) < 0.9 else [1.0, 0, 0]); e1 /= np.linalg.norm(e1); e2 = np.cross(n, e1)
        u, t = v @ e1, v @ e2
        cov = np.cov(np.stack([u, t]), aweights=w)
        ev = np.sort(np.linalg.eigvalsh(cov))
        out.append(dict(area=round(a / 1e6, 2), s9=4 * np.pi * a / p ** 2, s10=float(np.sqrt(ev[1] / ev[0]))))
    ws = [o['area'] for o in out]
    return dict(s9=wmedian([o['s9'] for o in out], ws), s10=float(np.median([o['s10'] for o in out])), masses=out)

def disc_ref(area_km2=1e7):
    r = np.sqrt(area_km2 / np.pi) / R
    m = (np.arccos(np.clip(X * 1 + 0, -1, 1)) < r)  # a cap centred on the equator at lon 0
    return shape(m)['s9']

if __name__ == '__main__':
    if sys.argv[1] == 'disc':
        print(json.dumps({k: disc_ref(k) for k in (2e6, 1e7, 4e7)})); sys.exit()
    print(json.dumps(shape(np.fromfile(sys.argv[1], dtype=np.uint8).reshape(H, W) > 0), default=float))
