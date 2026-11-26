"""Lead #184i item 2: the look sheet's "step" row, spike-side. For each crop of a run's crop store (and of the defaults' store): each stage minus the previous stage
(S0 - B, S_KH - S0, KS0 - S_KH, KS1+C - KS0, BAND6+C - KS1+C, KV - BAND6+C, FINAL - KV), drawn on the stage's own shade (sun az, alt 10, Horn gradient, 101.802 m
cells) tinted red where the step lowers and blue where it raises, full tint at 2 m. Reported per crop and step: max |step| and rms (m) over the 120-cell crop.
Writes <out>/<crop>-step-azNNN.png (rows defaults and switch, 2 px per L cell) and prints the table. Usage: step_row.py <run crops> <defaults crops> <out> [az]"""
import os, sys, numpy as np
from PIL import Image, ImageDraw
RUN, DEF, OUT = sys.argv[1], sys.argv[2], sys.argv[3]
AZ = float(sys.argv[4]) if len(sys.argv) > 4 else 315.0
cell, ALT, FULL, SC = 101.802, 10.0, 2.0, 2
STAGES = [('base', 'B'), ('s0', 'S0'), ('kh', 'S_KH'), ('ks0', 'KS0'), ('ks1c', 'KS1+C'), ('band6c', 'BAND6+C'), ('kv', 'KV'), ('final', 'FINAL')]
os.makedirs(OUT, exist_ok=True)


def crops(store):
    out = {}
    for line in open(f'{store}/crops.txt'):
        f = line.split()
        if f[0] == 'crop':
            out[f[1]] = (int(f[2]) - int(f[5]), int(f[3]) - int(f[6]), int(f[4]))
    return out


def load(store, c, s):
    a = np.fromfile(f'{store}/{c}/{s}.f32', '<f4').astype(np.float64)
    k = int(round(np.sqrt(a.size)))
    return a.reshape(k, k)


def shade(z):
    gy, gx = np.gradient(z, cell)
    az, alt = np.radians(AZ), np.radians(ALT)
    # sun direction (x east, y south in the array): azimuth clockwise from north
    lx, ly, lz = np.sin(az) * np.cos(alt), -np.cos(az) * np.cos(alt), np.sin(alt)
    nrm = np.sqrt(gx * gx + gy * gy + 1.0)
    return np.clip((-gx * lx - gy * ly + lz) / nrm, 0, 1)


def tint(sh, d):
    g = 0.25 + 0.75 * sh
    rgb = np.stack([g * 0.93, g * 0.90, g * 0.84], -1)
    t = np.clip(np.abs(d) / FULL, 0, 1)[..., None]
    red, blue = np.array([0.85, 0.1, 0.1]), np.array([0.1, 0.35, 0.95])
    col = np.where((d < 0)[..., None], red, blue)
    return rgb * (1 - t) + col * t


rows = []
for c, (ox, oy, n) in crops(RUN).items():
    panels = {}
    for lab, store in (('DEFAULTS', DEF), ('SWITCH', RUN)):
        z = [load(store, c, s) for s, _ in STAGES]
        ps = []
        for i in range(1, len(STAGES)):
            d = (z[i] - z[i - 1])
            sh = shade(z[i])
            ps.append(tint(sh, d)[oy:oy + n, ox:ox + n])
            dd = d[oy:oy + n, ox:ox + n]
            rows.append((c, lab, f'{STAGES[i][1]} - {STAGES[i - 1][1]}', np.abs(dd).max(), np.sqrt(np.mean(dd * dd))))
        panels[lab] = ps
    W = 110 + 7 * (n * SC + 10)
    img = Image.new('RGB', (W, 60 + 2 * (n * SC + 10)), (24, 24, 28))
    dr = ImageDraw.Draw(img)
    dr.text((8, 6), f'STEP ROW: {os.path.basename(RUN.rstrip("/"))} AGAINST {os.path.basename(DEF.rstrip("/"))}  CROP {c.upper()}  SUN AZ {AZ:.0f} ALT {ALT:.0f}  2 PX = 1 L CELL', fill=(230, 230, 230))
    dr.text((8, 20), f'EACH STAGE MINUS THE PREVIOUS: RED LOWER, BLUE HIGHER, FULL AT {FULL:.0f} M', fill=(150, 190, 255))
    for i in range(1, len(STAGES)):
        dr.text((110 + (i - 1) * (n * SC + 10), 38), f'{STAGES[i][1]}-{STAGES[i - 1][1]}', fill=(230, 230, 230))
    for r, lab in enumerate(('DEFAULTS', 'SWITCH')):
        y = 52 + r * (n * SC + 10)
        dr.text((8, y + n * SC // 2), lab, fill=(230, 230, 230))
        for i, p in enumerate(panels[lab]):
            im = Image.fromarray((np.clip(p, 0, 1) * 255).astype(np.uint8)).resize((n * SC, n * SC), Image.NEAREST)
            img.paste(im, (110 + i * (n * SC + 10), y))
    img.save(f'{OUT}/{c}-step-az{AZ:03.0f}.png')
print(f'step row (m) over the 120-cell crops: crop, row, step, max |step|, rms')
for c, lab, s, mx, rms in rows:
    print(f'{c:6s} {lab:8s} {s:18s} max {mx:8.3f}  rms {rms:7.3f}')
