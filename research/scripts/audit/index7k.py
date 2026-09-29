# Index 7K mania charts in osu! stable Songs dir. Read-only on the osu! install.
import os, sys, json, hashlib, concurrent.futures as cf
ROOT = "/mnt/e/Games/osu!/Songs"
OUT = os.path.join(os.path.dirname(__file__), "index7k.jsonl")

def parse(path):
    try:
        with open(path, 'rb') as f:
            data = f.read()
    except Exception as e:
        return ('err', path, str(e))
    text = data.decode('utf-8', 'replace')
    sec = None; h = {}
    lines = text.splitlines()
    ho_start = None
    for i, ln in enumerate(lines):
        s = ln.strip()
        if s.startswith('[') and s.endswith(']'):
            sec = s[1:-1]
            if sec == 'HitObjects':
                ho_start = i + 1; break
            continue
        if sec in ('General', 'Metadata', 'Difficulty') and ':' in s:
            k, v = s.split(':', 1); h[k.strip()] = v.strip()
    mode = h.get('Mode', '0'); cs = h.get('CircleSize', '')
    if mode != '3':
        return ('skip', mode)
    try: csf = float(cs)
    except: csf = -1
    if csf != 7:
        return ('mania', csf)
    n = ln_n = 0; first = last = None; cols = [0]*7
    for ln in lines[ho_start or len(lines):]:
        p = ln.strip().split(',')
        if len(p) < 5: continue
        try:
            x = int(float(p[0])); t = int(float(p[2])); ty = int(p[3])
        except: continue
        end = t
        if ty & 128:
            ln_n += 1
            try: end = int(p[5].split(':')[0])
            except: pass
        n += 1
        col = min(6, max(0, x * 7 // 512)); cols[col] += 1
        first = t if first is None else min(first, t)
        last = end if last is None else max(last, end)
    return ('7k', {
        'md5': hashlib.md5(data).hexdigest(),
        'folder': os.path.basename(os.path.dirname(path)),
        'file': os.path.basename(path),
        'artist': h.get('Artist', ''), 'title': h.get('Title', ''),
        'creator': h.get('Creator', ''), 'version': h.get('Version', ''),
        'source': h.get('Source', ''), 'tags': h.get('Tags', ''),
        'setid': h.get('BeatmapSetID', ''), 'bid': h.get('BeatmapID', ''),
        'od': h.get('OverallDifficulty', ''), 'hp': h.get('HPDrainRate', ''),
        'notes': n, 'ln': ln_n, 'ln_ratio': round(ln_n / n, 4) if n else 0,
        'length_s': round(((last or 0) - (first or 0)) / 1000, 1),
        'cols': cols,
    })

paths = []
for d in os.scandir(ROOT):
    if d.is_dir():
        try:
            for f in os.scandir(d.path):
                if f.name.lower().endswith('.osu'): paths.append(f.path)
        except Exception as e:
            print('ERR', d.path, e, file=sys.stderr)
print('osu files', len(paths), file=sys.stderr)
from collections import Counter
cnt = Counter(); keys = Counter()
with cf.ThreadPoolExecutor(32) as ex, open(OUT, 'w') as out:
    for r in ex.map(parse, paths, chunksize=64):
        cnt[r[0]] += 1
        if r[0] == 'mania': keys[r[1]] += 1
        if r[0] == '7k':
            keys[7.0] += 1; out.write(json.dumps(r[1], ensure_ascii=False) + '\n')
print(dict(cnt), file=sys.stderr); print('mania keys', dict(keys), file=sys.stderr)
