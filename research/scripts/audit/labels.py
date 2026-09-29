# Extract ordinal difficulty labels for 7K charts from the local index.
import json, re, csv, collections, os
HERE = os.path.dirname(os.path.abspath(__file__))
R = [json.loads(l) for l in open(os.path.join(HERE, 'index7k.jsonl'))]

# Ladder order per mania-hub algorithms/player/dan-courses.ts:128-158
DAN = {'gamma': 11, 'azimuth': 12, 'zenith': 13, 'stellium': 14}
def dan_ord(s):
    s = s.strip().lower()
    m = re.match(r'(\d+)(st|nd|rd|th)', s)
    if m: return int(m.group(1))
    for k, v in DAN.items():
        if s.startswith(k): return v
    return None

KD = {'1877617': 'jack', '1877625': 'tech', '1877636': 'speed', '1877727': 'stream',
      '1887981': 'ln_general', '1888000': 'ln_tech', '1888009': 'ln_inverse', '1888027': 'ln_release'}
RATE = re.compile(r'\d(?:\.\d+)?x \(\d+bpm\)|\bOD\d|\+FLN|\[\d\.\d+x\]$')
rows = []
def add(r, source, level, skill='', scale='', note='', variant=False):
    rows.append(dict(md5=r['md5'], folder=r['folder'], version=r['version'], creator=r['creator'],
                     setid=r['setid'], bid=r['bid'], source=source, scale=scale or source,
                     skill_slot=skill, ordinal_level=level, is_variant=int(variant),
                     ln_ratio=r['ln_ratio'], note_count=r['notes'], length_s=r['length_s'],
                     nps=round(r['notes'] / r['length_s'], 2) if r['length_s'] else '', od=r['od'], note=note))

for r in R:
    f, v = r['folder'], r['version']
    fid = f.split(' ')[0]
    if v.startswith('Delete Upon download'): continue
    # Jinjin dan courses (marathon charts, one chart per dan)
    if '7K Dan Course - Regular Dan Phase' in f:
        add(r, 'jinjin_dan_regular', dan_ord(v), 'regular_overall', note='marathon course')
    elif '7K Dan Course - LN Dan Phase' in f and 'Emperor' not in f:
        add(r, 'jinjin_dan_ln', dan_ord(v), 'ln_overall', note='marathon course (current LN ladder)')
    elif re.search(r'7K Dan Course - (Insane Level [23] \(LN\)|Extra Level \(LN\))', f):
        add(r, 'jinjin_dan_ln_v1', dan_ord(v), 'ln_overall', note='legacy LN dan (~0.3-0.6 LN ratio); not on current ladder')
    elif "Emperor's 7K LN Dan" in f:
        add(r, 'emperor_ln_dan', dan_ord(v), 'ln_overall', note='third-party LN dan')
    # KomeijiDove practice sets: "~ 9th ~ Song" or "- 9th ~ Song"
    elif (fid in KD or r['setid'] in KD) and re.match(r'^[~-] ', v):
        key = fid if fid in KD else r['setid']
        m = re.match(r'^[~-]\s*([^~]+?)\s*~', v)
        lab = m.group(1) if m else ''
        variant = bool(RATE.search(v)) or '/' in lab
        note = 'section cut: ' + lab.split('/')[1].strip() if '/' in lab else ''
        if r['setid'] == '-1': note = (note + '; ' if note else '') + 'local/unsubmitted copy (setid -1)'
        add(r, 'komeijidove_practice', dan_ord(lab), KD[key], scale='jinjin_dan', note=note, variant=variant)
    elif re.search(r'Wild 7K Dan Course', f):
        add(r, 'wild_dan', dan_ord(v), 'regular_overall', note='marathon course (tyrcs)')
    elif '7K Road to Gamma Dan Pack' in f:
        m = re.search(r'//\s*(\w+)\s*(?:\d|$)', v)
        sk = v.split('//')[-1].strip().split(' ')[0].lower()
        add(r, 'road_to_gamma', 'gamma_entry', sk, scale='jinjin_dan',
            note='single target level (pre-Gamma); no within-pack ordering', variant=bool(RATE.search(v)))
    elif r['creator'] == '5ynt3ck' and f.startswith('[_BMS_]'):
        tags = re.findall(r'\[([a-z]+\d?)_([^\]]+)\]', v)
        for tb, lv in tags:
            lvn = re.match(r'^(\d+)([+-]?)$', lv)
            add(r, 'bms_5ynt3ck', (int(lvn.group(1)) + {'+': 0.33, '-': -0.33, '': 0}[lvn.group(2)]) if lvn else lv,
                'regular_overall', scale='bms_' + tb,
                note=('non-numeric level' if not lvn else '') + ('; multi-table' if len(tags) > 1 else ''))
        if not tags: add(r, 'bms_5ynt3ck', '', '', scale='bms_unlabelled')
    elif re.match(r'\[O2Jam\] \[(\w)\] \[(\d+)\]', v):
        m = re.match(r'\[O2Jam\] \[(\w)\] \[(\d+)\]', v)
        add(r, 'o2jam', int(m.group(2)), '', scale='o2jam_' + m.group(1),
            note='O2Jam level; values <10 look bogus' if int(m.group(2)) < 10 else '')
    elif re.search(r'o2jam', f, re.I) and re.search(r'\[lvl (\d+)\]', v):
        add(r, 'o2jam', int(re.search(r'\[lvl (\d+)\]', v).group(1)), '', scale='o2jam_pack_lvl', note='pack-labelled O2Jam level')
    elif re.search(r'Lv\.(\d+) For O2Jam', v):
        add(r, 'o2jam', int(re.search(r'Lv\.(\d+)', v).group(1)), '', scale='o2jam_pack_lvl')
    elif re.search(r'gamma practice pack|azimuth dan practice pack', f, re.I):
        tgt = 'gamma' if 'gamma' in f.lower() else 'azimuth'
        sk = re.search(r'\((Delay|Chordjack|Bracket|Stamina|Tech|Jack|Speed|Stream)\)', v)
        add(r, 'other_dan_practice', tgt + '_entry', sk.group(1).lower() if sk else '', scale='jinjin_dan',
            note='community pack targeting ' + tgt, variant=bool(RATE.search(v)))

with open(os.path.join(HERE, 'labels.csv'), 'w', newline='') as fo:
    w = csv.DictWriter(fo, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
c = collections.Counter((x['source'], x['scale']) for x in rows)
for k, n in sorted(c.items()): print(k, n)
print('rows', len(rows), 'distinct md5', len({x['md5'] for x in rows}))
