import json, os, sys
sys.path.insert(0, '.')
from rejudge import *
from collections import Counter
d = json.load(open('batch_400_base.json')); maps = json.load(open('maps.json'))
sel = [x for x in d if 'error' not in x and x['v2'] and x['n_hdr'] == x['n_obj'] + x['n_ln'] and x['rate'] == 1][:40]
opt = dict(lock='earliest', tail_late='M', overhold=5, tailwin='div', early_rel='drop', cap=4)
data = []
for x in sel:
    m = maps[x['fn'].split('-')[0]]
    p = os.path.join(SONGS, m['folder'].replace('\\', '/'), m['file'])
    r, mi, res, cnt = rejudge(os.path.join(REPLAYS, x['fn']), p, v2opt=opt)
    base = Counter(J[y['j']] for y in res if y['kind'] != 'tail')
    tails = [y for y in res if y['kind'] == 'tail']
    data.append((x, mi['W'], base, tails))
def score(k, cap_on=True):
    ad = 0; ex = 0; net = Counter()
    for x, W, base, tails in data:
        c = Counter(base)
        for y in tails:
            if y['delta'] is None:
                c['miss'] += 1; continue
            a = abs(y['delta']) / k
            j = next((i for i in range(6) if a <= W[i]), 5)
            if cap_on and (y['broken'] or y['head_missed']) and j < 4: j = 4
            c[J[j]] += 1
        df = {j: c[j] - x['hdr'][j] for j in J}
        ad += sum(abs(v) for v in df.values()); ex += all(v == 0 for v in df.values()); net.update(df)
    return ad, ex, dict(net)
for k in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0]:
    print(k, score(k))
print('--- variants at k=1.5')
def score2(broken_rule, hm_rule, late_rule, k=1.5):
    ad = 0; ex = 0; net = Counter(); cat = Counter()
    for x, W, base, tails in data:
        c = Counter(base)
        for y in tails:
            if y['delta'] is None:
                c['miss'] += 1; continue
            d = y['delta'] / k
            a = abs(d)
            j = next((i for i in range(6) if a <= W[i]), 5)
            if late_rule == 'O' and d > W[3] - 1: j = 5
            if late_rule == 'M' and d > W[4]: j = 5
            if y['broken'] and not y['head_missed']:
                j = 5 if broken_rule == 'miss' else (max(j, 4) if broken_rule == 'cap50' else j)
            if y['head_missed']:
                j = 5 if hm_rule == 'miss' else (max(j, 4) if hm_rule == 'cap50' else j)
            c[J[j]] += 1
        df = {j: c[j] - x['hdr'][j] for j in J}
        ad += sum(abs(v) for v in df.values()); ex += all(v == 0 for v in df.values()); net.update(df)
    return ad, ex, dict(net)
for br in ['cap50', 'miss', 'none']:
    for hm in ['cap50', 'miss', 'none']:
        for lr in ['none', 'O', 'M']:
            print(br, hm, lr, score2(br, hm, lr))
n_b = sum(1 for _, _, _, t in data for y in t if y.get('broken') and not y.get('head_missed'))
n_h = sum(1 for _, _, _, t in data for y in t if y.get('head_missed'))
n_n = sum(1 for _, _, _, t in data for y in t if y['delta'] is None)
print('tails broken', n_b, 'head-missed', n_h, 'no-release(miss)', n_n)
print('--- asymmetric k (early,late), rule cap50/cap50')
def score3(ke, kl, late_cut):
    ad = 0; ex = 0; net = Counter()
    for x, W, base, tails in data:
        c = Counter(base)
        for y in tails:
            if y['delta'] is None:
                c['miss'] += 1; continue
            d = y['delta']
            dd = d / (ke if d < 0 else kl)
            a = abs(dd)
            j = next((i for i in range(6) if a <= W[i]), 5)
            if late_cut and dd > W[3] - 1: j = 5
            if (y['broken'] or y['head_missed']) and j < 4: j = 4
            c[J[j]] += 1
        df = {j: c[j] - x['hdr'][j] for j in J}
        ad += sum(abs(v) for v in df.values()); ex += all(v == 0 for v in df.values()); net.update(df)
    return ad, ex, dict(net)
for ke in [1.4, 1.5, 1.6]:
    for kl in [1.5, 1.75, 2.0, 2.5]:
        for lc in [True, False]:
            print(ke, kl, lc, score3(ke, kl, lc))
print('--- tail window rounding variants (k=1.5, late cut at 100 edge)')
def score4(fn):
    ad = 0; ex = 0; net = Counter()
    for x, W, base, tails in data:
        c = Counter(base)
        TW = fn(W)
        for y in tails:
            if y['delta'] is None:
                c['miss'] += 1; continue
            d = y['delta']; a = abs(d)
            j = next((i for i in range(6) if a <= TW[i]), 5)
            if d > TW[3]: j = 5
            if (y['broken'] or y['head_missed']) and j < 4: j = 4
            c[J[j]] += 1
        df = {j: c[j] - x['hdr'][j] for j in J}
        ad += sum(abs(v) for v in df.values()); ex += all(v == 0 for v in df.values()); net.update(df)
    return ad, ex, dict(net)
import math
print('floor(W*1.5)', score4(lambda W: [math.floor(w * 1.5) for w in W]))
print('(W+0.5)*1.5', score4(lambda W: [(w + 0.5) * 1.5 for w in W]))
print('W*1.5 (late<=)', score4(lambda W: [w * 1.5 for w in W]))
print('floor((W+.5)*1.5)', score4(lambda W: [math.floor((w + 0.5) * 1.5) for w in W]))
