import json, os, sys
sys.path.insert(0, '.')
from rejudge import *
from collections import Counter
d = json.load(open(sys.argv[1]))
maps = json.load(open('maps.json'))
VAR = tuple(sys.argv[2].split(',')) if len(sys.argv) > 2 and sys.argv[2] else ()
tot = Counter(); rows = []
for x in d:
    if 'error' in x or x['v2'] or x['rate'] != 1 or x['n_ln'] / x['n_obj'] <= 0.1: continue
    m = maps[x['fn'].split('-')[0]]
    p = os.path.join(SONGS, m['folder'].replace('\\', '/'), m['file'])
    r, mi, res, cnt = rejudge(os.path.join(REPLAYS, x['fn']), p, variant=VAR)
    c = Counter()
    for y in res:
        if y['kind'] != 'ln': continue
        cat = 'overhold' if y['overhold'] else ('mh' if y['missed_head'] else ('drop' if y['dropped'] else 'normal'))
        c[(cat, J[y['j']])] += 1
    diff = {j: cnt[j] - x['hdr'][j] for j in J}
    rows.append((c, diff)); tot.update(c)
    print(x['title'][:40], 'diff', diff, 'drop50', c[('drop', '50')], 'dropmiss', c[('drop', 'miss')], 'oh', sum(v for k, v in c.items() if k[0] == 'overhold'), 'norm50', c[('normal', '50')])
print(sorted(tot.items()))
