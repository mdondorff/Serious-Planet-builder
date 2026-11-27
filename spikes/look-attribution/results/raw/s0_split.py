"""Lead #184i item 1: where S0's stepped diagonal ridges come from. S0 = B + K7's regional read (RegionField::at, Catmull-Rom clamped) of K5's delta d = z_s - z_b
at H1 (rg_chain: `start = bl + regional_read.at(..)`). From a crop store: R = S0 - B is that read at L. The delta at the H1 nodes (the H1 cell centres, L 8i + 3.5) is
taken as R's mean over the node's 2 x 2 L cells (Catmull-Rom passes through its nodes, so this is the node value up to the read's curvature over half a cell).
Layers rendered at az 315, alt 10 (Horn gradient, 101.802 m cells), 3 px per L cell: B; B + d nearest (each H1 cell flat); B + d bilinear; S0 (B + the K7 read);
and the delta fields alone (nearest, bilinear, the K7 read) shaded on their own. Measured on each layer: X8 (SPIKE-143's 814 m phase excess, m1_facet), and the
dominant wavelength (L cells) and orientation (degrees, 0 = east, counter-clockwise, crest direction) of its high-pass (minus a 6-cell Gaussian) in the crop's
upper-left quadrant. Usage: s0_split.py <crops store> <crop> <out.png> [qx0 qy0 qsize]"""
import os, sys, numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from m1_facet import excess  # noqa: E402
ST, C, OUT = sys.argv[1], sys.argv[2], sys.argv[3]
cell, L, SC = 101.802, 8, 3
gx0 = gy0 = None
for line in open(f'{ST}/{C}/chain.txt'):
    if line.startswith('window '):
        gx0, gy0 = map(int, line.split()[1:3])
ld = lambda s: (lambda a: a.reshape(int(round(np.sqrt(a.size))), -1))(np.fromfile(f'{ST}/{C}/{s}.f32', '<f4').astype(np.float64))
B, S0 = ld('base'), ld('s0')
n = B.shape[0]
assert gx0 % L == 0 and gy0 % L == 0, 'crop not aligned to the H1 grid'
R = S0 - B
nh = n // L
node = np.array([[R[L * j + 3:L * j + 5, L * i + 3:L * i + 5].mean() for i in range(nh)] for j in range(nh)])
near = np.repeat(np.repeat(node, L, 0), L, 1)
# bilinear between node centres (L 8i + 3.5), clamped at the edge nodes
u = (np.arange(n) - 3.5) / L
bil = ndimage.map_coordinates(node, np.meshgrid(u, u, indexing='ij')[::-1][::-1], order=1, mode='nearest')
yy, xx = np.meshgrid(u, u, indexing='ij')
bil = ndimage.map_coordinates(node, [yy, xx], order=1, mode='nearest')
q = [int(v) for v in sys.argv[4:7]] if len(sys.argv) > 6 else [0, 0, n // 2]


def shade(z, az=315.0, alt=10.0):
    gy, gx = np.gradient(z, cell)
    a, e = np.radians(az), np.radians(alt)
    lx, ly, lz = np.sin(a) * np.cos(e), -np.cos(a) * np.cos(e), np.sin(e)
    return np.clip((-gx * lx - gy * ly + lz) / np.sqrt(gx * gx + gy * gy + 1.0), 0, 1)


def wave(z):
    s = z[q[1]:q[1] + q[2], q[0]:q[0] + q[2]]
    hp = s - ndimage.gaussian_filter(s, 6.0)
    hp = (hp - hp.mean()) * np.outer(np.hanning(q[2]), np.hanning(q[2]))
    F = np.abs(np.fft.fftshift(np.fft.fft2(hp))) ** 2
    f = np.fft.fftshift(np.fft.fftfreq(q[2]))
    FY, FX = np.meshgrid(f, f, indexing='ij')
    F[np.hypot(FX, FY) < 1.0 / 40] = 0
    j, i = np.unravel_index(np.argmax(F), F.shape)
    fx, fy = FX[j, i], FY[j, i]
    lam = 1.0 / np.hypot(fx, fy)
    # crest direction: perpendicular to the wavevector; y down in the array, so flip for counter-clockwise from east
    ang = (np.degrees(np.arctan2(-fy, fx)) + 90.0) % 180.0
    return lam, ang, np.sqrt(np.mean(hp * hp))


layers = [('B', B), ('B + K5 D NEAREST', B + near), ('B + K5 D BILINEAR', B + bil), ('S0 = B + K7 READ', S0),
          ('K5 D NEAREST', near), ('K5 D BILINEAR', bil), ('K7 READ (S0 - B)', R)]
mask = np.ones((n, n), bool)
print(f'{os.path.basename(ST.rstrip("/"))} {C}: crop {n} L cells at global L ({gx0}, {gy0}); K5 delta at {nh} x {nh} H1 nodes: p50 {np.median(node):.1f} m, '
      f'range {node.min():.1f} .. {node.max():.1f} m; high-pass window: x {q[0]}..{q[0] + q[2]}, y {q[1]}..{q[1] + q[2]}')
for lab, z in layers:
    o = excess(z, mask, gx0, gy0)
    lam, ang, rms = wave(z)
    print(f'  {lab:20s} X8 {o["X8"]:+.3f} X6 {o["X6"]:+.3f} Xdiag {o["Xdiag"]:+.3f}; high-pass: dominant wavelength {lam:5.1f} L cells, crest at {ang:5.1f} deg, rms {rms:.2f} m')
img = Image.new('RGB', (len(layers) * (n * SC + 8) + 8, n * SC + 40), (24, 24, 28))
dr = ImageDraw.Draw(img)
for k, (lab, z) in enumerate(layers):
    g = (0.2 + 0.8 * shade(z)) * 255
    im = Image.fromarray(np.stack([g * 0.93, g * 0.9, g * 0.84], -1).astype(np.uint8)).resize((n * SC, n * SC), Image.NEAREST)
    img.paste(im, (8 + k * (n * SC + 8), 32))
    dr.text((8 + k * (n * SC + 8), 6), lab, fill=(230, 230, 230))
    dr.text((8 + k * (n * SC + 8), 18), f'SUN AZ 315 ALT 10, {SC} PX = 1 L CELL', fill=(150, 190, 255))
img.save(OUT)
