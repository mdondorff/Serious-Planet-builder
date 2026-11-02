"""SPIKE-92: S1-S3 and the land fraction of a preview land mask (1440 x 720 bytes, row 0 north; SPIKE-91's definitions)."""
import json, sys
import numpy as np
from scipy import ndimage
H, W, R = 720, 1440, 6371.0
lat = 90 - (np.arange(H) + 0.5) * 0.25
cell = (R * np.radians(0.25)) ** 2 * np.cos(np.radians(lat))[:, None] * np.ones((1, W))
def wrap_label(m):
    lab, n = ndimage.label(m, structure=np.ones((3, 3)))
    parent = list(range(n + 1))
    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]; i = parent[i]
        return i
    for y in range(H):
        for yy in (y - 1, y, y + 1):
            if 0 <= yy < H and lab[y, 0] and lab[yy, W - 1]:
                a, b = find(lab[y, 0]), find(lab[yy, W - 1])
                if a != b: parent[a] = b
    return np.array([find(i) for i in range(n + 1)])[lab]
def coast_dim(land):
    coast = land & ~ndimage.binary_erosion(land, structure=np.ones((3, 3)))
    sizes, counts = [2, 4, 8, 16, 32], []
    for s in sizes:
        counts.append(coast[:H // s * s, :W // s * s].reshape(H // s, s, W // s, s).any(axis=(1, 3)).sum())
    return float(-np.polyfit(np.log(sizes), np.log(counts), 1)[0])
def score(path):
    land = np.fromfile(path, dtype=np.uint8).reshape(H, W) > 0
    la = float(cell[land].sum()); lab = wrap_label(land)
    areas = sorted((float(cell[lab == r].sum()) for r in np.unique(lab) if r), reverse=True)
    return dict(land=la / float(cell.sum()), s1=sum(a >= 1e6 for a in areas), s2=areas[0] / la, s3=coast_dim(land), top=[round(a / 1e6, 1) for a in areas[:7]])
if __name__ == '__main__':
    print(json.dumps(score(sys.argv[1])))
