"""SPIKE-127: the per-window table from the g1a_sills logs (`<dir>/<w>.log.out`, `<dir>/<w>.log`). Usage: g1a_table.py <dir> <out.csv>"""
import sys
import re
import csv

WINS = ['r1h', 'r3a', 'r3b', 'm1', 'f1']


def num(pat, text, k=1, cast=float):
    m = re.search(pat, text)
    return cast(m.group(k)) if m else None


def topo(text, name):
    line = next((l for l in text.splitlines() if l.startswith('G1A ') and f'{name}:' in l), '')
    r = {}
    m = re.search(r'reaches (\d+) -> (\d+) \((\d+) of the first not in the second, ([\d.]+) %; (\d+) new; (\d+) of the first and (\d+) of the second', line)
    if m:
        r.update(reaches_a=int(m[1]), reaches_b=int(m[2]), reaches_changed_pct=float(m[4]), reaches_moved1_a=int(m[6]), reaches_moved1_b=int(m[7]))
    m = re.search(r'junctions (\d+) -> (\d+) \((\d+) moved, (\d+) by more than 1 cell\)', line)
    if m:
        r.update(junctions_a=int(m[1]), junctions_b=int(m[2]), junctions_moved1=int(m[4]))
    m = re.search(r'outlets (\d+) -> (\d+) \((\d+) moved, (\d+) by more than 1 cell\)', line)
    if m:
        r.update(outlets_a=int(m[1]), outlets_b=int(m[2]), outlets_moved1=int(m[4]))
    m = re.search(r'trunk cells (\d+) -> (\d+) \((\d+) of the second off the first\'s cells, ([\d.]+) %; (\d+) more than 1 cell off, ([\d.]+) %\)', line)
    if m:
        r.update(cells_a=int(m[1]), cells_b=int(m[2]), cells_off_pct=float(m[4]), cells_off1_pct=float(m[6]))
    return r


def main():
    d, out = sys.argv[1], sys.argv[2]
    rows = []
    for w in WINS:
        t = open(f'{d}/{w}.log.out', encoding='utf-8').read()
        head = open(f'{d}/{w}.log', encoding='utf-8-sig').read()
        row = dict(window=w)
        row['run_s'] = num(r'BAKE-TIME ([\d,.]+) s', head, cast=lambda s: float(s.replace(',', '')))
        row['peak_gb'] = num(r'peak working set ([\d,]+) GB', head, cast=lambda s: float(s.replace(',', '.')))
        row['s0_s'] = num(r'G1A S0: .* in ([\d.]+) s', t)
        row['sill_s'] = num(r'edges in ([\d.]+) s', t)
        row['sill_zero_pct'] = num(r'zero on ([\d.]+) %', t)
        row['sill_p90_m'] = num(r'zero on [\d.]+ %, p50 [\d.]+ p90 ([\d.]+)', t)
        row['sill_p99_m'] = num(r'p90 [\d.]+ p99 ([\d.]+) max', t)
        row['zs_minus_h_p50_m'] = num(r'z_s - h p50 ([\d.-]+)', t)
        row['basins_today'] = num(r'basins \(kept, H1\): today (\d+)', t, cast=int)
        row['basins_k6p'] = num(r"K6' (\d+)$", t.split('G1A basins')[1].splitlines()[0] if 'G1A basins' in t else '', cast=int)
        for key, name in (('ctrl', 'today -> control (this code on z_s, no regrade)'), ('k6p', "today -> K6' (sills of S0)"),
                          ('mfdall', "today -> K6' with the MFD over all lower neighbours"), ('base_zs', 'today -> today with z_s +-0.5 m (baseline)'),
                          ('sens_s0', "K6' -> K6' with S0 +-0.5 m"), ('sens_sill', "K6' -> K6' with sills +-1 m")):
            for k, v in topo(t, name).items():
                row[f'{key}_{k}'] = v
        for key, name in (('ctrl', 'today -> control'), ('k6p', "today -> K6'"), ('base_zs', 'today -> z_s +-0.5 m'), ('sens_s0', "K6' -> S0 +-0.5 m"),
                          ('sens_sill', "K6' -> sills +-1 m")):
            m = re.search(rf"G1A catchments {re.escape(name)}: of ([\d.]+) km2 .*?\(([\d.]+) %\) drain .*?\(([\d.]+) %\) more than 10", t)
            if m:
                row[f'{key}_catch2_pct'], row[f'{key}_catch10_pct'] = float(m[2]), float(m[3])
            m = re.search(rf"G1A major {re.escape(name)}: (\d+) of (\d+) changed", t)
            if m:
                row[f'{key}_major_changed'] = int(m[1])
        for key, name in (('today', 'today'), ('ctrl', 'control'), ('k6p', "K6'")):
            m = re.search(rf"G1A K5 channels {re.escape(name)}: \d+ of \d+ trunk cells in the box off K5's channel cells \(([\d.]+) %\), \d+ more than 1 cell off \(([\d.]+) %\)", t)
            if m:
                row[f'k5off_{key}_pct'], row[f'k5off1_{key}_pct'] = float(m[1]), float(m[2])
        for key, name in (('today', 'today'), ('k6p', "K6'"), ('mfdall', "K6' MFD over all lower neighbours")):
            m = re.search(rf"G1A trunk stats {re.escape(name)}: (\d+) heads in the box \((\d+) with a D8 area under 75 km2.*?\), (\d+) km of trunk, (\d+) cells", t)
            if m:
                row[f'heads_{key}'], row[f'heads_side_{key}'], row[f'trunk_km_{key}'] = int(m[1]), int(m[2]), int(m[3])
        for key, name in (('k6p', "today -> K6'"), ('ctrl', 'today -> control'), ('base_zs', 'today -> z_s +-0.5 m'), ('sens_s0', "K6' -> S0 +-0.5 m"),
                          ('sens_sill', "K6' -> sills +-1 m")):
            m = re.search(rf"G1A stop test {re.escape(name)}: (\d+) of (\d+) top rivers", t)
            if m:
                row[f'stop_{key}'], row[f'stop_{key}_n'] = int(m[1]), int(m[2])
            rows_ = [l for l in t.splitlines() if l.startswith(f'G1A stop test {name}:   ')]
            row[f'stop_{key}_outlet_changed'] = sum('CHANGED' in l for l in rows_)
            row[f'stop_{key}_lost'] = sum(int(re.search(r'lost (\d+)', l)[1]) for l in rows_)
            row[f'stop_{key}_gained'] = sum(int(re.search(r'gained (\d+)', l)[1]) for l in rows_)
            row[f'stop_{key}_conf'] = sum(int(re.search(r'(\d+) major confluences', l)[1]) for l in rows_)
        row['bar_m'] = num(r'G1A bar ([\d.]+) m', t)
        for g, gk in (('today', 'today'), ("K6'", 'k6p'), ('control', 'ctrl'), ('S0+-0.5', 'sens_s0'), ('sills+-1', 'sens_sill')):
            for s in ('S0', 'B'):
                m = re.search(rf"G1A corridor {re.escape(g)} on {s}: (\d+) of (\d+) reaches pass \(([\d.]+) %\), (\d+) fail", t)
                if m:
                    row[f'corr_{gk}_{s}_pass_pct'], row[f'corr_{gk}_{s}_fail'], row[f'corr_{gk}_{s}_n'] = float(m[3]), int(m[4]), int(m[2])
        row['d2_list'] = num(r'G1A D-2 list: (\d+) reaches', t, cast=int)
        row['d2_land_fail'] = num(r'; (\d+) still fail on B', t, cast=int)
        row['d2_rejoins'] = num(r"\(\"rejoins\", (\d+)\)", t, cast=int)
        row['d2_diverted'] = num(r"\(\"diverted\", (\d+)\)", t, cast=int)
        row['d2_outlet'] = num(r"\(\"outlet\", (\d+)\)", t, cast=int)
        row['d2_gain_b'] = num(r'(\d+) gain a route under the bar on B', t, cast=int)
        row['d2_gain_s0'] = num(r'on B, (\d+) on S0', t, cast=int)
        rows.append(row)
    keys = []
    for r in rows:
        for k in r:
            if k not in keys:
                keys.append(k)
    with open(out, 'w', newline='') as f:
        wr = csv.DictWriter(f, fieldnames=keys)
        wr.writeheader()
        wr.writerows(rows)
    # transposed print
    for k in keys:
        print(f'{k:32s}', ' '.join(f'{str(r.get(k, "")):>10s}' for r in rows))


if __name__ == '__main__':
    main()
