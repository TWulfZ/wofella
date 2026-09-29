"""Oracle decoder and structural survey for osu! stable `.osg` files (spec 006 T1).

Usage:
  osg.py --survey <corpus_root> [--json OUT] [--examples N]
  osg.py --dump <file.osg> [--limit N] [--events]

Layout under test (spec 006 H0/H1): i32 client_version, i32 record_count, then record_count
fixed-size little-endian records of 29 bytes (ScoreV1) or 45 bytes (ScoreV2, two trailing f64).
The paired `.osr` header is read with a reader of its own, as in rejudge.py:read_osr, so this
oracle does not depend on the codecs it cross-checks. Nothing is ever written under the corpus.
"""
import sys

sys.dont_write_bytecode = True

import argparse
import datetime
import json
import operator
import re
import struct
from collections import Counter, defaultdict, namedtuple
from pathlib import Path

HEADER = struct.Struct('<ii')
_V1_FMT = '<iB6HiHHBHB'
RECORD_V1 = struct.Struct(_V1_FMT)
RECORD_V2 = struct.Struct(_V1_FMT + 'dd')
STRIDE_V1 = RECORD_V1.size
STRIDE_V2 = RECORD_V2.size
assert (STRIDE_V1, STRIDE_V2) == (29, 45)

_V1_FIELDS = ('t_ms b4 c300 c100 c50 cmax c200 cmiss score max_combo combo b25 hp_raw b28').split()
OsgRecord = namedtuple('OsgRecord', _V1_FIELDS + ['f0', 'f1'], defaults=(None, None))
COUNT_FIELDS = ('c300', 'c100', 'c50', 'cmax', 'c200', 'cmiss')
_COUNT_IDX = [_V1_FIELDS.index(c) for c in COUNT_FIELDS]
# Event kinds are listed in the fixed order MAX,300,200,100,50,miss (spec 006 Behaviour).
KIND_ORDER = (('MAX', 'cmax'), ('300', 'c300'), ('200', 'c200'), ('100', 'c100'), ('50', 'c50'), ('miss', 'cmiss'))

OsgFile = namedtuple('OsgFile', 'client_version count stride records')
OsgEvent = namedtuple('OsgEvent', 'idx t_ms added kinds n combo_delta score_delta')

MOD_SCORE_V2 = 1 << 29
MANIA_MODE = 3
NAME_RE = re.compile(r'^([0-9a-f]{32})-([0-9]+)\.(osr|osg)$')
FILETIME_EPOCH = datetime.datetime(1601, 1, 1)
FILETIME_TICKS_PER_US = 10
# The .osr fields this survey needs all precede the lifebar string, so a prefix read is enough.
OSR_PREFIX_BYTES = 1024


class OsgError(ValueError):
    pass


def decode(b):
    if len(b) < HEADER.size:
        raise OsgError(f'truncated header: len={len(b)}')
    ver, count = HEADER.unpack_from(b, 0)
    if count < 0:
        raise OsgError(f'negative record count: {count}')
    body = len(b) - HEADER.size
    if count == 0:
        if body:
            raise OsgError(f'stride mismatch: len={len(b)} count=0')
        return OsgFile(ver, 0, None, [])
    stride, rem = divmod(body, count)
    if rem or stride not in (STRIDE_V1, STRIDE_V2):
        raise OsgError(f'stride mismatch: len={len(b)} count={count}')
    rs = RECORD_V1 if stride == STRIDE_V1 else RECORD_V2
    records = [OsgRecord(*r) for r in rs.iter_unpack(memoryview(b)[HEADER.size:])]
    return OsgFile(ver, count, stride, records)


def encode(client_version, records):
    """Test helper: the inverse of decode. Real .osg bytes are never written by wolluf."""
    out = [HEADER.pack(client_version, len(records))]
    for r in records:
        if r.f0 is None:
            out.append(RECORD_V1.pack(*r[:len(_V1_FIELDS)]))
        else:
            out.append(RECORD_V2.pack(*r))
    return b''.join(out)


def events(records):
    prev = None
    out = []
    for i, r in enumerate(records):
        added = {}
        kinds = []
        for kind, field in KIND_ORDER:
            d = getattr(r, field) - (getattr(prev, field) if prev else 0)
            added[kind] = d
            kinds.extend([kind] * max(d, 0))
        combo_delta = r.combo - (prev.combo if prev else 0)
        score_delta = r.score - (prev.score if prev else 0)
        out.append(OsgEvent(i, r.t_ms, added, kinds, len(kinds), combo_delta, score_delta))
        prev = r
    return out


def parse_name(name):
    m = NAME_RE.match(name)
    if not m:
        return None
    return m.group(1), int(m.group(2)), m.group(3)


def filetime_to_datetime(ft):
    return FILETIME_EPOCH + datetime.timedelta(microseconds=ft // FILETIME_TICKS_PER_US)


class _Reader:
    def __init__(self, b):
        self.b, self.o = b, 0

    def take(self, n):
        if self.o + n > len(self.b):
            raise EOFError
        v = self.b[self.o:self.o + n]
        self.o += n
        return v

    def unpack(self, fmt):
        s = struct.Struct(fmt)
        return s.unpack(self.take(s.size))

    def string(self):
        tag = self.take(1)[0]
        if tag == 0:
            return None
        if tag != 0x0b:
            raise ValueError(f'bad string tag {tag} at {self.o - 1}')
        n = sh = 0
        while True:
            c = self.take(1)[0]
            n |= (c & 0x7f) << sh
            sh += 7
            if not c & 0x80:
                break
        return self.take(n).decode('utf-8', 'replace')


def read_osr_header(path):
    with open(path, 'rb') as f:
        b = f.read(OSR_PREFIX_BYTES)
    try:
        return _osr_header(b)
    except EOFError:
        with open(path, 'rb') as f:
            return _osr_header(f.read())


def _osr_header(b):
    r = _Reader(b)
    mode, ver = r.unpack('<Bi')
    md5, player, _replay_md5 = r.string(), r.string(), r.string()
    c300, c100, c50, cmax, c200, cmiss = r.unpack('<6H')
    score, max_combo, perfect, mods = r.unpack('<iHBi')
    return dict(mode=mode, version=ver, md5=md5, player=player,
                counts=(c300, c100, c50, cmax, c200, cmiss), score=score,
                max_combo=max_combo, perfect=perfect, mods=mods)


# ---------- survey ----------

INVARIANTS = ('I1', 'I2', 'I3', 'I3_b28', 'I3_b4', 'I3_b25', 'I4', 'I5')


class _Tally:
    def __init__(self, examples):
        self.examples = examples
        self.rows = {k: dict(pass_=0, fail=0, na=0, examples=[]) for k in INVARIANTS}

    def mark(self, inv, ok, name):
        row = self.rows[inv]
        if ok is None:
            row['na'] += 1
        elif ok:
            row['pass_'] += 1
        else:
            row['fail'] += 1
            if len(row['examples']) < self.examples:
                row['examples'].append(name)

    def as_dict(self):
        return {k: {'pass': v['pass_'], 'fail': v['fail'], 'na': v['na'], 'examples': v['examples']}
                for k, v in self.rows.items()}


def _nondecreasing(col):
    col = list(col)
    return col == sorted(col)


def _check_file(name, raw, hdr, tally, dist):
    try:
        f = decode(raw)
    except OsgError as e:
        tally.mark('I1', False, name)
        for inv in INVARIANTS[1:]:
            tally.mark(inv, None, name)
        dist['decode_errors'].append(f'{name}: {e}')
        return None
    tally.mark('I1', True, name)
    dist['client_versions'][f.client_version] += 1
    dist['strides'][f.stride if f.stride is not None else 'empty'] += 1
    if hdr is not None:
        dist['modes'][hdr['mode']] += 1
        dist['version_eq_osr'][f.client_version == hdr['version']] += 1

    is_v2 = hdr is not None and bool(hdr['mods'] & MOD_SCORE_V2)
    if hdr is None or f.stride is None:
        tally.mark('I2', None, name)
    else:
        tally.mark('I2', (f.stride == STRIDE_V2) == is_v2, name)

    if not f.records:
        for inv in ('I3', 'I3_b28', 'I3_b4', 'I3_b25', 'I4', 'I5'):
            tally.mark(inv, None, name)
        dist['empty_graph'] += 1
        return f

    cols = list(zip(*f.records))
    fi = _V1_FIELDS.index
    want_b28 = 1 if f.stride == STRIDE_V2 else 0
    ok_b28 = set(cols[fi('b28')]) == {want_b28}
    ok_b4 = set(cols[fi('b4')]) == {0}
    ok_b25 = set(cols[fi('b25')]) == {0}
    tally.mark('I3_b28', ok_b28, name)
    tally.mark('I3_b4', ok_b4, name)
    tally.mark('I3_b25', ok_b25, name)
    tally.mark('I3', ok_b28 and ok_b4 and ok_b25, name)
    if not ok_b25:
        dist['nonzero_b25_records'] += sum(1 for v in cols[fi('b25')] if v)

    monotone = _nondecreasing(cols[fi('t_ms')]) and all(_nondecreasing(cols[i]) for i in _COUNT_IDX)
    tally.mark('I4', monotone, name)

    totals = list(map(sum, zip(*(cols[i] for i in _COUNT_IDX))))
    per_rec = [totals[0]] + list(map(operator.sub, totals[1:], totals[:-1]))
    last = len(per_rec) - 1
    for i, n in enumerate(per_rec):
        dist['judgements_per_record'][n if n < 3 else '3+'] += 1
        if n == 0:
            dist['zero_judgement_position']['first' if i == 0 else ('last' if i == last else 'middle')] += 1
    dist['records'] += len(per_rec)

    lr = f.records[-1]
    if hdr is None:
        tally.mark('I5', None, name)
    else:
        counts_eq = tuple(getattr(lr, c) for c in COUNT_FIELDS) == hdr['counts']
        score_eq = lr.score == hdr['score']
        tally.mark('I5', counts_eq and score_eq, name)
        if not (counts_eq and score_eq):
            dist['i5_failures'].append(dict(
                file=name, v2=is_v2, mode=hdr['mode'], mods=hdr['mods'], counts_eq=counts_eq, score_eq=score_eq,
                osg_counts=[getattr(lr, c) for c in COUNT_FIELDS], osr_counts=list(hdr['counts']),
                osg_score=lr.score, osr_score=hdr['score']))
    return f


def survey(root, examples=5):
    rdir = Path(root) / 'Data' / 'r'
    pairs = defaultdict(dict)
    unnamed = []
    for p in sorted(rdir.iterdir()):
        parsed = parse_name(p.name)
        if parsed is None:
            unnamed.append(p.name)
            continue
        md5, ft, ext = parsed
        pairs[(md5, ft)][ext] = p

    tally = _Tally(examples)
    dist = dict(client_versions=Counter(), strides=Counter(), modes=Counter(), version_eq_osr=Counter(),
                judgements_per_record=Counter(), zero_judgement_position=Counter(), records=0,
                empty_graph=0, nonzero_b25_records=0, decode_errors=[], i5_failures=[])
    missing_osg = dict(by_month=Counter(), by_player=Counter(), by_version=Counter(), by_mode=Counter(), total=0)
    first_osg_ft = None
    n_osg = n_osr = orphan_osg = 0
    missing_after_first = []

    for key in sorted(pairs):
        files = pairs[key]
        hdr = None
        if 'osr' in files:
            n_osr += 1
            hdr = read_osr_header(files['osr'])
        if 'osg' in files:
            n_osg += 1
            if hdr is None:
                orphan_osg += 1
            first_osg_ft = key[1] if first_osg_ft is None else min(first_osg_ft, key[1])
            _check_file(files['osg'].name, files['osg'].read_bytes(), hdr, tally, dist)
        elif hdr is not None:
            missing_after_first.append((key[1], hdr))

    for ft, hdr in missing_after_first:
        missing_osg['total'] += 1
        missing_osg['by_month'][filetime_to_datetime(ft).strftime('%Y-%m')] += 1
        missing_osg['by_player'][hdr['player'] or ''] += 1
        missing_osg['by_version'][hdr['version']] += 1
        missing_osg['by_mode'][hdr['mode']] += 1
    after = [h for ft, h in missing_after_first if first_osg_ft is not None and ft >= first_osg_ft]
    missing_osg['since_first_osg'] = len(after)
    missing_osg['since_first_osg_by_player'] = Counter(h['player'] or '' for h in after)

    def plain(c):
        return {str(k): v for k, v in sorted(c.items(), key=lambda kv: str(kv[0]))}

    return dict(
        root=str(root),
        osg_files=n_osg, osr_files=n_osr, orphan_osg=orphan_osg, unrecognised_names=len(unnamed),
        first_osg=filetime_to_datetime(first_osg_ft).isoformat() if first_osg_ft else None,
        invariants=tally.as_dict(),
        distributions=dict(
            client_versions=plain(dist['client_versions']), strides=plain(dist['strides']),
            modes=plain(dist['modes']), client_version_equals_osr=plain(dist['version_eq_osr']),
            judgements_per_record=plain(dist['judgements_per_record']),
            zero_judgement_position=plain(dist['zero_judgement_position']),
            records=dist['records'], empty_graph=dist['empty_graph'],
            nonzero_b25_records=dist['nonzero_b25_records']),
        decode_errors=dist['decode_errors'],
        i5_failures=dist['i5_failures'],
        osr_without_osg={k: (plain(v) if isinstance(v, Counter) else v) for k, v in missing_osg.items()},
    )


def _print_survey(rep):
    print(f"root {rep['root']}")
    print(f"osg {rep['osg_files']}  osr {rep['osr_files']}  orphan_osg {rep['orphan_osg']}  "
          f"unrecognised {rep['unrecognised_names']}  first_osg {rep['first_osg']}")
    print('invariant   pass  fail    na  examples')
    for k, v in rep['invariants'].items():
        denom = v['pass'] + v['fail']
        print(f"{k:<8} {v['pass']:>7} {v['fail']:>5} {v['na']:>5}  {v['pass']}/{denom}  {' '.join(v['examples'])}")
    for k, v in rep['distributions'].items():
        print(f'{k}: {v}')
    if rep['decode_errors']:
        print('decode_errors:', *rep['decode_errors'], sep='\n  ')
    print(f"i5_failures ({len(rep['i5_failures'])}):")
    for f in rep['i5_failures']:
        print(f"  {f['file']} v2={f['v2']} mode={f['mode']} mods={f['mods']} counts_eq={f['counts_eq']} "
              f"score_eq={f['score_eq']} osg={f['osg_counts']}/{f['osg_score']} osr={f['osr_counts']}/{f['osr_score']}")
    print('osr_without_osg:')
    for k, v in rep['osr_without_osg'].items():
        print(f'  {k}: {v}')


def _dump(path, limit, want_events):
    f = decode(Path(path).read_bytes())
    score_system = {STRIDE_V1: 'v1', STRIDE_V2: 'v2'}.get(f.stride, 'unknown')
    print(f'client_version={f.client_version} record_count={f.count} stride={f.stride} score_system={score_system}')
    recs = f.records[:limit] if limit else f.records
    if want_events:
        print('idx t_ms n kinds combo_delta score_delta')
        for e in events(f.records)[:len(recs)]:
            print(e.idx, e.t_ms, e.n, ','.join(e.kinds) or 'score_only', e.combo_delta, e.score_delta)
    else:
        print(' '.join(OsgRecord._fields if f.stride == STRIDE_V2 else _V1_FIELDS))
        for r in recs:
            print(' '.join(str(v) for v in (r if f.stride == STRIDE_V2 else r[:len(_V1_FIELDS)])))


def _refuse_inside(out, root):
    out, root = Path(out).resolve(), Path(root).resolve()
    if out == root or root in out.parents:
        raise SystemExit(f'refusing to write inside the corpus: {out}')


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument('--survey', metavar='CORPUS_ROOT')
    g.add_argument('--dump', metavar='FILE')
    ap.add_argument('--json', metavar='OUT', help='write the survey report as JSON (never inside the corpus)')
    ap.add_argument('--examples', type=int, default=5, help='example file names kept per failing invariant')
    ap.add_argument('--limit', type=int, default=0)
    ap.add_argument('--events', action='store_true')
    a = ap.parse_args(argv)
    if a.dump:
        _dump(a.dump, a.limit, a.events)
        return 0
    if a.json:
        _refuse_inside(a.json, a.survey)
    rep = survey(a.survey, a.examples)
    _print_survey(rep)
    if a.json:
        with open(a.json, 'w', encoding='utf-8') as fh:
            json.dump(rep, fh, indent=1, ensure_ascii=False)
    return 0


if __name__ == '__main__':
    sys.exit(main())
