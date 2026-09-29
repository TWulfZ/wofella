"""Batch re-judge the most recent N 7K replays and compare with header judgement counts."""
import json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rejudge import *

maps = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'maps.json')))
N = int(sys.argv[1]) if len(sys.argv) > 1 else 200
VAR = tuple(sys.argv[2].split(',')) if len(sys.argv) > 2 and sys.argv[2] else ()
ONLY = sys.argv[3] if len(sys.argv) > 3 else ''
rows = []
for fn in os.listdir(REPLAYS):
    if fn.endswith('.osr'):
        md5, ticks = fn[:-4].split('-')
        m = maps.get(md5)
        if m and m['mode'] == 3 and round(m['cs']) == 7:
            rows.append((int(ticks), fn, m))
rows.sort(reverse=True)
out = []
t0 = time.time()
for ticks, fn, m in rows[:N]:
    if ONLY == 'v1ln' and not (0 == 0):
        pass
    osu_path = os.path.join(SONGS, m['folder'].replace('\\', '/'), m['file'])
    rec = dict(fn=fn, title=f"{m['artist']} - {m['title']} [{m['diff']}]")
    try:
        kw = dict(v2opt=dict(lock='earliest', tail_late='O', overhold=5, tailwin='div', early_rel='drop', cap=4)) if 'v2best' in VAR else {}
        r, mi, res, cnt = rejudge(os.path.join(REPLAYS, fn), osu_path, variant=VAR, **kw)
        hdr = r['counts']
        rec.update(ts=r['ts'], frames=len(r['frames']), mods=r['mods'], v2=mi['v2'], rate=mi['rate'], od=mi['od'], n_obj=mi['n_obj'], n_ln=mi['n_ln'],
                   player=r['player'], hdr=hdr, got=cnt, absdiff=sum(abs(cnt[j] - hdr[j]) for j in J),
                   n_hdr=sum(hdr.values()))
    except Exception as e:
        rec['error'] = f'{type(e).__name__}: {e}'
    out.append(rec)
json.dump(out, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), f'batch_{N}_{"-".join(VAR) or "base"}.json'), 'w'), indent=1)
print(f'{len(out)} replays in {time.time() - t0:.1f}s', file=sys.stderr)
