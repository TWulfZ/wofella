import json, os, sys
sys.path.insert(0, '.')
from rejudge import *
import numpy as np
from collections import Counter
d = json.load(open('batch_400_base.json')); maps = json.load(open('maps.json'))
VAR = tuple(sys.argv[1].split(','))
feats = ['drop_regrab', 'drop_prehead', 'drop_killed', 'oh200', 'oh100', 'oh50', 'norm50', 'norm100', 'norm200', 'mh50', 'mhmiss', 'normmiss', 'dropmiss', 'killmiss']
X = []; Y = []; ex = 0
for x in d:
    if 'error' in x or x['v2'] or x['rate'] != 1 or x['n_ln'] == 0 or x['n_hdr'] != x['n_obj']: continue
    m = maps[x['fn'].split('-')[0]]
    p = os.path.join(SONGS, m['folder'].replace('\\', '/'), m['file'])
    r, mi, res, cnt = rejudge(os.path.join(REPLAYS, x['fn']), p, variant=VAR)
    c = Counter()
    for y in res:
        if y['kind'] != 'ln': continue
        j = J[y['j']]
        if y['overhold']: c['oh' + j] += 1
        elif y['missed_head']: c['mh' + j] += 1
        elif y['dropped']:
            if y['killed']: c['drop_killed' if j != 'miss' else 'killmiss'] += 1
            elif j == 'miss': c['dropmiss'] += 1
            elif y['gaps'] and y['gaps'][0][0] is not None and y['gaps'][0][0] < y['t']: c['drop_prehead'] += 1
            else: c['drop_regrab'] += 1
        else: c['norm' + j] += 1
    X.append([c[f] for f in feats]); Y.append([cnt[j] - x['hdr'][j] for j in J])
    ex += all(cnt[j] == x['hdr'][j] for j in J)
X = np.array(X, float); Y = np.array(Y, float)
print('replays', len(X), 'exact', ex, 'sum|diff|', int(np.abs(Y).sum()), 'net', dict(zip(J, Y.sum(0).astype(int))))
print('feature totals', {f: int(v) for f, v in zip(feats, X.sum(0))})
coef, *_ = np.linalg.lstsq(X, Y, rcond=None)
print(f'{"":14s}', ' '.join(f'{j:>5s}' for j in J))
for f, row in zip(feats, coef): print(f'{f:14s}', ' '.join(f'{v:+.2f}' for v in row))
