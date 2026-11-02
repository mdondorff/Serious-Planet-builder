"""SPIKE-92: score every target/s92/<config>/seed-N/land.u8 and print a markdown table plus raw JSON."""
import json, os, sys
sys.path.insert(0, os.path.dirname(__file__))
from s92_score import score
root = sys.argv[1]; raw = {}
print('| Config | land % (seeds 1-5) | S1 | S2 % | S3 | all pass |'); print('|---|---|---|---|---|---|')
for c in sorted(os.listdir(root)):
    rs = []
    for s in range(1, 6):
        p = f'{root}/{c}/seed-{s}/land.u8'
        if os.path.exists(p): rs.append(score(p))
    if len(rs) < 5: continue
    raw[c] = rs
    ok = [0.27 <= r['land'] <= 0.31 and 4 <= r['s1'] <= 7 and 0.45 <= r['s2'] <= 0.62 and r['s3'] >= 1.18 for r in rs]
    j = lambda f: ' '.join(f(r) for r in rs)
    cols = [j(lambda r: '%.0f' % (100 * r['land'])), j(lambda r: str(r['s1'])), j(lambda r: '%.0f' % (100 * r['s2'])), j(lambda r: '%.2f' % r['s3'])]
    print('| %s | %s | %d/5 |' % (c, ' | '.join(cols), sum(ok)))
json.dump(raw, open(sys.argv[2], 'w'), indent=1)
