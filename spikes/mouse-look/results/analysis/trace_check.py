import json, math, sys

R = 1000.0

def dot(a, b): return a[0]*b[0] + a[1]*b[1] + a[2]*b[2]
def cross(a, b): return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
def add(a, b): return [a[0]+b[0], a[1]+b[1], a[2]+b[2]]
def sc(a, k): return [a[0]*k, a[1]*k, a[2]*k]
def ln(a): return math.sqrt(dot(a, a))
def nm(a): return sc(a, 1.0/ln(a))

def rotate(v, axis, ang):
    s, c = math.sin(ang), math.cos(ang)
    return add(add(sc(v, c), sc(cross(axis, v), s)), sc(axis, dot(axis, v)*(1-c)))

def pose_at(lat, lon, radius, alt, hdg):
    up = [math.cos(lat)*math.cos(lon), math.cos(lat)*math.sin(lon), math.sin(lat)]
    east = nm(cross([0, 0, 1], up))
    north = cross(up, east)
    heading = add(sc(north, math.cos(hdg)), sc(east, math.sin(hdg)))
    return {'pos': sc(up, radius+alt), 'heading': heading, 'pitch': 0.0}

LIMIT = 89.0*math.pi/180.0

def step(p, dyaw, dpitch, fwd, right, up):
    up_old = nm(p['pos'])
    heading = rotate(p['heading'], up_old, -dyaw)
    pitch = max(-LIMIT, min(LIMIT, p['pitch']+dpitch))
    rgt = nm(cross(heading, up_old))
    flat = add(sc(heading, fwd), sc(rgt, right))
    dist = ln(flat)
    r_old = ln(p['pos'])
    radius = max(r_old+up, 1e-9)
    if dist < 1e-12:
        up_new, hd = up_old, heading
    else:
        axis = nm(cross(up_old, sc(flat, 1.0/dist)))
        ang = dist/r_old
        up_new = nm(rotate(up_old, axis, ang))
        hd = nm(rotate(heading, axis, ang))
    hd = nm(add(hd, sc(up_new, -dot(hd, up_new))))
    return {'pos': sc(up_new, radius), 'heading': hd, 'pitch': pitch}

def basis(p):
    vert = nm(p['pos'])
    fwd = add(sc(p['heading'], math.cos(p['pitch'])), sc(vert, math.sin(p['pitch'])))
    right = nm(cross(fwd, vert))
    return vert, fwd, right

path = sys.argv[1]
lines = [json.loads(l) for l in open(path, encoding='utf-8') if l.strip()]
hdr = lines[0]
p = pose_at(hdr['lat'], hdr['lon'], hdr['radius'], hdr['alt'], hdr['hdg'])
sens = hdr['sens']
keys = set()
accum = [0.0, 0.0]
captured = False
frames = []
anoms = []
min_alt = 1e9
max_lat = 0.0
total_yaw = 0.0
held_log = []
for e in lines[1:]:
    k = e.get('k')
    if k == 'key':
        if e['down']:
            keys.add(e['code'])
        else:
            keys.discard(e['code'])
    elif k == 'focus':
        keys.clear()
    elif k == 'set':
        sens = e['sens']
    elif k == 'reset':
        p = pose_at(hdr['lat'], hdr['lon'], hdr['radius'], hdr['alt'], hdr['hdg'])
    elif k == 'frame':
        dt = e['dt_us']/1e6
        held = lambda c: 1.0 if c in keys else 0.0
        dyaw = (held('ArrowRight')-held('ArrowLeft'))*1.5*dt
        dpitch = (held('ArrowUp')-held('ArrowDown'))*1.5*dt
        alt = max(ln(p['pos'])-hdr['radius'], 0.0)
        speed = 5.0*max(alt/2.0, 1.0)
        fwd = (held('KeyW')-held('KeyS'))*speed*dt
        rgt = (held('KeyD')-held('KeyA'))*speed*dt
        upm = (held('KeyE')-held('KeyQ'))*speed*dt
        vert, f, r = basis(p)
        before = p
        p = step(p, dyaw, dpitch, fwd, rgt, upm)
        total_yaw += dyaw
        d = [p['pos'][i]-before['pos'][i] for i in range(3)]
        a = ln(before['pos'])-hdr['radius']
        min_alt = min(min_alt, ln(p['pos'])-hdr['radius'])
        max_lat = max(max_lat, abs(math.degrees(math.asin(p['pos'][2]/ln(p['pos'])))))
        if fwd or rgt or upm:
            cf = dot(d, before['heading']); cr = dot(d, r); cu = dot(d, vert)
            frames.append((e['t'], dt, sorted(k for k in keys if k.startswith('Key')), before['pitch'], a, cf, cr, cu, fwd, rgt, upm, math.asin(max(-1, min(1, before['pos'][2]/ln(before['pos']))))))
fin = [e for e in lines if e.get('k') == 'final'][0]
err = max(abs(fin['pos'][i]-p['pos'][i]) for i in range(3))
print(f"replica vs recorded final pose: max position difference {err:.3e} m, pitch {fin['pitch']:.6f} vs {p['pitch']:.6f}")
print(f"frames with movement keys: {len(frames)}; minimum altitude reached: {min_alt:.2f} m; largest |latitude| reached: {max_lat:.1f} deg")
bad = 0
for (t, dt, ks, pitch, a, cf, cr, cu, fwd, rgt, upm, lat) in frames:
    # the displacement along heading/right/up must have the sign of the commanded one when the commanded one is not tiny
    ok = (abs(fwd) < 1e-9 or cf*fwd > 0) and (abs(rgt) < 1e-9 or cr*rgt > 0) and (abs(upm) < 1e-9 or cu*upm > 0)
    if not ok:
        bad += 1
        if bad <= 10:
            print('SIGN MISMATCH', t/1e6, ks, f'pitch {math.degrees(pitch):.1f} alt {a:.1f} lat {math.degrees(lat):.1f}', cf, cr, cu, fwd, rgt, upm)
print('frames whose movement disagrees in sign with the keys:', bad)
# Timeline of key holds
cur = None
spans = []
for (t, dt, ks, pitch, a, cf, cr, cu, fwd, rgt, upm, lat) in frames:
    s = tuple(ks)
    if cur and cur[0] == s and t - cur[2] < 200_000:
        cur[2] = t; cur[3] += cf; cur[4] += cr; cur[5] += cu; cur[6] = a; cur[7] = pitch; cur[8] = lat
    else:
        if cur: spans.append(cur)
        cur = [s, t, t, cf, cr, cu, a, pitch, lat, a, pitch]
if cur: spans.append(cur)
print('\nkey holds (keys, start s, duration s, moved along heading / right / up m, altitude before -> after m, pitch deg, lat deg):')
for s in spans:
    ks, t0, t1, cf, cr, cu, a1, pitch, lat, a0, p0 = s
    print(f"  {'+'.join(k[3:] for k in ks):8s} {t0/1e6:8.1f}s dur {max(t1-t0,0)/1e6:5.2f}  fwd {cf:9.1f} right {cr:9.1f} up {cu:9.1f}  alt {a0:9.1f}->{a1:9.1f}  pitch {math.degrees(p0):6.1f}  lat {math.degrees(lat):6.1f}")
