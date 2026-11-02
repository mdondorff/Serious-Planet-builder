"""SPIKE-92 #67: H2r3 seed sheets. Land-sea and elevation (4 x 6, seed label, pass mark: PASS or the failed checks) and a polar strip
(north and south azimuthal views to 40 degrees from the pole per seed). Palette PNGs keep each sheet under 1 MB. Usage: python s92_seedsheets.py <config dir> <out dir>."""
import json, os, sys
import numpy as np
from PIL import Image, ImageDraw
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from s92_polar import polar, rgb
root, out = sys.argv[1], sys.argv[2]
rows = json.load(open(f'{root}/scores.json'))
def fails(r):
    f = []
    if not 0.27 <= r['land'] <= 0.31: f.append('land %.0f%%' % (100 * r['land']))
    if not 4 <= r['s1'] <= 7: f.append('S1 %d' % r['s1'])
    if not 0.45 <= r['s2'] <= 0.62: f.append('S2 %.0f%%' % (100 * r['s2']))
    if r['s3'] < 1.18: f.append('S3 %.2f' % r['s3'])
    if r['s9'] > 0.20: f.append('S9 %.2f' % r['s9'])
    if not 1.5 <= r['s10'] <= 2.5: f.append('S10 %.1f' % r['s10'])
    if not (35 <= r['s11'] <= 70 and 0.02 <= r['s11_share'] <= 0.06): f.append('S11 %d/%.1f%%' % (r['s11'], 100 * r['s11_share']))
    if r['polar'] > 1: f.append('polar %d' % r['polar'])
    return f
def mark(d, x, y, w, r, seed):
    f = fails(r)
    d.rectangle([x, y, x + w - 1, y + 15], fill=(255, 255, 255))
    d.text((x + 4, y + 2), 'seed %d' % seed, fill=(0, 0, 0))
    txt = 'PASS' if not f else 'FAIL: ' + ', '.join(f)
    d.text((x + 70, y + 2), txt, fill=(0, 130, 0) if not f else (200, 0, 0))
    d.text((x + w - 120, y + 2), 'polar %d  isl %d' % (r['polar'], r['s11']), fill=(60, 60, 60))
def sheet(kind, cw, ch):
    pad = 8
    img = Image.new('RGB', (4 * (cw + pad) + pad, 6 * (ch + 16 + pad) + pad), (255, 255, 255)); d = ImageDraw.Draw(img)
    for k, r in enumerate(rows):
        seed = k + 1; x = pad + (k % 4) * (cw + pad); y = pad + (k // 4) * (ch + 16 + pad)
        if kind == 'land':
            m = np.fromfile(f'{root}/seed-{seed}/land.u8', dtype=np.uint8).reshape(720, 1440) > 0
            tile = Image.fromarray(rgb(m.astype(int)))
        else:
            tile = Image.open(f'{root}/seed-{seed}/elevation.png').convert('RGB').crop((0, 48, 1440, 768))
        img.paste(tile.resize((cw, ch), Image.LANCZOS), (x, y + 16)); mark(d, x, y, cw, r, seed)
    return img
sheet('land', 480, 240).quantize(colors=64, dither=Image.Dither.NONE).save(f'{out}/H2r3-seeds-land-sea.png', optimize=True)
sheet('elev', 400, 200).quantize(colors=96, dither=Image.Dither.NONE).save(f'{out}/H2r3-seeds-elevation.png', optimize=True)
# Polar strip: 4 seeds per row, each a north and a south disc.
P, pad = 200, 8
img = Image.new('RGB', (4 * (2 * P + 3 * pad) + pad, 6 * (P + 16 + pad) + pad), (255, 255, 255)); d = ImageDraw.Draw(img)
for k, r in enumerate(rows):
    seed = k + 1; x = pad + (k % 4) * (2 * P + 3 * pad); y = pad + (k // 4) * (P + 16 + pad)
    m = np.fromfile(f'{root}/seed-{seed}/land.u8', dtype=np.uint8).reshape(720, 1440) > 0
    for i, north in enumerate((True, False)):
        img.paste(Image.fromarray(rgb(polar(m, north))).resize((P, P), Image.LANCZOS), (x + i * (P + pad), y + 16))
    d.rectangle([x, y, x + 2 * P + pad - 1, y + 15], fill=(255, 255, 255))
    d.text((x + 4, y + 2), 'seed %d  north | south  polar masses %d' % (seed, r['polar']), fill=(0, 0, 0))
img.quantize(colors=64, dither=Image.Dither.NONE).save(f'{out}/H2r3-seeds-polar.png', optimize=True)
print([(k + 1, fails(r)) for k, r in enumerate(rows) if fails(r)])
