import json, sys

lines = [json.loads(l) for l in open(sys.argv[1], encoding='utf-8') if l.strip()]
down = {}
rows = []
unmatched_up = []
for e in lines:
    k = e.get('k')
    if k == 'key':
        c = e['code']
        if e['down']:
            if c in down:
                rows.append((c, down[c], None, 'second down without up'))
            down[c] = e['t']
        else:
            if c in down:
                rows.append((c, down.pop(c), e['t'], ''))
            else:
                unmatched_up.append((c, e['t']))
    elif k == 'focus' and not e['on']:
        for c, t in list(down.items()):
            rows.append((c, t, e['t'], 'cleared by focus loss'))
            down.pop(c)
for c, t in down.items():
    rows.append((c, t, None, 'still down at the end'))
rows.sort(key=lambda r: r[1])
print('key presses: code, down at s, held s, note')
for c, t0, t1, note in rows:
    held = f'{(t1 - t0)/1e6:7.2f}' if t1 else '   ----'
    print(f'  {c:11s} {t0/1e6:8.1f} {held} {note}')
print('up events without a down:', unmatched_up)
long = [(c, (t1 - t0)/1e6) for c, t0, t1, n in rows if t1 and (t1 - t0) > 10e6]
print('presses longer than 10 s:', long)
