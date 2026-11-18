"""SPIKE-140 item 1: the orientation of a window's base. Gradient direction histogram (energy weighted, angle mod 180, 15 deg bins) and the structure tensor's anisotropy, on the L grid
and corrected by the face metric (gradient components divided by the physical cell edge along each axis, ratio at the window centre), plus the 2-D power spectrum's share at the axes
(within 5 deg of 0 and 90) per wavelength octave. Usage: python -I orient.py <base.f32> <x/y edge ratio at the centre> <label>"""
import sys
import numpy as np

path, ratio, label = sys.argv[1], float(sys.argv[2]), sys.argv[3]
N = 8160
z = np.fromfile(path, dtype="<f4").reshape(N, N).astype(np.float64)
box = z[1600:6560, 1600:6560]
land = box > 0
gy, gx = np.gradient(box)


def report(gx, gy, name):
    m = land[1:-1, 1:-1]
    gx, gy = gx[1:-1, 1:-1][m], gy[1:-1, 1:-1][m]
    e = gx * gx + gy * gy
    ang = (np.degrees(np.arctan2(gy, gx)) + 180.0) % 180.0
    h, _ = np.histogram(ang, bins=12, range=(0, 180), weights=e)
    h = h / h.sum() * 12
    jxx, jyy, jxy = (gx * gx).mean(), (gy * gy).mean(), (gx * gy).mean()
    tr, det = jxx + jyy, jxx * jyy - jxy * jxy
    l1 = tr / 2 + np.sqrt(max(tr * tr / 4 - det, 0))
    l2 = tr - l1
    theta = 0.5 * np.degrees(np.arctan2(2 * jxy, jxx - jyy))
    print(f"{label} {name}: energy per 15 deg bin over the mean (0 = gradient along x): " + " ".join(f"{v:.2f}" for v in h))
    print(f"{label} {name}: structure tensor anisotropy {(l1 - l2) / (l1 + l2):.3f}, main gradient direction {theta:.1f} deg")


report(gx, gy, "grid")
report(gx / ratio, gy, "metric-corrected")
# spectrum: share of power within 5 deg of the axes, per wavelength band (in L cells)
# a plane removed and a 2-D Hann taper (the window is not periodic: its edges would put power on the axes); sea cells at the land mean
yy, xx = np.mgrid[0:box.shape[0], 0:box.shape[1]]
zz = np.where(land, box, box[land].mean())
A = np.c_[xx.ravel(), yy.ravel(), np.ones(xx.size)]
coef = np.linalg.lstsq(A[::97], zz.ravel()[::97], rcond=None)[0]
zz = zz - (coef[0] * xx + coef[1] * yy + coef[2])
del A, xx, yy
hann = np.hanning(box.shape[0])[:, None] * np.hanning(box.shape[1])[None, :]
f = np.fft.rfft2(zz * hann)
p = np.abs(f) ** 2
ky = np.fft.fftfreq(box.shape[0])[:, None]
kx = np.fft.rfftfreq(box.shape[1])[None, :]
k = np.hypot(kx, ky)
a = np.degrees(np.arctan2(np.abs(ky), kx))
near_axis = (a < 5) | (a > 85)
for lo, hi in [(8, 32), (32, 128), (128, 512), (512, 2048)]:
    band = (k > 1 / hi) & (k <= 1 / lo)
    share = p[band & near_axis].sum() / p[band].sum()
    area = (band & near_axis).sum() / band.sum()
    print(f"{label} spectrum, wavelengths {lo}-{hi} L cells ({lo * 0.1018:.1f}-{hi * 0.1018:.1f} km): {100 * share:.1f} % of the power within 5 deg of the axes (isotropic: {100 * area:.1f} %)")
