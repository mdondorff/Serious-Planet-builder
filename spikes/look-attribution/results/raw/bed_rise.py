"""Lead #184t: do trunk beds rise when H1 is routed on K6' while K5's z_s is not re-solved? From a chain's graph.txt: every L reach with a parent H1 reach (a trunk image),
its `bed=` per vertex (upstream first). A rise is a vertex's bed above the running minimum of the reach's beds before it. Reported over the trunk images: the reaches
with a rise over 1 / 10 / 50 m (and of them the ones carrying an exception marker), the largest rise p50 / p90 / max over those with one, and the share of trunk
vertices standing over 1 m above their running minimum. Usage: bed_rise.py <graph.txt>"""
import sys, numpy as np
G = sys.argv[1]
mx, flagged, up, tot = [], [], 0, 0
for line in open(G):
    if not line.startswith('reach ') or ' level=l ' not in line:
        continue
    f = dict(t.split('=', 1) for t in line.split()[2:] if '=' in t)
    if f.get('parent', '-') == '-' or 'bed' not in f:
        continue
    bed = np.array([float(v) for v in f['bed'].split(',')])
    if len(bed) < 2:
        continue
    rise = bed - np.minimum.accumulate(bed)
    mx.append(rise.max()); flagged.append(f.get('exceptions', '-') != '-')
    up += int((rise > 1.0).sum()); tot += len(rise)
mx = np.array(mx); fl = np.array(flagged)
print(f'{G.split("/")[-1]}: {len(mx)} trunk-image L reaches; bed rising over 1 / 10 / 50 m above its running minimum on '
      + ' / '.join(f'{int((mx > t).sum())} ({int(((mx > t) & fl).sum())} flagged)' for t in (1, 10, 50))
      + f'; largest rise p50 / p90 / max over those with one {np.median(mx[mx > 1]) if (mx > 1).any() else 0:.1f} / '
      f'{np.percentile(mx[mx > 1], 90) if (mx > 1).any() else 0:.1f} / {mx.max():.1f} m; trunk vertices over 1 m above it {100 * up / max(tot, 1):.2f} %')
