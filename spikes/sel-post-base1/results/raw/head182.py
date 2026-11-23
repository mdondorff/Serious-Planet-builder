"""#182: reach a_geo (every level; a trunk reach is one whose areas differ), first and last vertex, in two graph texts; ratio b/a per reach. Usage: head182.py <graph a> <graph b>"""
import sys, struct, numpy as np
def rd(p):
    out = {}
    for ln in open(p, encoding='utf-8'):
        if not ln.startswith('reach '):
            continue
        f = ln.split()
        g = next(x for x in f if x.startswith('a_geo=')).split('=', 1)[1].split(',')
        v = [struct.unpack('>d', bytes.fromhex(h))[0] for h in g]
        out[f[1]] = (v[0], v[-1])
    return out
a, b = rd(sys.argv[1]), rd(sys.argv[2])
ks = [k for k in a if k in b and a[k][1] > 0 and a[k][0] > 0 and a[k] != b[k]]
print(f'{len(a)} reaches in a, {len(b)} in b, {sum(k in b for k in a)} common, {len(ks)} with different areas')
for i, nm in ((1, 'last'), (0, 'first')):
    r = np.array([b[k][i] / a[k][i] for k in ks])
    print(f'{nm} vertex: {len(ks)} reaches that differ; b/a p10 {np.percentile(r,10):.3f} p50 {np.percentile(r,50):.3f} p90 {np.percentile(r,90):.3f}; lower {100*np.mean(r<1-1e-12):.1f} %')
