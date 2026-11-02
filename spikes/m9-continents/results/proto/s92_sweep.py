import json, os, subprocess, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from s92_score import score
from s92_shape import shape
import numpy as np
X = './target/release/examples/preview_world.exe'
def run(name, env):
    rows = []
    for seed in range(1, 6):
        o = f'target/s92/{name}/seed-{seed}'; os.makedirs(o, exist_ok=True)
        subprocess.run([X, '--seed', str(seed), '--width', '1440', '--out', o], env={**os.environ, **env}, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
        r = score(o + '/land.u8'); sh = shape(np.fromfile(o + '/land.u8', dtype=np.uint8).reshape(720, 1440) > 0)
        r.update(s9=sh['s9'], s10=sh['s10']); rows.append(r)
    ok = lambda r: (4 <= r['s1'] <= 7, 0.45 <= r['s2'] <= 0.62, r['s3'] >= 1.18, 0.27 <= r['land'] <= 0.31, r['s9'] <= 0.20, 1.5 <= r['s10'] <= 2.5)
    passes = [ok(r) for r in rows]
    print(f"{name:28s} S1 {sum(p[0] for p in passes)}/5 S2 {sum(p[1] for p in passes)}/5 S3 {sum(p[2] for p in passes)}/5 S9 {sum(p[4] for p in passes)}/5 S10 {sum(p[5] for p in passes)}/5 all {sum(all(p) for p in passes)}/5 | " + ' '.join(f"[{r['s1']},{r['s2']:.2f},{r['s3']:.2f},{r['s9']:.2f},{r['s10']:.1f}]" for r in rows), flush=True)
    json.dump(rows, open(f'target/s92/{name}/scores.json', 'w'))
for spec in sys.argv[1:]:
    name, *kv = spec.split('|')
    run(name, dict(x.split('=') for x in kv))
