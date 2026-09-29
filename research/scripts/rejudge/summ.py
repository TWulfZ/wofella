import json, sys
from collections import Counter
d = json.load(open(sys.argv[1]))
ok = [x for x in d if 'error' not in x]
def grp(f, name):
    s = [x for x in ok if f(x)]
    ex = sum(1 for x in s if x['absdiff'] == 0)
    tot = sum(x['n_hdr'] for x in s); ad = sum(x['absdiff'] for x in s)
    print(f'{name:34s} n={len(s):4d} exact={ex:4d} ({(ex/len(s)*100 if s else 0):5.1f}%) sum|diff|={ad:5d}/{tot} ({(ad/tot*100 if tot else 0):.3f}%)')
comp = lambda x: x['n_hdr'] in (x['n_obj'], x['n_obj'] + x['n_ln'])
grp(lambda x: not x['v2'] and x['rate'] == 1, 'V1 1.0x all')
grp(lambda x: not x['v2'] and x['rate'] == 1 and x['n_ln'] == 0, 'V1 1.0x no LN')
grp(lambda x: not x['v2'] and x['rate'] == 1 and 0 < x['n_ln'] / x['n_obj'] <= 0.1, 'V1 1.0x LN<=10%')
grp(lambda x: not x['v2'] and x['rate'] == 1 and x['n_ln'] / x['n_obj'] > 0.1, 'V1 1.0x LN>10%')
grp(lambda x: not x['v2'] and x['rate'] != 1, 'V1 DT/HT')
grp(lambda x: x['v2'], 'V2 all')
grp(lambda x: x['v2'] and x['n_ln'] / x['n_obj'] <= 0.1, 'V2 LN<=10%')
agg = Counter()
for x in ok:
    if not x['v2'] and x['rate'] == 1 and x['n_ln']:
        for j in x['got']: agg[j] += x['got'][j] - x['hdr'][j]
print('V1 1.0x LN maps net diff (got-hdr):', dict(agg))
agg = Counter()
for x in ok:
    if x['v2']:
        for j in x['got']: agg[j] += x['got'][j] - x['hdr'][j]
print('V2 net diff (got-hdr):', dict(agg))
