"""SPIKE-119: the GLO-30 benchmark's per-tile table from dem_prep.py and grid_gate (main's river gate) outputs.
Usage: bench_table.py <bench dir> <out.csv>   (prints a markdown table; the csv holds every number)
"""
import sys, re, csv, os
import numpy as np

D = sys.argv[1]
LAND = {
    'N27E086': ('Himalaya (Everest)', 'high', 0.687), 'N40W078': ('Appalachian Valley and Ridge', 'high', 0.217), 'N42W123': ('Crater Lake, Cascades: volcanic', 'high', 0.275),
    'N45E014': ('Kras, Slovenia: karst', 'high', 0.295), 'N47E011': ('Alps (Tyrol)', 'high', 0.591), 'N48E008': ('Black Forest', 'high', 0.2165),
    'N57W005': ('Scottish Highlands: glaciated', 'high', 0.242), 'N61E006': ('Sognefjord: fjord, glacial troughs', 'high', 0.679),
    'N31W004': ('Erg Chebbi: dunes, desert', 'mid', 0.078), 'N41W096': ('Loess hills (Iowa/Nebraska)', 'mid', 0.0368), 'N43W102': ('Badlands, South Dakota', 'mid', 0.0452),
    'S25E015': ('Namib Sand Sea', 'mid', 0.1127), 'N35W076': ('Outer Banks: barrier coast', 'low', 0.0082), 'N52E012': ('Brandenburg: glacial plain', 'low', 0.0268),
    'N70W154': ('Alaska North Slope: tundra, thaw lakes', 'low', 0.0131),
    'N33W092': ('NEW Lower Mississippi: broad floodplain', None, None), 'N36W113': ('NEW Grand Canyon: incised plateau', None, None),
    'N46W122': ('NEW Cascades (St. Helens): volcanic, forest', None, None), 'S44E170': ('NEW Southern Alps (Mackenzie): glacial, braided', None, None),
    'N36W117': ('NEW Death Valley: arid mountains', None, None), 'S04W061': ('NEW Amazon at Manaus: large river, rain forest', None, None),
}
SEA = {'N36W117': -200}


def slope_p95(z):
    h, w = z.shape
    b = 8
    zb = z[:h // b * b, :w // b * b].reshape(h // b, b, w // b, b).mean((1, 3))
    c = zb[1:-1, 1:-1]
    m = np.zeros_like(c)
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            if dx == 0 and dy == 0:
                continue
            d = 814.4 * (2 ** 0.5 if dx and dy else 1)
            nb = zb[1 + dy:zb.shape[0] - 1 + dy, 1 + dx:zb.shape[1] - 1 + dx]
            m = np.maximum(m, (c - nb) / d)
    s = m[c > 0]
    return float(np.percentile(s, 95)) if s.size else float('nan')


def num(pat, text, k=1, f=float):
    m = re.search(pat, text)
    return f(m.group(k)) if m else None


rows = []
sizes = {l.split()[0]: (int(l.split()[1]), int(l.split()[2])) for l in open(os.path.join(D, 'sizes.txt')) if len(l.split()) == 3}
for tag in sorted(sizes):
    kind, tile = tag.split('-')
    prep = open(os.path.join(D, tag + '.prep.txt')).read()
    gate = open(os.path.join(D, tag + '.gate.txt')).read()
    w, h = sizes[tag]
    z = np.fromfile(os.path.join(D, tag + '.f64'), '<f8').reshape(h, w)
    name, pool, frozen = LAND[tile]
    sp = slope_p95(z)
    if pool is None:
        pool = 'high' if sp >= 0.15 else ('mid' if sp >= 0.03 else 'low')
        pool += ' (placed)'
    r = dict(
        source='GLO-30' if kind == 'glo' else '3DEP DTM', tile=tile, landform=name, pool=pool, slope_p95_frozen=frozen, slope_p95_here=round(sp, 4),
        pixels=re.search(r'(\d+ x \d+) pixels', prep).group(1), spacing=re.search(r'\(([\d.]+" x [\d.]+")', prep).group(1),
        px_m=num(r', ([\d.]+) x ([\d.]+) m at lat', prep), py_m=num(r', ([\d.]+) x ([\d.]+) m at lat', prep, 2), lat_c=num(r'm at lat (-?[\d.]+)', prep),
        nodata_px=num(r'no-data pixels (\d+)', prep, f=int), cells=f'{w} x {h}', px_per_cell=re.search(r'pixels a cell ([\d.]+ x [\d.]+)', prep).group(1),
        sea_level=SEA.get(tile, 0),
        cells_inside=num(r'counts: (\d+) cells inside', gate, f=int), land=num(r'inside, (\d+) land', gate, f=int), sea_or_outlet=num(r'land, (\d+) sea', gate, f=int),
        river_cells=num(r'river cells \(area >= 75 km2\) (\d+)', gate, f=int), river_1000=num(r'>= 1,000 km2 (\d+)', gate, f=int),
        area_max_km2=num(r'largest drained area inside (\d+)', gate, f=int),
        basins_pct=num(r'basins: ([\d.]+) %', gate), basins_cells=num(r'basins: [\d.]+ % of river length in closed basins \((\d+) of', gate, f=int),
        basins_n=num(r'cells; (\d+) basins of', gate, f=int), pit5cm_pct=num(r'any pit of 0.05 m\): ([\d.]+) %', gate),
        valley_12_pct=num(r'valley depth: ([\d.]+) %', gate), valley_12_cells=num(r'valley depth: [\d.]+ % of river cells .*?; (\d+) cells', gate, f=int),
        valley_24_pct=num(r'ring 2-4: ([\d.]+) %', gate), valley_24_cells=num(r'ring 2-4: [\d.]+ % of river cells .*?\((\d+) cells', gate, f=int),
        burn_pct=num(r'burn: ([\d.]+) %', gate),
        lakes_pct=num(r'lakes: ([\d.]+) %', gate), lakes_cells=num(r'lakes at D 20 m \((\d+) cells', gate, f=int),
        relief_2=num(r'over 2 x 2 cells ([\d.]+) m', gate), relief_10=num(r'over 10 x 10 cells ([\d.]+) m', gate),
        uphill_pct=num(r'gradient: \d+ of \d+ river steps rise by more than 0.25 m \(([\d.]+) %', gate),
        width10_p50=num(r'at 10 m above the river cell p50 (\d+) m', gate), width10_p90=num(r'at 10 m above the river cell p50 \d+ m p90 (\d+) m', gate),
        widthhalf_p50=num(r'at half the ring 2-4 depth p50 (\d+) m', gate),
        asym_p50=num(r'asymmetry: .*? p50 ([\d.]+)', gate), density=num(r'drainage density: ([\d.]+) km/km2', gate),
        narrow_p50=num(r'narrowness: .*? p50 ([\d.]+)', gate), theta=num(r'slope-area: theta (-?[\d.]+)', gate), beta=num(r'slope beta (-?[\d.]+)', gate),
    )
    rows.append(r)
with open(sys.argv[2], 'w', newline='') as f:
    wr = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
    wr.writeheader()
    wr.writerows(rows)
def f(v, d=2):
    return '-' if v is None else f'{v:,.{d}f}'


print('| Source | Tile | Landform | Pool | Slope p95 (frozen / here) | px a cell | Land cells | River cells (>= 1,000 km2) | Largest area km2 | Basins % (cells, basins) | Valley depth 1-2 / ring 2-4 % (cells) | Lakes % (cells) | Relief 2x2 / 10x10 m |')
print('|---|---|---|---|---|---|---|---|---|---|---|---|---|')
for r in rows:
    fr = f"{r['slope_p95_frozen']}" if r['slope_p95_frozen'] is not None else '-'
    print(f"| {r['source']} | {r['tile']} | {r['landform']} | {r['pool']} | {fr} / {r['slope_p95_here']} | {r['px_per_cell']} | {r['land']:,} | {r['river_cells']:,} ({r['river_1000']:,}) | {r['area_max_km2']:,} | "
          f"{r['basins_pct']:.2f} ({r['basins_cells']:,}, {r['basins_n']:,}) | {f(r['valley_12_pct'])} / {f(r['valley_24_pct'])} ({r['valley_12_cells']:,} / {r['valley_24_cells']:,}) | {r['lakes_pct']:.2f} ({r['lakes_cells']:,}) | {r['relief_2']:.1f} / {r['relief_10']:.1f} |")
print()
print('| Source | Tile | Uphill steps % | Width at 10 m p50 / p90 (m) | Width at half ring depth p50 (m) | Asymmetry p50 | Drainage density km/km2 | Narrowness p50 | Slope-area theta | Spectrum beta |')
print('|---|---|---|---|---|---|---|---|---|---|')
for r in rows:
    print(f"| {r['source']} | {r['tile']} | {f(r['uphill_pct'])} | {f(r['width10_p50'], 0)} / {f(r['width10_p90'], 0)} | {f(r['widthhalf_p50'], 0)} | {f(r['asym_p50'])} | {f(r['density'], 3)} | {f(r['narrow_p50'])} | {f(r['theta'])} | {f(r['beta'])} |")
