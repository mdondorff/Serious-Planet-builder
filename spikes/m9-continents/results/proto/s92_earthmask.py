"""SPIKE-92: Earth's land mask at 0.25 degree (ETOPO 2022 surface elevation > 0, 15x15 block mean), written as 1440x720 bytes, row 0 north."""
import sys, numpy as np
from glo30 import read_tile
a, _, _, _ = read_tile(sys.argv[1])
a = a[:10800, :21600]
e = a.reshape(720, 15, 1440, 15).mean(axis=(1, 3))
(e > 0).astype(np.uint8).tofile(sys.argv[2])
print('land', (e > 0).mean())
