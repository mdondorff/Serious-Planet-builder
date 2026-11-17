"""SPIKE-136 addendum 2 (spikes #138): the tables of h2, h3 and h4 from kh_smooth's per-window files (`<w>-ladder.csv`, `<w>-handover.csv`), or their committed copies
(`smooth-ladder-<w>.csv`, `smooth-handover-<w>.csv`). Usage: smooth_tables.py <dir> r1h r3a r3b m1 f1 > smooth-tables.md"""
import os
import sys

d, wins = sys.argv[1], sys.argv[2:]
NAMES = {'r1h': 'R1h', 'r3a': 'R3a', 'r3b': 'R3b', 'm1': 'M1', 'f1': 'F1'}
LADDER = {'S0': 'L4 S0 = C1(z_s) + r', 'C1': 'L2s C1(z_s) at L', 'h2r': 'h2 + r', 'h3': 'h3 skeleton', 'h3r': 'h3 + r', 'h4': 'h4 Clough-Tocher', 'h4r': 'h4 + r'}
LABEL = {'S0': '`S0` = `C1(z_s)` + `r`', 'C1': '`C1(z_s)` (no band)', 'h2r': 'h2 + `r`', 'h3': 'h3 (no band)', 'h3r': 'h3 + `r`', 'h4': 'h4 (no band)', 'h4r': 'h4 + `r`'}


def path(a, b):
    return a if os.path.exists(a) else b


lad, ho, h4 = {}, {}, {}
for w in wins:
    for line in open(path(f'{d}/smooth-ladder-{w}.csv', f'{d}/{w}-ladder.csv'), encoding='utf-8'):
        f = line.strip().split(',')
        if f[0] == 'window':
            head = f
        elif f[0] == '#h4':
            h4[w] = f[2:]
        elif not f[0].startswith('#'):
            lad[(w, f[1])] = dict(zip(head, f))
    for line in open(path(f'{d}/smooth-handover-{w}.csv', f'{d}/{w}-handover.csv'), encoding='utf-8'):
        f = line.strip().split(',')
        if f[0] == 'window':
            h2 = f
        else:
            ho[(w, f[1])] = dict(zip(h2, f))


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


ALL = ['S0', 'C1', 'h2r', 'h3', 'h3r', 'h4', 'h4r']
HO = ['h2r', 'h3', 'h3r', 'h4', 'h4r']
table('River length (75 km2 channels) on closed depressions: all / over 5 m deep, %', ALL, lambda r: f"{float(r['riv75_pit_pct']):.1f} / {float(r['riv75_pit5_pct']):.1f}")
table('River length (1 km2 channels) on closed depressions: all / over 5 m deep, %', ALL, lambda r: f"{float(r['riv1_pit_pct']):.1f} / {float(r['riv1_pit5_pct']):.1f}")
table('Breach: land cells lowered, % / over 10 m, % / over 50 m, % of the land', ALL, lambda r: f"{share(r, 'breach_cells'):.2f} / {share(r, 'breach_over10'):.3f} / {share(r, 'breach_over50'):.3f}")
table('Breach volume, km3 / max depth, m', ALL, lambda r: f"{float(r['breach_km3']):.2f} / {float(r['breach_max_m']):.0f}")
table('D-2: H1 trunk reaches failing the corridor measure / measured', ['S0', 'h2r', 'h3', 'h3r', 'h4', 'h4r'], lambda r: f"{int(r['d2_failed']):,} / {int(r['d2_measured']):,}")
table('Own ridge cells (G0 section 9) breached over 10 m', ALL, lambda r: f"{int(r['ridge_breach10']):,}")
table('Inland land at or below 0 m, cells', ALL, lambda r: f"{int(r['inland_cells']):,}")
table('Relief kept against `S0`: band std / 1 km block range p50 / p95, %', HO, lambda r: f"{float(r['band_std_kept_pct']):.0f} / {float(r['p50_kept_pct']):.0f} / {float(r['p95_kept_pct']):.0f}", 'ho')
table('Lowered under the reference (`C1(z_s)` without the band, `S0` with it): cells over 10 m / over 50 m; raised over 10 m', HO,
      lambda r: f"{int(r['lowered_vs_ref_over10']):,} / {int(r['lowered_vs_ref_over50']):,}; {int(r['raised_vs_ref_over10']):,}", 'ho')
table('`S0`\'s ridge cells lowered over 10 m / over 50 m (against the reference)', HO, lambda r: f"{int(r['s0_ridges_lowered10']):,} / {int(r['s0_ridges_lowered50']):,}", 'ho')
table('`S0`\'s breach cells (over 1 m) with no breach of the surface within 4 L cells, by `S0` class: base / K5 / interpolation / band / all, %', HO,
      lambda r: f"{float(r['fixed_base_pct']):.0f} / {float(r['fixed_k5_pct']):.0f} / {float(r['fixed_interp_pct']):.0f} / {float(r['fixed_band_pct']):.0f} / {float(r['fixed_all_pct']):.0f}", 'ho')
table('The surface\'s breach volume (cells over 1 m) by `S0`\'s class at the cell: none / base / K5 / interpolation / band, km3', HO,
      lambda r: f"{float(r['on_none_km3']):.2f} / {float(r['on_base_km3']):.2f} / {float(r['on_k5_km3']):.2f} / {float(r['on_interp_km3']):.2f} / {float(r['on_band_km3']):.2f}", 'ho')
print('\n**h4\'s overshoot beyond the corners of its macro triangle (over 1 cm), over the land of the box; h4 along every D8 step of the box\'s land (33 samples centre to centre)**\n')
print('| Window | Above: cells (%), max m | Below: cells (%), max m | D8 steps | Rising over 1 mm along the step (max rise, m) |')
print('|---|---|---|---|---|')
for w in wins:
    n, o, om, u, um, s, rs, rm = h4[w]
    n = int(n)
    print(f"| {NAMES[w]} | {int(o):,} ({100 * int(o) / n:.2f} %), {float(om):.1f} | {int(u):,} ({100 * int(u) / n:.2f} %), {float(um):.1f} | {int(s):,} | {int(rs):,} ({float(rm):.3f}) |")
