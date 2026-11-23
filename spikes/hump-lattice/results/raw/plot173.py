"""SPIKE-142: the crest phase histograms (mod 8 = 814 m, mod 6 = 611 m) of R1h's 2,373 hump trenches against reference sets."""
import sys, numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
out = sys.argv[1]
# shares in % from r1h-lattice.txt / r1h-lattice-b.txt (g1f15 R1h)
p8 = {
    'trench crests on S0': ([19.3, 12.5, 8.8, 9.7, 9.0, 9.1, 12.3, 19.4], [19.3, 12.5, 8.6, 9.4, 9.7, 9.4, 10.3, 20.9]),
    'trench crests on S_KH': ([15.9, 12.5, 9.4, 12.0, 10.4, 11.1, 12.1, 16.6], [16.0, 12.6, 9.3, 11.7, 11.3, 10.0, 11.1, 17.8]),
    'every trench path cell': ([12.4, 12.5, 12.5, 12.5, 12.6, 12.6, 12.5, 12.4], [12.1, 12.4, 12.7, 13.2, 13.2, 12.5, 12.0, 11.9]),
    'S0 local maxima': ([10.4, 10.9, 12.8, 15.7, 15.8, 12.9, 11.2, 10.5], [9.9, 10.8, 12.7, 16.5, 16.4, 12.7, 10.8, 10.1]),
}
p6 = {
    'trench crests on S_KH': ([15.3, 17.8, 16.6, 15.9, 15.8, 18.6], [16.1, 17.5, 15.6, 17.4, 16.1, 17.3]),
    'S0 local maxima': ([22.3, 15.2, 11.7, 12.0, 16.1, 22.7], [20.1, 14.8, 12.8, 13.3, 17.0, 22.0]),
}
colors = {'trench crests on S0': '#c0392b', 'trench crests on S_KH': '#e67e22', 'every trench path cell': '#7f8c8d', 'S0 local maxima': '#2c7fb8'}
fig, ax = plt.subplots(2, 2, figsize=(11, 7), dpi=90)
for j, (axis, name) in enumerate((('x', 0), ('y', 1))):
    a = ax[0, j]
    for k, (xs, ys) in p8.items():
        a.plot(range(8), (xs, ys)[name], marker='o', label=k, color=colors[k])
    a.axhline(12.5, color='k', lw=0.8, ls=':')
    a.axvspan(-0.4, 0.4, color='#eee'); a.axvspan(6.6, 7.4, color='#eee')
    a.set_title(f'{axis} modulo 8 L cells (814 m, H1 cell; shaded: the cells at an H1 edge)', fontsize=9)
    a.set_ylabel('share (%)'); a.set_xticks(range(8))
    b = ax[1, j]
    for k, (xs, ys) in p6.items():
        b.plot(range(6), (xs, ys)[name], marker='o', label=k, color=colors[k])
    b.axhline(100 / 6, color='k', lw=0.8, ls=':')
    b.set_title(f'{axis} modulo 6 L cells (611 m, the base lattice)', fontsize=9)
    b.set_ylabel('share (%)'); b.set_xticks(range(6))
ax[0, 0].legend(fontsize=8, loc='upper center')
ax[1, 0].legend(fontsize=8, loc='upper center')
fig.suptitle('SPIKE-142, R1h (g1f15): where the 2,373 hump trenches cross their crest; dotted: uniform', fontsize=10)
fig.tight_layout()
fig.savefig(out)
