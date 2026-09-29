"""List the user's local 7K mania replays by joining Data/r filenames (md5-ticks.osr) with the osu!.db index."""
import json, os, sys, datetime
from osrparse import Replay

R = "/mnt/e/Games/osu!/Data/r"
maps = json.load(open(os.path.join(os.path.dirname(__file__), 'maps.json')))
rows = []
for fn in os.listdir(R):
    if not fn.endswith('.osr'):
        continue
    md5, ticks = fn[:-4].split('-')
    m = maps.get(md5)
    if not m or m['mode'] != 3 or round(m['cs']) != 7:
        continue
    rows.append((int(ticks), fn, m))
rows.sort(reverse=True)
print('7K replays:', len(rows), file=sys.stderr)
out = []
for ticks, fn, m in rows[:int(sys.argv[1]) if len(sys.argv) > 1 else 60]:
    r = Replay.from_path(os.path.join(R, fn))
    out.append(dict(fn=fn, player=r.username, mods=int(r.mods), c320=r.count_geki, c300=r.count_300, c200=r.count_katu,
                    c100=r.count_100, c50=r.count_50, miss=r.count_miss, score=r.score, combo=r.max_combo,
                    ts=str(r.timestamp), ver=r.game_version, od=m['od'], title=f"{m['artist']} - {m['title']} [{m['diff']}]",
                    folder=m['folder'], file=m['file'], md5=m['md5']))
json.dump(out, open(os.path.join(os.path.dirname(__file__), 'replays7k.json'), 'w'), indent=1)
for o in out:
    tot = o['c320'] + o['c300'] + o['c200'] + o['c100'] + o['c50'] + o['miss']
    acc = (300 * (o['c320'] + o['c300']) + 200 * o['c200'] + 100 * o['c100'] + 50 * o['c50']) / (300 * tot) if tot else 0
    print(f"{o['ts'][:16]} mods={o['mods']:<6} od={o['od']:<4} n={tot:<5} acc={acc*100:6.2f} miss={o['miss']:<4} {o['title'][:70]}")
