import json, sys
d = json.load(open(sys.argv[1]))
ok = [x for x in d if 'error' not in x]
err = [x for x in d if 'error' in x]
comp = lambda x: x['n_hdr'] in (x['n_obj'] + (x['n_ln'] if x['v2'] else 0),)
print(f'replays={len(d)} errors={len(err)} incomplete(header total != objects)={sum(1 for x in ok if not comp(x))}')
def grp(f, name):
    s = [x for x in ok if comp(x) and f(x)]
    if not s: print(f'{name:28s} n=0'); return
    ex = sum(1 for x in s if x['absdiff'] == 0)
    tot = sum(x['n_hdr'] for x in s); ad = sum(x['absdiff'] for x in s)
    within = sum(1 for x in s if x['absdiff'] <= 2)
    print(f'{name:28s} n={len(s):4d} exact={ex/len(s)*100:5.1f}%  <=1 misjudged={within/len(s)*100:5.1f}%  misjudged~{ad/2/tot*100:.3f}% of {tot} judgements')
lnr = lambda x: x['n_ln'] / x['n_obj']
grp(lambda x: not x['v2'] and x['rate'] == 1 and x['n_ln'] == 0, 'V1 1.0x rice (0 LN)')
grp(lambda x: not x['v2'] and x['rate'] == 1 and 0 < lnr(x) <= 0.1, 'V1 1.0x LN<=10%')
grp(lambda x: not x['v2'] and x['rate'] == 1 and 0.1 < lnr(x) <= 0.4, 'V1 1.0x LN 10-40%')
grp(lambda x: not x['v2'] and x['rate'] == 1 and lnr(x) > 0.4, 'V1 1.0x LN>40%')
grp(lambda x: not x['v2'] and x['rate'] != 1 and x['n_ln'] == 0, 'V1 DT/HT rice')
grp(lambda x: not x['v2'] and x['rate'] != 1 and x['n_ln'] > 0, 'V1 DT/HT with LN')
grp(lambda x: x['v2'] and x['n_ln'] == 0, 'V2 rice')
grp(lambda x: x['v2'] and 0 < lnr(x) <= 0.4, 'V2 LN<=40%')
grp(lambda x: x['v2'] and lnr(x) > 0.4, 'V2 LN>40%')
