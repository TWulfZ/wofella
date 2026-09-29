import json, os, sys, itertools
sys.path.insert(0, '.')
from rejudge import *
d = json.load(open('batch_400_base.json')); maps = json.load(open('maps.json'))
sel = [x for x in d if 'error' not in x and x['v2'] and x['n_hdr'] == x['n_obj'] + x['n_ln'] and x['rate'] == 1]
print('V2 complete 1.0x replays:', len(sel), 'LN-free:', sum(1 for x in sel if x['n_ln'] == 0))
cache = {}
def run(opt, subset):
    ex = ad = 0; net = {j: 0 for j in J}
    for x in subset:
        m = maps[x['fn'].split('-')[0]]
        p = os.path.join(SONGS, m['folder'].replace('\\', '/'), m['file'])
        r, mi, res, cnt = rejudge(os.path.join(REPLAYS, x['fn']), p, v2opt=opt)
        df = {j: cnt[j] - x['hdr'][j] for j in J}
        ex += all(v == 0 for v in df.values()); ad += sum(abs(v) for v in df.values())
        for j in J: net[j] += df[j]
    return ex, ad, net
base = dict(lock='earliest', tail_late='O', overhold=5, tailwin='div', early_rel='miss', cap=4)
grid = dict(lock=['earliest', 'lazer'], tail_late=['O', 'M'], overhold=[5, 4], tailwin=['div', 'floor'], early_rel=['miss', 'drop'])
sub = sel[:int(sys.argv[1])] if len(sys.argv) > 1 else sel
for k, vals in grid.items():
    for v in vals:
        o = dict(base); o[k] = v
        ex, ad, net = run(o, sub)
        print(f'{k}={v!s:9s} exact={ex}/{len(sub)} sum|diff|={ad} net={net}')
