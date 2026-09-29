import osudb, json, csv, collections, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
I = {}
for l in open(os.path.join(HERE, 'index7k.jsonl')):
    r = json.loads(l); I[r['md5']] = r
L = collections.defaultdict(list)
for r in csv.DictReader(open(os.path.join(HERE, 'labels.csv'))): L[r['md5']].append(r)
ver, nb, S = osudb.read_scores_db('/mnt/e/Games/osu!/scores.db')
SELF = {'TWulfZ'}
ALIAS = {'', 'W', 'w', 'Wulf', 's', 'TWulfZasdasdasd d jSS||'}
out = []
for s in S:
    if s['mode'] != 3 or s['beatmap_md5'] not in I: continue
    c = I[s['beatmap_md5']]
    tot = s['geki'] + s['n300'] + s['katu'] + s['n100'] + s['n50'] + s['miss']
    s.update(dict(acc_v1=osudb.acc_v1(s), acc_max=osudb.acc_max(s), mods_s=osudb.mods_str(s['mods']),
                  dt=osudb.ticks_to_dt(s['ticks']), judged=tot, notes=c['notes'], lns=c['ln'],
                  chart=c, labels=L.get(s['beatmap_md5'], [])))
    out.append(s)
json.dump([{k: (str(v) if k == 'dt' else v) for k, v in s.items() if k not in ('chart', 'labels')} for s in out],
          open(os.path.join(HERE, 'plays7k.json'), 'w'))
if __name__ == '__main__':
    who = collections.Counter(s['player'] for s in out)
    print('7K plays by player', who)
    me = [s for s in out if s['player'] in SELF]; al = [s for s in out if s['player'] in ALIAS]
    for name, xs in [('TWulfZ', me), ('aliases(unverified)', al)]:
        ds = sorted(s['dt'] for s in xs)
        lab = [s for s in xs if s['labels']]
        print(name, 'plays', len(xs), 'charts', len({s['beatmap_md5'] for s in xs}), ds[0].date(), '->', ds[-1].date(),
              'on labelled', len(lab), 'labelled charts', len({s['beatmap_md5'] for s in lab}))
        print('  by source', collections.Counter(l['source'] for s in lab for l in s['labels'][:1]))
    # judgement-count check: ScoreV2 LN double-judging
    chk = collections.Counter()
    for s in out:
        if s['lns'] == 0: continue
        k = 'V2' if s['mods'] & (1 << 29) else 'v1'
        chk[(k, 'notes+lns' if s['judged'] == s['notes'] + s['lns'] else 'notes' if s['judged'] == s['notes'] else 'other')] += 1
    print('judged-count check on LN charts', chk)
