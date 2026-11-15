"""River review (lead #118): one oblique contact sheet per pose: two production stills (stores r3a-e1 and r3a-p2e) and five start-surface obliques.
Usage: review_obl_sheet.py <pose: f43680|f33547> <out.jpg>
"""
import sys, os
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from PIL import Image

R = 'C:/DEV/Serious-Planet-Builder/.claude/worktrees/sonnet-spikes-36fc88/target/river-review/'
O = 'C:/Users/matth/AppData/Local/Temp/claude/56f790637f65dbcc/311fabe4-6176-40ef-9b59-bc0d7ae0c85f/scratchpad/obl/'
POSE = {
    'f43680': "frame-43680 (F3, canal-like river), low sun: eye 352 m above ground, 850 m before 38.90206 S 165.99522 W, heading 100.75, 12.1 deg down, sun 17.5 h",
    'f33547': "frame-33547 (F2, gorge, 'water flows uphill'), low sun: eye 102 m above ground, 1,200 m before 38.43330 S 166.16921 W, heading 38.96, 4.78 deg down, sun 17.5 h",
}


def main():
    p, out = sys.argv[1], sys.argv[2]
    cells = [
        (R + 'stills-r3a-e1-%s/%s-lowsun.png' % (p, p), 'PRODUCTION PATH: store r3a-e1 (today: B + regional, K7, H1 rivers)\nviewer at main 4b4dd86a, before the GEN-173 cap (kernel 7/9)'),
        (R + 'stills-r3a-p2e-%s/%s-lowsun.png' % (p, p), 'PRODUCTION PATH: store r3a-p2e (SPIKE-115 spike bake: one round at L, rivers routed at L;\nno river water drawn) viewer at spike/erosion-at-l 1000977a, the build that baked it'),
        (O + '%s-1-today.png' % p, 'start surface, no bake: today (B + regional)'),
        (O + '%s-2-round.png' % p, 'start surface, no bake: one round at L only'),
        (O + '%s-3-d4.png' % p, 'start surface, no bake: 4-neighbour drain only'),
        (O + '%s-4-d4round.png' % p, 'start surface, no bake: 4-neighbour drain + one round at L'),
        (O + '%s-5-schott.png' % p, 'start surface, no bake: Schott 2024 fair (2 rounds a scale)'),
    ]
    fig, ax = plt.subplots(2, 4, figsize=(26, 8.4))
    ax = ax.ravel()
    for k, (f, lab) in enumerate(cells):
        if os.path.exists(f):
            im = Image.open(f).convert('RGB')
            if k >= 2:  # the start-surface renders carry a caption strip on top: crop it (the label is the panel title)
                w, h = im.size
                im = im.crop((0, h - 540, w, h))
            im = im.resize((960, 540), Image.LANCZOS)
            ax[k].imshow(np.asarray(im))
        else:
            ax[k].text(0.5, 0.5, 'missing\n' + os.path.basename(f), ha='center', va='center')
        ax[k].set_title(lab, fontsize=9)
        ax[k].axis('off')
    ax[7].axis('off')
    ax[7].text(0.0, 0.95, 'The E1 region (R1h window: face 1, seed 14,\ni 8056-8675, j 760-1379); the pose lies in its margin.\n\n'
               'Production stills: planet test-render --scene capture-stills\n--adapter hardware (RTX 3080 Ti), 1920 x 1080 scaled to 960 x 540;\n'
               'the in-image scale bar is the renderer\'s.\n\n'
               'Start surfaces: the drain_base probe\'s whole-window dump at 101.8 m,\nray-marched in Python (bilinear heights, Lambert + sky light + cast\n'
               'shadows, no materials, no sub-cell detail); river cells blue\n(L route, area >= 75 km2). Same camera as capture-stills (fov 60).\n\n'
               'Rivers differ in kind: production draws K7\'s burned H1 lines,\nthe start surfaces show L-route cells.', fontsize=9, va='top', family='monospace')
    fig.suptitle(POSE[p], fontsize=11)
    fig.tight_layout(rect=[0, 0, 1, 0.96])
    fig.savefig(out, dpi=100, pil_kwargs={'quality': 85})
    print('wrote', out, os.path.getsize(out))


main()
