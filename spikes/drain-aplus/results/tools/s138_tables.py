"""SPIKE-138 (spikes #143, #145): the tables from kh_smooth's ladder files.
Inputs in <dir>: ladder-<w>.csv (sel-post base + A+ band, F and the reading-23 forms), bc-ladder-<w>.csv (the same with the band drain capped at 50 m; M1 and R1h),
base-sel-post-<w>.csv and base-sel-clamp-<w>.csv (--base-only), today-ladder-<w>.csv (today's base, #141's chain), and #141's remain-ladder-<w>.csv for the comparison.
Usage: s138_tables.py <dir> <dir141> r1h r3a r3b m1 f1 > tables.md"""
import os
import sys

d, d141, wins = sys.argv[1], sys.argv[2], sys.argv[3:]
NAMES = {'r1h': 'R1h', 'r3a': 'R3a', 'r3b': 'R3b', 'm1': 'M1', 'f1': 'F1'}
CLASSES = ['base', 'K5', 'interpolation', 'band']


def lines(path):
    if not os.path.exists(path):
        return []
    return [l.strip().split(',') for l in open(path, encoding='utf-8') if l.startswith('#')]


def remain(path):
    # #remain, tag, class, river cells, in closed depressions, in G-DRAIN basins, over 5 m, breach cells, breach km3
    out = {}
    for f in lines(path):
        if f[0] == '#remain':
            out.setdefault(f[1], {})[f[2]] = (int(f[3]), int(f[4]), int(f[5]), int(f[6]), int(f[7]), float(f[8]))
    return out


def first(path, key):
    for f in lines(path):
        if f[0] == key:
            return f
    return None


def pct(a, b):
    return 100.0 * a / b if b else 0.0


print('**Kept H1 basins holding trunk cells in the box after K5 (SPIKE-134 census): basins (trunk km), of them deeper than the budget at the floor**\n')
print('| Window | Today | sel-post | sel-clamp |')
print('|---|---|---|---|')
for w in wins:
    cells = []
    for path in (f'{d}/today-ladder-{w}.csv', f'{d}/base-sel-post-{w}.csv', f'{d}/base-sel-clamp-{w}.csv'):
        c = first(path, '#census')
        cells.append(f'{int(c[2]):,} ({float(c[4]):,.0f} km), {int(c[5]):,}' if c else '-')
    print(f'| {NAMES[w]} | ' + ' | '.join(cells) + ' |')

print('\n**The selective pre-K5 drain of `z_b` (same for both forms), land of the box: cells lowered / raised over 1 m; over 50 m; km3 lowered / raised; `z_b` ridge cells crossed (lowered over 1 m / over 20 m)**\n')
print('| Window | Cells over 1 m | Over 50 m | km3 | Ridge cells crossed |')
print('|---|---|---|---|---|')
for w in wins:
    b = first(f'{d}/base-sel-post-{w}.csv', '#base')
    # #base, tag, land, lo1, hi1, lo50, hi50, km3 lo, km3 hi, ridge_n, crossed1, crossed20, k5 lo1, hi1, lo50, hi50, km3 lo, km3 hi, relief zb, zbin, zs, std zb, zbin, zs
    print(f'| {NAMES[w]} | {int(b[3]):,} / {int(b[4]):,} | {int(b[5]):,} / {int(b[6]):,} | {float(b[7]):.1f} / {float(b[8]):.1f} | {int(b[10]):,} / {int(b[11]):,} of {int(b[9]):,} |')

print('\n**K5\'s work (`z_s` - the base it starts from), land of the box: km3 lowered / raised; cells lowered over 50 m; relief of `z_s` (p50 of the 5-cell block range, m)**\n')
print('| Window | Today | sel-post | sel-clamp |')
print('|---|---|---|---|')
for w in wins:
    cells = []
    for path in (f'{d}/today-ladder-{w}.csv', f'{d}/base-sel-post-{w}.csv', f'{d}/base-sel-clamp-{w}.csv'):
        b = first(path, '#base')
        cells.append(f'{float(b[16]):,.0f} / {float(b[17]):,.0f}; {int(b[14]):,}; {float(b[20]):.0f}' if b else '-')
    print(f'| {NAMES[w]} | ' + ' | '.join(cells) + ' |')

print('\n**A+: the band drained on KH(`z_s`) (h4c or h5), land of the box: pits of KH + `r` (any / G-DRAIN / over 5 m; km3) -> of KH + `r\'\'`; band lowered / raised (km3); band std, m; `S0` ridge cells lowered over 20 m**\n')
print('| Window | Band drain | Pits before | Pits after | Band lowered / raised, km3 | Band std, m | Ridge cells over 20 m |')
print('|---|---|---|---|---|---|---|')
for w in wins:
    for label, path in (('h4c, uncapped', f'{d}/ladder-{w}.csv'), ('h5, uncapped', f'{d}/ladder-h5-{w}.csv'), ('h4c, capped 50 m', f'{d}/bc-ladder-{w}.csv')):
        b = first(path, '#band')
        if not b:
            continue
        # #band, tag, rounds, depr, filled, breached, left, b_any, b_g, b_5, b_km3, a_any, a_g, a_5, a_km3, sd1, sd2, rel1, rel2, srel1, srel2, lo, lv, lo20, hi, hv, ridge_n, r20
        print(f'| {NAMES[w]} | {label} | {int(b[7]):,} / {int(b[8]):,} / {int(b[9]):,} ({float(b[10]):.1f}) | {int(b[11]):,} / {int(b[12]):,} / {int(b[13]):,} ({float(b[14]):.1f}) | '
              f'{float(b[22]):.1f} / {float(b[25]):.1f} | {float(b[15]):.2f} -> {float(b[16]):.2f} | {int(b[27]):,} |')

CL = ['base', 'K5', 'interpolation', 'band']


def gdrain(rows):
    if not rows:
        return None
    return pct(sum(rows[k][2] for k in CL), rows['base'][0])


def three(path, t, f=gdrain, fmt=lambda x: f'{x:.2f}'):
    r = remain(path)
    v = [f(r.get(t)), f(r.get(f'{t}-r23c')), f(r.get(f'{t}-r23p'))]
    return '-' if all(x is None for x in v) else ' / '.join('-' if x is None else fmt(x) for x in v)


print("\n**River length (75 km2 channels) in G-DRAIN basins (at least 0.5 m deep and 16 cells), % of the river. #141: today's base, KH(h4c) + r' and KS1. "
      "Then the new base (sel-post) with A+ and KS1 / with reading 23 clamp / with reading 23 post, for KH h4c, KH h5, and KH h4c with the band drain capped at 50 m**\n")
print('| Window | #141 | h4c | h5 | h4c, band capped 50 m |')
print('|---|---|---|---|---|')
for w in wins:
    g141 = gdrain(remain(f'{d141}/remain-ladder-{w}.csv').get(w))
    print(f'| {NAMES[w]} | ' + ('-' if g141 is None else f'{g141:.2f}') + f' | {three(f"{d}/ladder-{w}.csv", w)} | {three(f"{d}/ladder-h5-{w}.csv", w + "-h5")} | '
          f'{three(f"{d}/bc-ladder-{w}.csv", w + "-bc")} |')


def cls(rows):
    if not rows:
        return None
    tot = sum(rows[k][2] for k in CL)
    return 'none' if tot == 0 else ' / '.join(f'{pct(rows[k][2], tot):.0f}' for k in CL)


print('\n**By origin class: river cells in G-DRAIN basins, base / K5 / interpolation / band, % of the remainder; #141, then KH h5 on the new base with A+ and KS1 / reading 23 clamp / reading 23 post**\n')
print('| Window | #141 | h5 + KS1 | h5 + clamp | h5 + post |')
print('|---|---|---|---|---|')
for w in wins:
    r = remain(f'{d}/ladder-h5-{w}.csv')
    c141 = cls(remain(f'{d141}/remain-ladder-{w}.csv').get(w)) or '-'
    print(f'| {NAMES[w]} | {c141} | ' + ' | '.join(cls(r.get(t)) or '-' for t in (f'{w}-h5', f'{w}-h5-r23c', f'{w}-h5-r23p')) + ' |')


def vol(rows):
    return None if not rows else sum(rows[k][5] for k in CL)


print('\n**Breach volume of the final surface (cells over 1 m), km3: #141; KS1 / clamp / post on the new base, KH h4c and h5**\n')
print('| Window | #141 | h4c | h5 |')
print('|---|---|---|---|')
for w in wins:
    v141 = vol(remain(f'{d141}/remain-ladder-{w}.csv').get(w))
    print(f'| {NAMES[w]} | ' + ('-' if v141 is None else f'{v141:.3f}') + f' | {three(f"{d}/ladder-{w}.csv", w, vol, lambda x: f"{x:.3f}")} | '
          f'{three(f"{d}/ladder-h5-{w}.csv", w + "-h5", vol, lambda x: f"{x:.3f}")} |')
