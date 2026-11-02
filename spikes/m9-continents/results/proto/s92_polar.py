"""SPIKE-92 round 3: north and south polar azimuthal-equidistant views (poleward of 40 degrees) of 1440x720 land masks beside their
equirectangular map, nearest-cell sampling. Usage: python s92_polar.py <out.png> <label=mask> ..."""
import sys
import numpy as np
from PIL import Image, ImageDraw
H, W, N = 720, 1440, 400
def polar(m, north):
    y, x = np.mgrid[0:N, 0:N]
    u, v = (x + 0.5) / N * 2 - 1, (y + 0.5) / N * 2 - 1
    rho = np.hypot(u, v) * 50.0  # degrees from the pole
    lat = 90 - rho if north else -90 + rho
    lon = np.degrees(np.arctan2(u, -v if north else v))
    i = np.clip(((90 - lat) / 0.25).astype(int), 0, H - 1); j = (((lon + 180) / 0.25).astype(int)) % W
    img = np.where(m[i, j], 1, 0); img[rho > 50] = 2
    return img
def rgb(a):
    pal = np.array([(40, 70, 120), (150, 180, 110), (255, 255, 255)], np.uint8); return pal[a]
if __name__ == '__main__':
    rows = []
    for spec in sys.argv[2:]:
        label, path = spec.split('=')
        m = np.fromfile(path, dtype=np.uint8).reshape(H, W) > 0
        eq = Image.fromarray(rgb(m.astype(int))).resize((800, 400))
        row = Image.new('RGB', (800 + 2 * N + 20, N + 16), (255, 255, 255))
        row.paste(eq, (0, 16)); row.paste(Image.fromarray(rgb(polar(m, True))), (810, 16)); row.paste(Image.fromarray(rgb(polar(m, False))), (820 + N, 16))
        d = ImageDraw.Draw(row); d.text((4, 2), label + ': equirectangular | north pole (to 40 N) | south pole (to 40 S)', fill=(0, 0, 0))
        rows.append(row)
    sheet = Image.new('RGB', (rows[0].width, sum(r.height for r in rows)), (255, 255, 255))
    y = 0
    for r in rows: sheet.paste(r, (0, y)); y += r.height
    sheet.save(sys.argv[1], optimize=True)
