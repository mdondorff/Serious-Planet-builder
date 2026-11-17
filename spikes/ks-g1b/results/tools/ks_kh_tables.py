"""SPIKE-128 addendum (spikes #140): KS on KH(h4c) + r' against KS on C1(z_s) + r (SPIKE-128), five windows, H from today's K6.
Inputs in <dir>: `ks-kh-<w>.csv` (ks_kh --kh h4c), `ks-c1-<w>.csv` (SPIKE-128's ks.csv, ks-g1b2/h1), `split-kh-<w>.txt` and `split-c1-<w>.txt` (the `#ks` lines of SPIKE-135's
probe on each run's KS lowering). Usage: ks_kh_tables.py <dir> r1h r3a r3b m1 f1 > ks-kh-tables.md"""
import csv
import sys

d, wins = sys.argv[1], sys.argv[2:]
NAMES = {'r1h': 'R1h', 'r3a': 'R3a', 'r3b': 'R3b', 'm1': 'M1', 'f1': 'F1'}


def load(path):
    rows = {}
    with open(path, encoding='utf-8') as f:
        lines = [l for l in f if l.strip()]
    head = lines[0].strip().split(',')
    for l in lines[1:]:
        v = l.strip().split(',')
        # the window field holds commas: rung, then six window numbers, then the rest
        rec = dict(zip(head[:1] + head[2:], [v[0]] + v[7:]))
        rows[v[0]] = rec
    return rows


kh = {w: load(f'{d}/ks-kh-{w}.csv') for w in wins}
c1 = {w: load(f'{d}/ks-c1-{w}.csv') for w in wins}


def split(w, which):
    out = {}
    for l in open(f'{d}/split-{which}-{w}.txt', encoding='utf-8'):
        f = l.strip().split(',')
        if f[0] == '#ks' and 'today' in f[2]:
            out[f[2].split()[1]] = [float(x) for x in f[3:]]
    return out


def pct(a, b):
    return 100.0 * float(a) / max(1.0, float(b))


def table(title, rungs, fmt):
    print(f'\n**{title}**\n')
    print('| Start | Stage | ' + ' | '.join(NAMES[w] for w in wins) + ' |')
    print('|---|---|' + '---|' * len(wins))
    for label, src in (('`C1(z_s)` + `r` (SPIKE-128)', c1), ('KH(h4c) + `r\'`', kh)):
        for rg in rungs:
            print(f'| {label} | {rg} | ' + ' | '.join(fmt(src[w][rg]) for w in wins) + ' |')


table('KS\'s lowering: land cells lowered, %; p95, m; volume / requested, km3', ['RG-1', 'RG-2'],
      lambda r: f"{pct(r['lowered'], r['land']):.2f} %; {float(r['low_p95_m']):.1f}; {float(r['vol_km3']):.2f} / {float(r['vol_req_km3']):.2f}")
table('Residuals (need over the budget), cells; at 95-100 % of the budget or residual, cells; land at or below the sea after the stage, cells', ['RG-0', 'RG-1', 'RG-2'],
      lambda r: f"{int(r['resid']):,}; {int(r['at95']):,}; {int(r['to_sea']):,}")
table('`S0`\'s ridge cells lowered over 20 m against `S0` (of the ridge cells); cut-through saddles at 1 km2 (cells; not measured at RG-0)', ['RG-0', 'RG-1', 'RG-2'],
      lambda r: f"{int(r['ridge20']):,} ({pct(r['ridge20'], r['ridge_n']):.2f} %); " + ('-' if r['rung'] == 'RG-0' else f"{int(r['saddle1']):,} ({int(r['saddle1_cells']):,})"))
table('River gate: basins; G-DRAIN, % of river cells in basins; relief10 / `S0`\'s, m; uphill, %', ['RG-0', 'RG-1', 'RG-2'],
      lambda r: f"{int(r['basins']):,}; {float(r['basin_pct']):.1f}; {float(r['relief10_m']):.1f} / {float(r['relief10_start_m']):.1f}; {float(r['uphill_pct']):.1f}")

print('\n**SPIKE-135\'s split of KS\'s lowering by the cell\'s `S0` class: volume, km3; share by class (no `S0` breach / base / K5 / interpolation / band), %; within 4 L cells of an `S0` breach cell, %; the part `S0`\'s breach also asks for, %**\n')
print('| Window | Stage | `C1(z_s)` + `r` | KH(h4c) + `r\'` |')
print('|---|---|---|---|')
for w in wins:
    a, b = split(w, 'c1'), split(w, 'kh')
    for rg in ('rg1', 'rg2'):
        f = lambda v: f"{v[0]:.2f}; {v[1]:.0f} / {v[2]:.1f} / {v[3]:.1f} / {v[4]:.0f} / {v[5]:.1f}; {v[6]:.0f}; {v[7]:.0f}"
        print(f"| {NAMES[w]} | {rg.upper().replace('RG', 'RG-')} | {f(a[rg])} | {f(b[rg])} |")
