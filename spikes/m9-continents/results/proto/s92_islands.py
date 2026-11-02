"""SPIKE-92 round 3: S11 islands (masses of 1e4 to 1e6 km2: count and share of land) and polar masses (masses of at least 1 Mkm2 whose
area-weighted mean direction lies poleward of 70 degrees) of a 1440x720 land mask, row 0 north."""
import json, sys
import numpy as np
from s92_score import wrap_label, cell, H, W
from s92_shape import X, Y, Z

def islands(land):
    lab = wrap_label(land)
    ids, inv = np.unique(lab, return_inverse=True)
    areas = np.bincount(inv.ravel(), weights=cell.ravel())
    total = float(cell[land].sum())
    n, a_isl, polar, polar_lat = 0, 0.0, 0, []
    for k, r in enumerate(ids):
        if not r: continue
        a = areas[k]
        if 1e4 <= a < 1e6:
            n += 1; a_isl += a
        elif a >= 1e6:
            m = (inv.reshape(lab.shape) == k); w = cell[m]
            v = np.array([(X[m] * w).sum(), (Y[m] * w).sum(), (Z[m] * w).sum()]); v /= np.linalg.norm(v)
            lat = float(np.degrees(np.arcsin(v[2])))
            if abs(lat) >= 70: polar += 1; polar_lat.append(round(lat, 1))
    return dict(s11=n, s11_share=a_isl / total, polar=polar, polar_lat=polar_lat)

if __name__ == '__main__':
    print(json.dumps(islands(np.fromfile(sys.argv[1], dtype=np.uint8).reshape(H, W) > 0)))
