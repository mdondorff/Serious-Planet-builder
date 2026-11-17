"""SPIKE-135: the results tables from the probe's per-window CSVs (`<w>-ladder.csv`). Usage: ladder_tables.py <dir> r1h r3a r3b m1 f1 > tables.md"""
import sys
import csv

d, wins = sys.argv[1], sys.argv[2:]
NAMES = {'r1h': 'R1h', 'r3a': 'R3a', 'r3b': 'R3b', 'm1': 'M1', 'f1': 'F1'}
RUNGS = ['L0b z_b lattice 611 m', 'L0 z_b 814 m', 'L1 z_s 814 m', 'L2b C1(z_b) at L', 'L2s C1(z_s) at L', 'L3 z_s D4-breached bilinear at L', 'L5 L3 + r', 'L4 S0 = C1(z_s) + r']
LABEL = {
    'L0b z_b lattice 611 m': 'L0b `z_b`, B\'s lattice (611 m)',
    'L0 z_b 814 m': 'L0 `z_b` (814 m)',
    'L1 z_s 814 m': 'L1 `z_s` (814 m)',
    'L2b C1(z_b) at L': 'L2b `C1(z_b)` at L',
    'L2s C1(z_s) at L': 'L2s `C1(z_s)` at L',
    'L3 z_s D4-breached bilinear at L': 'L3 `z_s` 4-neighbour breached, bilinear',
    'L5 L3 + r': 'L5 L3 + `r`',
    'L4 S0 = C1(z_s) + r': 'L4 `S0` = `C1(z_s)` + `r`',
}
rows, k5, cls, ks = {}, {}, {}, {}
for w in wins:
    for line in open(f'{d}/{w}-ladder.csv', encoding='utf-8'):
        line = line.strip()
        if line.startswith('window,'):
            head = line.split(',')
            continue
        f = line.split(',')
        if f[0] == '#k5':
            k5[w] = f[2:]
        elif f[0] == '#class':
            cls.setdefault(w, {})[f[2]] = f[3:]
        elif f[0] == '#ks':
            ks.setdefault(w, {})[f[2]] = f[3:]
        else:
            rows[(w, f[1])] = dict(zip(head, f))


def table(title, fmt):
    print(f'\n**{title}**\n')
    print('| Rung | ' + ' | '.join(NAMES[w] for w in wins) + ' |')
    print('|---|' + '---|' * len(wins))
    for r in RUNGS:
        print(f'| {LABEL[r]} | ' + ' | '.join(fmt(rows[(w, r)]) for w in wins) + ' |')


def share(r, k):
    return 100.0 * float(r[k]) / max(1.0, float(r['land_cells']))


table('River length (75 km2 channels) on closed depressions: all / over 5 m deep, %', lambda r: f"{float(r['riv75_pit_pct']):.1f} / {float(r['riv75_pit5_pct']):.1f}")
table('River length (1 km2 channels) on closed depressions: all / over 5 m deep, %', lambda r: f"{float(r['riv1_pit_pct']):.1f} / {float(r['riv1_pit5_pct']):.1f}")
table('Depressions in the box: all / over 5 m deep', lambda r: f"{int(r['depressions']):,} / {int(r['depr5']):,}")
table('Breach (complete, along the routing): land cells lowered, % / over 10 m, % / over 50 m, %', lambda r: f"{share(r, 'breach_cells'):.2f} / {share(r, 'breach_over10'):.3f} / {share(r, 'breach_over50'):.3f}")
table('Breach depth on lowered cells p50 / p95 / max, m', lambda r: f"{float(r['breach_p50_m']):.2f} / {float(r['breach_p95_m']):.1f} / {float(r['breach_max_m']):.0f}")
table('On 75 km2 rivers: breach p95 / max, m; cells over 10 m / over 50 m', lambda r: f"{float(r['riv_breach_p95_m']):.1f} / {float(r['riv_breach_max_m']):.0f}; {int(r['riv_breach_over10']):,} / {int(r['riv_breach_over50']):,}")
table('Volume: breach / fill alternative, km3', lambda r: f"{float(r['breach_km3']):.2f} / {float(r['fill_km3']):.1f}")
table('Fill alternative: land cells raised, % / over 10 m, % / over 50 m, %', lambda r: f"{share(r, 'fill_cells'):.2f} / {share(r, 'fill_over10'):.3f} / {share(r, 'fill_over50'):.3f}")
table('Ridge cells (G0 section 9) the breach lowers over 1 m / over 10 m (of the ridge cells)', lambda r: f"{int(r['ridge_breach1']):,} / {int(r['ridge_breach10']):,} (of {int(r['ridge_cells']):,})")
table('D-2: H1 trunk reaches failing the corridor measure (k_c 2 snapped, land-only snap) / measured', lambda r: '-' if r['d2_measured'] == '-' else f"{int(r['d2_failed']):,} / {int(r['d2_measured']):,}")
table('Inland land at or below 0 m (reading 12), cells', lambda r: f"{int(r['inland_cells']):,}")

print('\n**K5 at 814 m: cells breached over 1 m (both / only in `z_b`: K5 repaired / only in `z_s`: K5 made); K5\'s lowering / raising of `z_b`, km3; the base\'s minimal breach / fill, km3**\n')
print('| Window | Both | Repaired | Made | K5 lowers / raises | `z_b` breach / fill |')
print('|---|---|---|---|---|---|')
for w in wins:
    b, o0, o1, lo, hi = k5[w]
    r = rows[(w, 'L0 z_b 814 m')]
    print(f"| {NAMES[w]} | {int(b):,} | {int(o0):,} | {int(o1):,} | {float(lo):,.0f} / {float(hi):,.0f} | {float(r['breach_km3']):,.0f} / {float(r['fill_km3']):,.0f} |")

print('\n**`S0`\'s breach cells (lowered over 1 m) by the first rung with a breach there: cells (share) / volume share / share of the class that the hand-over (L5) has no breach within 4 L cells of**\n')
print('| Class | ' + ' | '.join(NAMES[w] for w in wins) + ' |')
print('|---|' + '---|' * len(wins))
for k in ['base', 'K5', 'interpolation', 'band']:
    cells = []
    for w in wins:
        c = cls[w]
        tot = sum(float(v[0]) for v in c.values())
        vtot = sum(float(v[3]) for v in c.values())
        v = c[k]
        cells.append(f"{int(v[0]):,} ({100 * float(v[0]) / tot:.1f} %) / {100 * float(v[3]) / vtot:.1f} % / {100 * float(v[5]) / max(1.0, float(v[0])):.0f} %")
    print(f'| {k} | ' + ' | '.join(cells) + ' |')
print('| over 10 m: base / K5 / interpolation / band, % | ' + ' | '.join(
    '{} / {} / {} / {}'.format(*(f"{100 * float(cls[w][k][1]) / max(1.0, sum(float(v[1]) for v in cls[w].values())):.0f}" for k in ['base', 'K5', 'interpolation', 'band'])) for w in wins) + ' |')
print('| on 75 km2 rivers: base / K5 / interpolation / band, % | ' + ' | '.join(
    '{} / {} / {} / {}'.format(*(f"{100 * float(cls[w][k][4]) / max(1.0, sum(float(v[4]) for v in cls[w].values())):.0f}" for k in ['base', 'K5', 'interpolation', 'band'])) for w in wins) + ' |')

print('\n**KS\'s lowering of `S0` (SPIKE-128): volume, km3; share of it by the cell\'s class (no `S0` breach / base / K5 / interpolation / band), %; within 4 L cells of an `S0` breach cell, %; the part the `S0` breach also asks for (min of the two), %**\n')
print('| Window | Stage | Volume | none / base / K5 / interp. / band | Near a breach | Asked for |')
print('|---|---|---|---|---|---|')
for w in wins:
    for name, v in ks.get(w, {}).items():
        print(f"| {NAMES[w]} | {name.replace('KS ', '').replace('rg', 'RG-')} | {float(v[0]):.2f} | {float(v[1]):.0f} / {float(v[2]):.1f} / {float(v[3]):.1f} / {float(v[4]):.0f} / {float(v[5]):.1f} | {float(v[6]):.0f} | {float(v[7]):.0f} |")
