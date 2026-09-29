"""Re-judge osu!mania stable replays per column.

ScoreV1 model: port of Interlude prelude (MIT) osu!mania ruleset
  YAVSRG/prelude/src/Gameplay/Rulesets/osu!mania.fs, Scoring/HitMechanics.fs (osu_mania),
  Scoring/GameplayEventProcessor.fs, Scoring/OsuHolds.fs
ScoreV2 model: head + tail judged separately, tail windows x1.5 (osu-wiki Gameplay/Judgement/osu!mania#scorev2);
  details not covered by the wiki (overhold, early-release) are configurable guesses.
"""
import lzma, math, os, struct, sys, json, hashlib
from collections import Counter

SONGS = "/mnt/e/Games/osu!/Songs"
REPLAYS = "/mnt/e/Games/osu!/Data/r"
M_EZ, M_HR, M_DT, M_HT, M_NC, M_RD, M_V2, M_MR = 2, 16, 64, 256, 512, 1 << 21, 1 << 29, 1 << 30
J = ['320', '300', '200', '100', '50', 'miss']


# ---------- .osr ----------
def read_osr(path):
    b = open(path, 'rb').read()
    o = 0

    def take(n):
        nonlocal o
        v = b[o:o + n]; o += n; return v

    def s():
        t = take(1)[0]
        if t == 0:
            return None
        n = sh = 0
        while True:
            c = take(1)[0]; n |= (c & 0x7f) << sh; sh += 7
            if not c & 0x80:
                break
        return take(n).decode('utf-8')

    mode = take(1)[0]; ver = struct.unpack('<i', take(4))[0]
    bmd5 = s(); player = s(); rmd5 = s()
    c300, c100, c50, cgeki, ckatu, cmiss = struct.unpack('<6h', take(12))
    score, combo, perfect, mods = struct.unpack('<ihBi', take(11))
    s(); ts = struct.unpack('<q', take(8))[0]
    n = struct.unpack('<i', take(4))[0]
    raw = lzma.decompress(take(n), format=lzma.FORMAT_ALONE).decode('ascii')
    frames, seed = [], None
    t = 0
    for ev in raw.rstrip(',').split(','):
        w, x, y, z = ev.split('|')
        if w == '-12345':
            seed = int(z); continue
        # absolute time accumulates over ALL frames, incl. the (256,-500) lead-in frames lazer later drops
        t += int(w)
        frames.append([t, float(x), float(y), int(z)])
    # lazer LegacyScoreDecoder.readLegacyReplay fix-ups (mirror stable ReplayWatcher)
    if len(frames) >= 2 and frames[1][0] < frames[0][0]:
        frames[1][0] = frames[0][0]; frames[0][0] = 0
    if len(frames) >= 3 and frames[0][0] > frames[2][0]:
        frames[0][0] = frames[1][0] = frames[2][0]
    if len(frames) >= 2 and frames[1][1:3] == [256.0, -500.0]:
        frames.pop(1)
    if len(frames) >= 1 and frames[0][1:3] == [256.0, -500.0]:
        frames.pop(0)
    counts = {'320': cgeki, '300': c300, '200': ckatu, '100': c100, '50': c50, 'miss': cmiss}
    return dict(mode=mode, ver=ver, md5=bmd5, player=player, counts=counts, score=score, combo=combo,
                mods=mods, ts=ts, frames=frames, seed=seed)


# ---------- .osu ----------
def read_osu(path):
    sec, K, od, fmt = None, None, None, None
    objs = []
    for line in open(path, encoding='utf-8-sig', errors='replace'):
        line = line.strip()
        if fmt is None and line.startswith('osu file format v'):
            fmt = int(line.split('v')[-1])
        if line.startswith('['):
            sec = line; continue
        if sec == '[Difficulty]':
            if line.startswith('CircleSize:'):
                K = int(round(float(line.split(':')[1])))
            elif line.startswith('OverallDifficulty:'):
                od = float(line.split(':')[1])
        elif sec == '[HitObjects]' and line:
            p = line.split(',')
            x, t, typ = float(p[0]), int(float(p[2])), int(p[3])
            col = min(max(int(math.floor(x * K / 512)), 0), K - 1)
            end = int(float(p[5].split(':')[0])) if typ & 128 else None
            objs.append((t, col, end))
    return K, od, fmt, objs


# ---------- windows ----------
def _fl(x):
    # 24.9 - 1.1 * 9 evaluates to 14.999999999999998 in binary floating point; stable clearly floors to 15
    return math.floor(x + 1e-6)


def windows(od, mods, v2):
    m = 1 / 1.4 if mods & M_HR else (1.4 if mods & M_EZ else 1.0)
    if v2:
        pw = 22.4 - 0.6 * od if od <= 5 else 24.9 - 1.1 * od
    else:
        pw = 16.0
    raw = [pw, 64 - 3 * od, 97 - 3 * od, 127 - 3 * od, 151 - 3 * od, 188 - 3 * od]
    hv = os.environ.get('HRVAR', 'unscaled:5')
    if m < 1 and not v2 and os.environ.get('HRP', '') == 'v2formula':
        pw = 22.4 - 0.6 * od if od <= 5 else 24.9 - 1.1 * od
        raw = [pw, 64 - 3 * od, 97 - 3 * od, 127 - 3 * od, 151 - 3 * od, 188 - 3 * od]
    if m != 1.0 and hv == 'dblfloor':
        return [_fl(_fl(w) * m) for w in raw], m
    if m != 1.0 and hv == 'odmul':
        od2 = min(10.0, od * 1.4) if m < 1 else od / 2
        raw2 = [pw, 64 - 3 * od2, 97 - 3 * od2, 127 - 3 * od2, 151 - 3 * od2, 188 - 3 * od2]
        return [_fl(w) for w in raw2], 1.0
    if m != 1.0 and hv.startswith('unscaled:'):
        idx = [int(i) for i in hv.split(':')[1].split(',')]
        return [_fl(raw[i]) if i in idx else _fl(raw[i] * m) for i in range(6)], m
    if m != 1.0 and hv == 'missunscaled':
        out = [_fl(w * m) for w in raw]
        out[5] = _fl(raw[5])
        return out, m
    return [_fl(w * m) for w in raw], m


def note_judge(d, W):
    """d = press - note (int ms, map time). Interlude windows: 100/50/miss have late edge O-1."""
    P, G, Gd, O, M, X = W
    a = abs(d)
    if a <= P: return 0
    if a <= G: return 1
    if a <= Gd: return 2
    if -O <= d <= O - 1: return 3
    if -M <= d <= O - 1: return 4
    return 5


# ---------- per-column simulation ----------
H_NOTHING, H_HOLDING, H_DROPPED, H_REGRABBED, H_MH_DROPPED, H_MH_REGRABBED, H_REGRABBED_PENDING = range(7)
DROPPED_STATES = {H_DROPPED, H_REGRABBED, H_MH_DROPPED, H_MH_REGRABBED}
MISSED_HEAD = {H_MH_DROPPED, H_MH_REGRABBED}


def simulate_v1(entries, events, W, od, m, rate, variant=(), end_time=float('inf')):
    """entries: sorted list of dicts {t, kind: note|head|tail, obj}. events: [(t, down:bool)] for this column.
    Returns list of per-object results."""
    P, G, Gd, O, M, X = W
    early = X + 0.5          # early window (note & release)
    late = O - 0.5           # late window (note & release)
    lnw = dict(w320=math.floor(P * 1.2) + 0.5, w300=math.floor(G * 1.1) + 0.5, w200=Gd + 0.5, w100=O + 0.5,
               w50=M + 0.5, oh200=math.floor(43 - od * 3) + 0.5, oh100=math.floor(103 - od * 3) + 0.5)
    if m != 1.0:  # Interlude EZ/HR overhold adjustments
        for k in ('oh200', 'oh100'):
            bw = lnw[k]
            lnw[k] = (math.floor(bw * 1.4) - 0.5) if m > 1 else (math.floor(bw / 1.4) + 1.5)
    early *= rate; late *= rate
    for e in entries:
        e['status'] = 'req'
        e['delta'] = late if e['kind'] != 'tail' else (O + 0.5 - 1) * rate
    results = []
    state, hidx = H_NOTHING, -1
    exp = 0
    down = False

    def ln_judgement(hd, rd, overheld, dropped):
        ra, ha = abs(rd), abs(hd)
        if overheld:
            return 2 if ha < lnw['oh200'] else (3 if ha < lnw['oh100'] else 4)
        if dropped:
            return 5 if rd < -lnw['w50'] else 4
        mean = (ra + ha) * 0.5
        if rd < -lnw['w50']: return 5
        if ha < lnw['w320'] and mean < lnw['w320']: return 0
        if ha < lnw['w300'] and mean < lnw['w300']: return 1
        if ha < lnw['w200'] and mean < lnw['w200']: return 2
        if ha < lnw['w100'] and mean < lnw['w100']: return 3
        return 4

    iv = []  # key-down intervals of this column
    st = None
    for t_, dn in events:
        if dn: st = t_
        elif st is not None: iv.append((st, t_)); st = None
    if st is not None: iv.append((st, float('inf')))

    def held_in(a, b):
        return any(x < b and y > a for x, y in iv)

    def release(tail, missed, overheld, dropped, head, missed_head, rd):
        if missed and missed_head and not overheld:
            j = 5
        elif 'wiki_tail_miss' in variant and missed and not overheld and not held_in(tail['t'] - M, tail['t'] + O):
            j = 5
        elif 'kill_miss' in variant and killing:
            j = 5
        else:
            j = ln_judgement(head['delta'] / rate, rd / rate, overheld, dropped)
            gaps = head.get('gaps', [])
            rc = next((int(v.split('=')[1]) for v in variant if v.startswith('rehitcap=')), None)
            if rc is not None and head.get('rehit') and not missed_head:
                j = max(j, rc)
            if dropped and not missed_head and not overheld and not killing and j != 5 and gaps:
                pre = gaps[0][0] is not None and gaps[0][0] < head['t']
                rule = next((v.split('=')[1] for v in variant if v.startswith(('pre=' if pre else 'body='))), 'cap50')
                full = ln_judgement(head['delta'] / rate, rd / rate, False, False)
                gm = next((int(v.split('=')[1]) for v in variant if v.startswith('gapmin=')), None)
                if gm is not None and not pre and all(b is not None and b - a < gm for a, b in gaps):
                    rule = 'full'
                if rule == 'cap200': j = max(full, 2)
                elif rule == 'cap100': j = max(full, 3)
                elif rule == 'cap300': j = max(full, 1)
                elif rule == 'full': j = full
                elif rule == 'headonly': j = ln_judgement(head['delta'] / rate, rd / rate, True, False)
                elif rule == 'rehead': # re-press becomes the head
                    regrab = gaps[0][1] if gaps[0][1] is not None else head["t"] + head["delta"]
                    j = ln_judgement((regrab - head['t']) / rate, rd / rate, False, False)
                elif rule == 'rehead200':
                    regrab = gaps[0][1] if gaps[0][1] is not None else head["t"] + head["delta"]
                    j = max(2, ln_judgement((regrab - head['t']) / rate, rd / rate, False, False))
        results.append(dict(gaps=head.get('gaps', []), killed=killing, t=head['t'], end=tail['t'], kind='ln', j=j, head_delta=None if missed_head else head['delta'] / rate,
                            rel_delta=None if (missed and not overheld) else rd / rate, overhold=overheld, dropped=dropped,
                            missed_head=missed_head))

    def expire(now):
        nonlocal exp, state, hidx
        while exp < len(entries) and entries[exp]['t'] < now - late:
            e = entries[exp]
            if e['status'] == 'req':
                if e['kind'] == 'note':
                    results.append(dict(t=e['t'], kind='note', j=5, delta=None))
                    e['status'] = 'acc'
                elif e['kind'] == 'head':
                    state, hidx = H_MH_DROPPED, exp
                    e['status'] = 'acc'
                else:
                    overhold = state in (H_REGRABBED, H_HOLDING, H_MH_REGRABBED) and down
                    release(e, True, overhold, state in DROPPED_STATES, entries[hidx], state in MISSED_HEAD, e['delta'])
                    e['status'] = 'acc'
                    state = H_NOTHING
            exp += 1

    killing = False

    def kill_hold(now):
        nonlocal state, killing
        if state == H_NOTHING:
            return
        for i in range(hidx, len(entries)):
            e = entries[i]
            if e['kind'] == 'tail' and e['status'] == 'req':
                e['status'] = 'acc'
                killing = True
                release(e, True, False, True, entries[hidx], state in MISSED_HEAD, late)
                killing = False
                break
        state = H_NOTHING

    for now, is_down in events:
        expire(now)
        if is_down:
            down = True
            si = exp
            while si < len(entries) and entries[si]['t'] < now - late:
                si += 1
            cand, blocked = -1, False
            i = si
            while i < len(entries) and entries[i]['t'] <= now + early:
                e = entries[i]
                d = now - e['t']
                if e['status'] == 'acc' and e['kind'] != 'tail' and d < 0:
                    blocked = True; break
                if e['status'] == 'req' and e['kind'] in ('note', 'head'):
                    cand = i; break
                i += 1
            if blocked:
                continue
            if cand >= 0:
                kill_hold(now)
                e = entries[cand]
                e['status'] = 'acc'
                e['delta'] = now - e['t']
                if e['kind'] == 'head':
                    state, hidx = H_HOLDING, cand
                else:
                    results.append(dict(t=e['t'], kind='note', j=note_judge(e['delta'] / rate, W), delta=e['delta'] / rate))
            else:
                if state in (H_MH_DROPPED, H_DROPPED):
                    entries[hidx].setdefault('gaps', []).append([None, now]) if not entries[hidx].get('gaps') or entries[hidx]['gaps'][-1][1] is not None else entries[hidx]['gaps'][-1].__setitem__(1, now)
                if state == H_MH_DROPPED: state = H_MH_REGRABBED
                elif state == H_DROPPED: state = H_REGRABBED
                elif state == H_REGRABBED_PENDING: state = H_HOLDING
        else:
            down = False
            if 'pre_release_unhit' in variant and state == H_HOLDING and now < entries[hidx]['t']:
                entries[hidx]['rehit'] = entries[hidx].get('rehit', 0) + 1
                entries[hidx]['status'] = 'req'
                entries[hidx]['delta'] = late
                state, hidx = H_NOTHING, -1
                continue
            if 'pre_release_ignore' in variant and state == H_HOLDING and now < entries[hidx]['t']:
                state = H_REGRABBED_PENDING
                continue
            if state in (H_HOLDING, H_REGRABBED, H_MH_REGRABBED):
                found = -1
                for i in range(hidx, len(entries)):
                    e = entries[i]
                    if e['t'] > now + early:
                        break
                    if e['kind'] == 'tail' and e['status'] == 'req':
                        found = i; break
                if found >= 0 and now - entries[found]['t'] >= -early:
                    e = entries[found]
                    e['status'] = 'acc'
                    d = now - e['t']
                    overhold = d > late
                    if not overhold:
                        e['delta'] = d
                    release(e, overhold, overhold, state in DROPPED_STATES, entries[hidx], state in MISSED_HEAD, e['delta'])
                    state = H_NOTHING
                else:
                    if state in (H_HOLDING, H_REGRABBED, H_MH_REGRABBED):
                        entries[hidx].setdefault('gaps', []).append([now, None])
                    if state in (H_HOLDING, H_REGRABBED): state = H_DROPPED
                    elif state == H_MH_REGRABBED: state = H_MH_DROPPED
    expire(end_time)
    return results


def simulate_v2(entries, events, W, rate, overhold_j=5, tail_mult=1.5):
    """ScoreV2: heads and tails judged separately; tail judged on |release|/1.5 capped at 50 if head missed or body released."""
    P, G, Gd, O, M, X = W
    early, late = (X + 0.5) * rate, (O - 0.5) * rate
    t_early, t_late = (X + 0.5) * tail_mult * rate, (O - 0.5) * tail_mult * rate
    res = []
    for e in entries:
        e['status'] = 'req'
    exp = 0
    active = None  # [head_entry, tail_entry, broken, head_missed]
    down = False

    def tail_result(tail, j, d, info):
        res.append(dict(t=tail['t'], kind='tail', j=j, delta=d, **info))

    def expire(now):
        nonlocal exp, active
        while exp < len(entries):
            e = entries[exp]
            lim = t_late if e['kind'] == 'tail' else late
            if not e['t'] < now - lim:
                break
            if e['status'] == 'req':
                e['status'] = 'acc'
                if e['kind'] == 'note':
                    res.append(dict(t=e['t'], kind='note', j=5, delta=None))
                elif e['kind'] == 'head':
                    res.append(dict(t=e['t'], kind='head', j=5, delta=None))
                    active = [e, e['tail'], True, True]
                else:
                    held = down and active is not None and active[1] is e
                    tail_result(e, overhold_j if held else 5, None, dict(overhold=held))
                    if active is not None and active[1] is e:
                        active = None
            exp += 1
        # tails can expire out of order w.r.t. later notes because of the x1.5 window
        for e in entries[exp:]:
            if e['kind'] == 'tail' and e['status'] == 'req' and e['t'] < now - t_late:
                e['status'] = 'acc'
                held = down and active is not None and active[1] is e
                tail_result(e, overhold_j if held else 5, None, dict(overhold=held))
                if active is not None and active[1] is e:
                    active = None
            if e['t'] > now + early:
                break

    for now, is_down in events:
        expire(now)
        if is_down:
            down = True
            cand, blocked = -1, False
            for i in range(exp, len(entries)):
                e = entries[i]
                if e['t'] > now + early:
                    break
                if e['kind'] == 'tail' or e['t'] < now - late:
                    continue
                if e['status'] == 'acc' and now - e['t'] < 0:
                    blocked = True; break
                if e['status'] == 'req':
                    cand = i; break
            if blocked:
                continue
            if cand >= 0:
                e = entries[cand]
                e['status'] = 'acc'
                d = (now - e['t']) / rate
                j = note_judge(d, W)
                res.append(dict(t=e['t'], kind=e['kind'], j=j, delta=d))
                if e['kind'] == 'head':
                    active = [e, e['tail'], False, j == 5]
            elif active is not None:
                pass  # regrab: keep broken flag
        else:
            down = False
            if active is not None:
                tail = active[1]
                if tail['status'] == 'req':
                    d = now - tail['t']
                    if d >= -t_early and d < -(M + 0.5) * tail_mult * rate:
                        active[2] = True  # released during body, before the tail's 50 window
                    elif d >= -(M + 0.5) * tail_mult * rate:
                        tail['status'] = 'acc'
                        dd = d / rate / tail_mult
                        j = note_judge(round(dd), W) if d <= t_late else 5
                        if (active[2] or active[3]) and j < 4:
                            j = 4
                        tail_result(tail, j, d / rate, dict(broken=active[2], head_missed=active[3]))
                        active = None
                    else:
                        active[2] = True
    expire(float('inf'))
    return res


def build_entries(objs, K, mirror):
    cols = [[] for _ in range(K)]
    for t, c, end in objs:
        if mirror:
            c = K - 1 - c
        if end is None:
            cols[c].append(dict(t=t, kind='note'))
        else:
            h = dict(t=t, kind='head')
            tl = dict(t=end, kind='tail')
            h['tail'] = tl
            cols[c].append(h); cols[c].append(tl)
    for c in cols:
        c.sort(key=lambda e: (e['t'], 0 if e['kind'] == 'tail' else 1))
    return cols


def column_events(frames, K):
    ev = [[] for _ in range(K)]
    prev = 0
    last_t = -10 ** 9
    for t, x, y, z in frames:
        if t < last_t:  # lazer: never allow backwards time traversal
            continue
        last_t = t
        st = int(x) & ((1 << K) - 1)
        ch = st ^ prev
        for k in range(K):
            if ch >> k & 1:
                ev[k].append((t, bool(st >> k & 1)))
        prev = st
    return ev


def rejudge(osr_path, osu_path, variant=(), **kw):
    r = read_osr(osr_path)
    K, od, fmt, objs = read_osu(osu_path)
    mods = r['mods']
    if mods & M_RD:
        raise ValueError('Random mod: column shuffle algorithm of stable is not public; cannot attribute notes')
    rate = 1.5 if mods & M_DT else (0.75 if mods & M_HT else 1.0)
    v2 = bool(mods & M_V2)
    W, m = windows(od, mods, v2)
    if rate != 1.0 and os.environ.get('RATEVAR', 'mapfloor') == 'mapfloor':
        Wr, _ = windows(od, mods, v2)
        raw = [W_ for W_ in Wr]
        pw = 16.0 if not v2 else (22.4 - 0.6 * od if od <= 5 else 24.9 - 1.1 * od)
        base = [pw, 64 - 3 * od, 97 - 3 * od, 127 - 3 * od, 151 - 3 * od, 188 - 3 * od]
        W = [_fl(b * (1.0 if (i == 5 and m < 1) else m) * rate) for i, b in enumerate(base)]
        rate = 1.0
    if rate != 1.0 and os.environ.get('RATEVAR') == 'none':
        rate = 1.0
    cols = build_entries(objs, K, bool(mods & M_MR))
    evs = column_events(r['frames'], K)
    allres = []
    for k in range(K):
        res = (simulate_v2b(cols[k], evs[k], W, rate, kw['v2opt']) if 'v2opt' in kw else simulate_v2(cols[k], evs[k], W, rate)) if v2 else simulate_v1(cols[k], evs[k], W, od, m, rate, variant, r['frames'][-1][0] if r['frames'] else 0)
        for x in res:
            x['col'] = k
        allres += res
    cnt = Counter(J[x['j']] for x in allres)
    return r, dict(K=K, od=od, fmt=fmt, v2=v2, rate=rate, W=W, n_obj=len(objs),
                   n_ln=sum(1 for o in objs if o[2] is not None)), allres, {j: cnt.get(j, 0) for j in J}


def md5file(p):
    return hashlib.md5(open(p, 'rb').read()).hexdigest()


if __name__ == '__main__':
    info = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'replays7k.json')))
    sel = sys.argv[1:] or [x['fn'] for x in info[:5]]
    byfn = {x['fn']: x for x in info}
    for fn in sel:
        meta = byfn[fn]
        osu_path = os.path.join(SONGS, meta['folder'].replace('\\', '/'), meta['file'])
        ok_md5 = md5file(osu_path) == meta['md5']
        r, mi, res, cnt = rejudge(os.path.join(REPLAYS, fn), osu_path)
        hdr = r['counts']
        diff = {j: cnt[j] - hdr[j] for j in J}
        exact = all(v == 0 for v in diff.values())
        print(f"\n{meta['title'][:80]}\n  file md5 ok={ok_md5} K={mi['K']} OD={mi['od']} fmt=v{mi['fmt']} objs={mi['n_obj']} LN={mi['n_ln']} "
              f"mods={r['mods']} v2={mi['v2']} W={mi['W']}")
        print('  header  ', hdr)
        print('  rejudged', cnt)
        print('  diff    ', diff, 'EXACT' if exact else '')


def simulate_v2b(entries, events, W, rate, opt):
    """Configurable ScoreV2 model. opt keys: lock ('earliest'|'lazer'), tail_late ('O'|'M'), overhold (5|4),
    tailwin ('div'|'floor'), early_rel ('miss'|'drop'), cap (4 = MEH cap after break / missed head)."""
    P, G, Gd, O, M, X = W
    TM = 1.5
    objs = [e for e in entries if e['kind'] != 'tail']
    for e in entries:
        e['status'] = 'req'
    res = []
    down = False
    active = None  # dict(head, tail, broken, head_missed)

    def tj(d):  # tail judgement for release offset d (map ms), None if outside any window
        a = abs(d)
        if opt['tailwin'] == 'div':
            a = a / TM
            wins = W
        else:
            wins = [math.floor(w * TM) for w in W]
        late_lim = wins[3] - 1 if opt['tail_late'] == 'O' else wins[4]
        if d > late_lim and opt['tailwin'] == 'floor':
            return None
        if opt['tailwin'] == 'div' and d / TM > (W[3] - 1 if opt['tail_late'] == 'O' else W[4]):
            return None
        for i in range(5):
            if a <= wins[i]:
                return i
        if a <= wins[5]:
            return 5
        return None

    def late_lim_tail():
        if opt['tailwin'] == 'div':
            return (W[3] - 1 if opt['tail_late'] == 'O' else W[4]) * TM
        wins = [math.floor(w * TM) for w in W]
        return wins[3] - 1 if opt['tail_late'] == 'O' else wins[4]

    def miss_obj(o):
        o['status'] = 'acc'
        res.append(dict(t=o['t'], kind=o['kind'], j=5, delta=None))
        if o['kind'] == 'head':
            return dict(head=o, tail=o['tail'], broken=True, head_missed=True)
        return None

    def judge_tail(a, j, d, overhold=False):
        a['tail']['status'] = 'acc'
        if j != 5 and (a['broken'] or a['head_missed']):
            j = max(j, opt.get('cap', 4))
        res.append(dict(t=a['tail']['t'], kind='tail', j=j, delta=d, overhold=overhold, broken=a['broken'], head_missed=a['head_missed']))

    pending_tails = []  # actives whose tail is unjudged

    def expire(now):
        nonlocal active
        for o in objs:
            if o['status'] == 'req' and o['t'] < now - (O - 0.5) * rate:
                a = miss_obj(o)
                if a:
                    pending_tails.append(a)
            elif o['t'] > now:
                break
        for a in list(pending_tails):
            if a['tail']['status'] == 'req' and a['tail']['t'] < now - late_lim_tail() * rate - 0.5:
                held = down and active is a
                judge_tail(a, opt['overhold'] if held else 5, None, overhold=held)
                pending_tails.remove(a)
                if active is a:
                    active = None
            elif a['tail']['status'] != 'req':
                pending_tails.remove(a)

    for now, is_down in events:
        expire(now)
        if is_down:
            down = True
            cand = None
            for idx, o in enumerate(objs):
                if o['t'] > now + (X + 0.5) * rate:
                    break
                if o['status'] != 'req':
                    continue
                if opt['lock'] == 'lazer':
                    nxt = objs[idx + 1] if idx + 1 < len(objs) else None
                    if nxt is not None and now >= nxt['t']:
                        continue
                cand = o
                break
            if cand is not None:
                if opt['lock'] == 'lazer':
                    for o in objs:
                        if o is cand:
                            break
                        if o['status'] == 'req':
                            a = miss_obj(o)
                            if a:
                                pending_tails.append(a)
                d = (now - cand['t']) / rate
                j = note_judge(d, W)
                cand['status'] = 'acc'
                res.append(dict(t=cand['t'], kind=cand['kind'], j=j, delta=d))
                if cand['kind'] == 'head':
                    a = dict(head=cand, tail=cand['tail'], broken=False, head_missed=(j == 5))
                    pending_tails.append(a)
                    active = a
            else:
                # regrab of a broken hold whose tail is still pending
                for a in pending_tails:
                    if a['tail']['status'] == 'req' and a['head']['t'] <= now:
                        active = a
                        break
        else:
            down = False
            if active is not None and active['tail']['status'] == 'req':
                a = active
                d = (now - a['tail']['t']) / rate
                j = tj(d)
                if j is None or (j == 5 and opt['early_rel'] == 'drop'):
                    a['broken'] = True
                else:
                    judge_tail(a, j, d)
                    if a in pending_tails:
                        pending_tails.remove(a)
            active = None
    expire(float('inf'))
    return res
