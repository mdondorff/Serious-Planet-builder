"""SPIKE-136: the results tables from the probe's per-window files (`<w>-ladder.csv`, `<w>-handover.csv`, the run log `<w>.log.out`), or their committed copies
(`ladder-<w>.csv`, `handover-<w>.csv`, `run-<w>.txt`). Usage: handover_tables.py <dir> r1h r3a r3b m1 f1 > tables.md"""
import os
import re
import sys

d, wins = sys.argv[1], sys.argv[2:]
NAMES = {'r1h': 'R1h', 'r3a': 'R3a', 'r3b': 'R3b', 'm1': 'M1', 'f1': 'F1'}
LADDER = {'S0': 'L4 S0 = C1(z_s) + r', 'C1': 'L2s C1(z_s) at L', 'h1': 'h1 D4-breached bilinear', 'h1r': 'h1 + r', 'h2': 'h2 flow-diagonal TIN', 'h2r': 'h2 + r'}
LABEL = {'S0': '`S0` = `C1(z_s)` + `r`', 'C1': '`C1(z_s)` (no band)', 'h1': 'h1 (no band)', 'h1r': 'h1 + `r`', 'h2': 'h2 (no band)', 'h2r': 'h2 + `r`'}


def path(a, b):
    return a if os.path.exists(a) else b


lad, ho, h1low, h2split, logs = {}, {}, {}, {}, {}
for w in wins:
    for line in open(path(f'{d}/ladder-{w}.csv', f'{d}/{w}-ladder.csv'), encoding='utf-8'):
        f = line.strip().split(',')
        if f[0] == 'window':
            head = f
        elif f[0] == '#h1low':
            h1low[w] = f[2:]
        elif f[0] == '#h2split':
            h2split[w] = f[2:]
        elif not f[0].startswith('#'):
            lad[(w, f[1])] = dict(zip(head, f))
    for line in open(path(f'{d}/handover-{w}.csv', f'{d}/{w}-handover.csv'), encoding='utf-8'):
        f = line.strip().split(',')
        if f[0] == 'window':
            h2 = f
        else:
            ho[(w, f[1])] = dict(zip(h2, f))
    logs[w] = open(path(f'{d}/run-{w}.txt', f'{d}/{w}.log.out'), encoding='utf-8').read()


def table(title, keys, fmt, src='lad'):
    print(f'\n**{title}**\n')
    print('| Surface | ' + ' | '.join(NAMES[w] for w in wins) + ' |')
    print('|---|' + '---|' * len(wins))
    for k in keys:
        cells = []
        for w in wins:
            r = lad.get((w, LADDER[k])) if src == 'lad' else ho.get((w, k))
            cells.append(fmt(r) if r else '-')
        print(f'| {LABEL[k]} | ' + ' | '.join(cells) + ' |')


def share(r, k):
    return 100.0 * float(r[k]) / max(1.0, float(r['land_cells']))


ALL = ['S0', 'C1', 'h1', 'h1r', 'h2', 'h2r']
table('River length (75 km2 channels) on closed depressions: all / over 5 m deep, %', ALL, lambda r: f"{float(r['riv75_pit_pct']):.1f} / {float(r['riv75_pit5_pct']):.1f}")
table('River length (1 km2 channels) on closed depressions: all / over 5 m deep, %', ALL, lambda r: f"{float(r['riv1_pit_pct']):.1f} / {float(r['riv1_pit5_pct']):.1f}")
table('Breach: land cells lowered, % / over 10 m, % / over 50 m, % of the land', ALL, lambda r: f"{share(r, 'breach_cells'):.2f} / {share(r, 'breach_over10'):.3f} / {share(r, 'breach_over50'):.3f}")
table('Breach volume, km3 / max depth, m', ALL, lambda r: f"{float(r['breach_km3']):.2f} / {float(r['breach_max_m']):.0f}")
table('D-2: H1 trunk reaches failing the corridor measure / measured', ['S0', 'h1', 'h1r', 'h2', 'h2r'], lambda r: f"{int(r['d2_failed']):,} / {int(r['d2_measured']):,}")
table('Own ridge cells (G0 section 9) breached over 10 m', ALL, lambda r: f"{int(r['ridge_breach10']):,}")
table('Inland land at or below 0 m, cells', ALL, lambda r: f"{int(r['inland_cells']):,}")
HO = ['h1', 'h1r', 'h2', 'h2r']
table('Relief kept against `S0`: band std / 1 km block range p50 / p95, %', HO, lambda r: f"{float(r['band_std_kept_pct']):.0f} / {float(r['p50_kept_pct']):.0f} / {float(r['p95_kept_pct']):.0f}", 'ho')
table('Lowered under the reference (`C1(z_s)` without the band, `S0` with it): cells over 10 m / over 50 m; raised over 10 m', HO,
      lambda r: f"{int(r['lowered_vs_ref_over10']):,} / {int(r['lowered_vs_ref_over50']):,}; {int(r['raised_vs_ref_over10']):,}", 'ho')
table('`S0`\'s ridge cells lowered over 10 m / over 50 m (against the reference)', HO, lambda r: f"{int(r['s0_ridges_lowered10']):,} / {int(r['s0_ridges_lowered50']):,}", 'ho')
table('`S0`\'s breach cells (over 1 m) with no breach of the surface within 4 L cells, by `S0` class: base / K5 / interpolation / band / all, %', HO,
      lambda r: f"{float(r['fixed_base_pct']):.0f} / {float(r['fixed_k5_pct']):.0f} / {float(r['fixed_interp_pct']):.0f} / {float(r['fixed_band_pct']):.0f} / {float(r['fixed_all_pct']):.0f}", 'ho')
table('The surface\'s breach volume (cells over 1 m) by `S0`\'s class at the cell: none / base / K5 / interpolation / band, km3', HO,
      lambda r: f"{float(r['on_none_km3']):.2f} / {float(r['on_base_km3']):.2f} / {float(r['on_k5_km3']):.2f} / {float(r['on_interp_km3']):.2f} / {float(r['on_band_km3']):.2f}", 'ho')

print('\n**h1\'s 814 m lowering (the 4-neighbour breach of `z_s`, step 1 cm, land floored at 5 mm), over the land of the box**\n')
print('| Window | Lowered, % of land | p50 / p95 / max, m | Over 10 / 50 / 100 / 200 m, cells (over 50 m, % of land) | Volume, km3 | Ridge cells of `z_s` lowered over 10 / 50 m |')
print('|---|---|---|---|---|---|')
for w in wins:
    land, nlow, o10, o50, o100, o200, vol, r10, r50 = h1low[w]
    m = re.search(r'h1 814 m lowering: \d+ of \d+ land cells \(([\d.]+) %\): p50 ([\d.]+) p95 ([\d.]+) max ([\d.]+) m.*\(of (\d+)\)', logs[w])
    print(f"| {NAMES[w]} | {m.group(1)} | {float(m.group(2)):.1f} / {float(m.group(3)):.0f} / {float(m.group(4)):.0f} | {int(o10):,} / {int(o50):,} / {int(o100):,} / {int(o200):,} ({100 * int(o50) / int(land):.2f} %) | {float(vol):.1f} | {int(r10):,} / {int(r50):,} of {int(m.group(5)):,} |")

print('\n**h2\'s quads: split along a D8 diagonal step of `z_s` / both diagonals stepped / split along the higher pair**\n')
print('| Window | Step | Both | Higher pair |')
print('|---|---|---|---|')
for w in wins:
    a, b, c = h2split[w]
    print(f'| {NAMES[w]} | {int(a):,} | {int(b):,} | {int(c):,} |')

# the hand-overs' own lowering at L apart from the interpolant (handover_low.rs): `<w>-lowering.csv` or `lowering-<w>.csv`
low = {}
for w in wins:
    p = path(f'{d}/lowering-{w}.csv', f'{d}/{w}-lowering.csv')
    if not os.path.exists(p):
        continue
    for line in open(p, encoding='utf-8'):
        f = line.strip().split(',')
        if f[0] != 'window':
            low[(w, f[1].split(':')[0])] = f
CMP = [('h1 breach at L', 'h1\'s breach read at L: bilinear(`z_s`) - h1'), ('h2 against bilinear(z_s)', 'h2 against bilinear(`z_s`): bilinear - TIN'),
       ('C1(z_s) against bilinear(z_s)', '`C1(z_s)` against bilinear(`z_s`): C1 - bilinear'), ('h1 against C1(z_s)', 'h1 against `C1(z_s)`'), ('h2 against C1(z_s)', 'h2 against `C1(z_s)`')]
if low:
    print('\n**Lowering at L by its source, over the land of the box: cells lowered over 10 m / over 50 m; raised over 50 m; `S0` ridge cells lowered over 10 m / over 50 m; lowering p95 / max, m**\n')
    print('| Comparison | ' + ' | '.join(NAMES[w] for w in wins) + ' |')
    print('|---|' + '---|' * len(wins))
    for key, label in CMP:
        cells = []
        for w in wins:
            f = low.get((w, key))
            cells.append('-' if f is None else f"{int(f[3]):,} / {int(f[4]):,}; {int(f[6]):,}; {int(f[7]):,} / {int(f[8]):,}; {float(f[9]):.1f} / {float(f[10]):.0f}")
        print(f'| {label} | ' + ' | '.join(cells) + ' |')
