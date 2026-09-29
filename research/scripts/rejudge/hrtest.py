import json, os, sys
sys.path.insert(0, '.')
from rejudge import *
d = json.load(open('batch_5000_base.json')); maps = json.load(open('maps.json'))
sel = [x for x in d if 'error' not in x and x['mods'] in tuple(int(v) for v in os.environ.get('HRMODS','16').split(',')) and x['n_ln'] == 0 and x['n_hdr'] == x['n_obj']][:40]
ex = ad = 0; net = {j: 0 for j in J}
for x in sel:
    m = maps[x['fn'].split('-')[0]]
    p = os.path.join(SONGS, m['folder'].replace('\\', '/'), m['file'])
    r, mi, res, cnt = rejudge(os.path.join(REPLAYS, x['fn']), p)
    df = {j: cnt[j] - x['hdr'][j] for j in J}
    ex += all(v == 0 for v in df.values()); ad += sum(abs(v) for v in df.values())
    for j in J: net[j] += df[j]
print(os.environ.get('HRVAR'), 'n', len(sel), 'exact', ex, 'sum|diff|', ad, net)
