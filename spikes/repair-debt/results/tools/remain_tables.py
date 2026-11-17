"""SPIKE-135 addendum (spikes #141): what stays in closed basins after KH(h4c) + r' and KS1, by origin class, from the `#remain` lines of `remain-ladder-<w>.csv`
(kh_smooth --ks1). Usage: remain_tables.py <dir> r1h r3a r3b m1 f1 > remain-tables.md"""
import sys

d, wins = sys.argv[1], sys.argv[2:]
NAMES = {'r1h': 'R1h', 'r3a': 'R3a', 'r3b': 'R3b', 'm1': 'M1', 'f1': 'F1'}
CLASSES = ['base', 'K5', 'interpolation', 'band']
rows = {}
for w in wins:
    for line in open(f'{d}/remain-ladder-{w}.csv', encoding='utf-8'):
        f = line.strip().split(',')
        if f[0] == '#remain':
            # #remain, tag, class, river cells, in closed depressions, in G-DRAIN basins, over 5 m, breach cells, breach km3
            rows[(w, f[2])] = (int(f[3]), int(f[4]), int(f[5]), int(f[6]), int(f[7]), float(f[8]))


def pct(a, b):
    return 100.0 * a / b if b else 0.0


print('**The remainder on F = KH(h4c) + r\' - KS1, river cells (75 km2) of the box: in G-DRAIN basins (at least 0.5 m deep, 16 cells), % of the river; in any closed depression; over 5 m deep**\n')
print('| Window | River cells | In G-DRAIN basins | In any closed depression | Over 5 m deep | Breach volume over 1 m, km3 |')
print('|---|---|---|---|---|---|')
for w in wins:
    riv = rows[(w, 'base')][0]
    g = sum(rows[(w, k)][2] for k in CLASSES)
    a = sum(rows[(w, k)][1] for k in CLASSES)
    f5 = sum(rows[(w, k)][3] for k in CLASSES)
    v = sum(rows[(w, k)][5] for k in CLASSES)
    print(f'| {NAMES[w]} | {riv:,} | {g:,} ({pct(g, riv):.1f} %) | {a:,} ({pct(a, riv):.1f} %) | {f5:,} ({pct(f5, riv):.1f} %) | {v:.3f} |')

for title, idx in (('River cells in G-DRAIN basins by origin class, % of the remainder', 2), ('River cells over 5 m deep by origin class, % of the remainder', 3),
                   ('Breach volume (cells over 1 m) by origin class, % of the volume', 5)):
    print(f'\n**{title}: base / K5 / interpolation / band**\n')
    print('| Window | base | K5 | interpolation | band |')
    print('|---|---|---|---|---|')
    for w in wins:
        tot = sum(rows[(w, k)][idx] for k in CLASSES)
        print(f'| {NAMES[w]} | ' + ' | '.join(f'{pct(rows[(w, k)][idx], tot):.1f}' for k in CLASSES) + ' |')
