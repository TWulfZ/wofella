# Minimal readers for osu! stable scores.db and .osr headers.
# Format ref: https://github.com/ppy/osu/wiki/Legacy-database-file-structure
import struct, datetime

class R:
    def __init__(s, b): s.b = b; s.p = 0
    def u8(s): v = s.b[s.p]; s.p += 1; return v
    def i16(s): v = struct.unpack_from('<H', s.b, s.p)[0]; s.p += 2; return v
    def i32(s): v = struct.unpack_from('<i', s.b, s.p)[0]; s.p += 4; return v
    def i64(s): v = struct.unpack_from('<q', s.b, s.p)[0]; s.p += 8; return v
    def f64(s): v = struct.unpack_from('<d', s.b, s.p)[0]; s.p += 8; return v
    def uleb(s):
        r = sh = 0
        while True:
            x = s.u8(); r |= (x & 0x7f) << sh; sh += 7
            if not x & 0x80: return r
    def string(s):
        t = s.u8()
        if t == 0: return ''
        assert t == 0x0b, f'bad string tag {t} at {s.p-1}'
        n = s.uleb(); v = s.b[s.p:s.p+n].decode('utf-8', 'replace'); s.p += n; return v

def ticks_to_dt(t):
    # .NET DateTime ticks (100ns since 0001-01-01)
    return datetime.datetime(1, 1, 1) + datetime.timedelta(microseconds=t // 10)

def score_body(r):
    d = {}
    d['mode'] = r.u8(); d['version'] = r.i32(); d['beatmap_md5'] = r.string()
    d['player'] = r.string(); d['replay_md5'] = r.string()
    d['n300'], d['n100'], d['n50'], d['geki'], d['katu'], d['miss'] = (r.i16() for _ in range(6))
    d['score'] = r.i32(); d['max_combo'] = r.i16(); d['perfect'] = r.u8(); d['mods'] = r.i32()
    d['lifebar'] = r.string(); d['ticks'] = r.i64()
    return d

def read_scores_db(path):
    r = R(open(path, 'rb').read())
    ver = r.i32(); nb = r.i32(); out = []
    for _ in range(nb):
        md5 = r.string(); ns = r.i32()
        for _ in range(ns):
            d = score_body(r)
            assert r.i32() == -1
            d['online_id'] = r.i64()
            if d['mods'] & (1 << 23): d['tp_acc'] = r.f64()
            d['db_md5'] = md5
            out.append(d)
    assert r.p == len(r.b), (r.p, len(r.b))
    return ver, nb, out

def read_osr_header(path):
    with open(path, 'rb') as f: b = f.read(4096)
    r = R(b); d = score_body(r)
    return d

MODS = {1:'NF',2:'EZ',8:'HD',16:'HR',32:'SD',64:'DT',256:'HT',512:'NC',1024:'FL',4096:'SO',16384:'PF',
        1<<20:'FI',1<<21:'RD',1<<22:'CN',1<<29:'V2',1<<30:'MR',1<<18:'7K',1<<15:'4K',1<<16:'5K',1<<17:'6K',1<<19:'8K'}
def mods_str(m): return ','.join(v for k, v in MODS.items() if m & k) or 'NM'

def acc_v1(d):
    tot = d['geki'] + d['n300'] + d['katu'] + d['n100'] + d['n50'] + d['miss']
    if not tot: return None
    return (300 * (d['geki'] + d['n300']) + 200 * d['katu'] + 100 * d['n100'] + 50 * d['n50']) / (300 * tot)

def acc_max(d):
    # ScoreV2-style weighting: MAX=305, 300=300
    tot = d['geki'] + d['n300'] + d['katu'] + d['n100'] + d['n50'] + d['miss']
    if not tot: return None
    return (305 * d['geki'] + 300 * d['n300'] + 200 * d['katu'] + 100 * d['n100'] + 50 * d['n50']) / (305 * tot)
