"""Correlation oracle: `.osg` events against rejudge.py per-object judgements (spec 006 T9, C1-C5).

Usage:
  correlate.py --corpus <osu_root> [--out FILE] [--songs DIR] [--max-per-group N] [--workers N]

Groups (a play can sit in one base group and in several mod groups):
  base (no EZ/HR/DT/NC/HT/Mirror): v1_rice_nm, v1_ln, v2_rice_nm, v2_ln (LN share threshold below)
  mods: dt_nc, ht, hr, ez, mirror; plus all_mania
Random-mod plays are skipped (rejudge.py cannot attribute columns). Output is JSON written to --out,
never inside the corpus; it holds file names and aggregate numbers only, no player names.
"""
import sys

sys.dont_write_bytecode = True

import argparse
import bisect
import hashlib
import json
import math
import os
import statistics
import tempfile
from collections import Counter, defaultdict, namedtuple
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'rejudge'))
import osg  # noqa: E402
import osudb  # noqa: E402  (rejudge/osudb.py)
import rejudge  # noqa: E402

PARAMS = dict(
    # Spec 006 Correlations: "V1 LN (>= 30% LN)"; plays below it count as rice.
    ln_group_min_share=0.30,
    # Spec 006 C2: (time, kind) join with a +-1 ms tolerance.
    join_tolerance_ms=1,
    # Spec 006 T9 sanity check: C1 on the first 50 events of the three sample files.
    sanity_events=50,
    # Spec 006 H8: f0 increment ~ 150 * log2(max(combo, 2)).
    f0_base=150.0,
    f0_min_combo=2,
    f0_ratio_tolerance=1e-6,
    # ScoreV2 tails use windows x1.5 (rejudge.simulate_v2 tail_mult).
    v2_tail_window_mult=1.5,
)

SANITY_FILES = (
    '0012f2ce06255dbd3b63be22e5ee9218-134350010443098880',
    '02b893b4d9d2a04f646007563a2c271c-134223203794182479',
    '34585edd1c28eef006c46cb41d221827-134334778853511434',
)

# rejudge.J index -> .osg kind name; '320' is osu!mania MAX (geki).
J_TO_KIND = {'320': 'MAX', '300': '300', '200': '200', '100': '100', '50': '50', 'miss': 'miss'}
WINDOW_OK = 3  # index of the 100 window in rejudge's W list, whose late edge ends a note
LN_KIND_TO_CAT = {'note': 'note', 'head': 'head', 'tail': 'tail', 'ln': 'tail'}

JUDGEMENT_MODS = dict(ez=rejudge.M_EZ, hr=rejudge.M_HR, dt=rejudge.M_DT, ht=rejudge.M_HT, nc=rejudge.M_NC,
                      mirror=rejudge.M_MR)
BASE_GROUPS = ('v1_rice_nm', 'v1_ln', 'v2_rice_nm', 'v2_ln')
MOD_GROUPS = ('dt_nc', 'ht', 'hr', 'ez', 'mirror')
ALL_GROUPS = BASE_GROUPS + MOD_GROUPS + ('all_mania',)

Instant = namedtuple('Instant', 't kind cat source')


# ---------- pure helpers (unit-tested) ----------

def _deadline(t, late):
    # rejudge expires an object once now > t + late, so the first integer ms past it.
    return int(t + math.floor(late) + 1)


def judgement_instants(results, W, v2):
    """Map rejudge results to (instant, kind, category, source). Results without an input instant
    (misses by expiry) get the expiry deadline, which is when stable emits them."""
    note_late = W[WINDOW_OK] - 0.5
    tail_late = note_late * PARAMS['v2_tail_window_mult'] if v2 else note_late
    out = []
    for x in results:
        kind = J_TO_KIND[rejudge.J[x['j']]]
        cat = LN_KIND_TO_CAT[x['kind']]
        if x['kind'] == 'ln':
            base, delta = x['end'], x.get('rel_delta')
        else:
            base, delta = x['t'], x.get('delta')
        if delta is not None:
            out.append(Instant(int(round(base + delta)), kind, cat, 'input'))
        else:
            out.append(Instant(_deadline(base, tail_late if cat == 'tail' else note_late), kind, cat, 'deadline'))
    return out


def exact_share(events, instants, by_kind):
    """C1: events whose t_ms equals a rejudge instant (of the same kind when by_kind)."""
    def key(t, k):
        return (t, k) if by_kind else (t, None)

    have = Counter(key(i.t, i.kind) for i in instants)
    ev_n = ev_ok = u_n = u_ok = carried = 0
    by_n = defaultdict(lambda: dict(events=0, events_matched=0))
    prev_surplus = Counter()
    for e in events:
        if not e.n:
            continue
        need = Counter(key(e.t_ms, k) for k in e.kinds)
        surplus = Counter()
        got = 0
        for (t, k), n in need.items():
            h = have.get((t, k), 0)
            got += min(n, h)
            if n > h:
                # A same-frame judgement whose count only shows up in the next record (H4 check).
                carried += min(n - h, prev_surplus.get(k, 0))
            surplus[k] = max(h - n, 0)
        prev_surplus = surplus
        ev_n += 1
        u_n += e.n
        u_ok += got
        ev_ok += got == e.n
        b = by_n[str(e.n) if e.n < 3 else '3+']
        b['events'] += 1
        b['events_matched'] += got == e.n
    return dict(events=ev_n, events_matched=ev_ok, units=u_n, units_matched=u_ok,
                units_carried_from_previous_record=carried, by_n=dict(sorted(by_n.items())))


def join(events, instants, tol, by_kind):
    """C2: assign each judgement unit to rejudge objects under a (time[, kind]) join.
    A record with k units of one kind is unique when exactly k unconsumed candidates exist:
    the per-object judgement is then determined even if the units are indistinguishable."""
    pool = defaultdict(list)
    for i, x in enumerate(instants):
        pool[x.kind if by_kind else None].append((x.t, i))
    for v in pool.values():
        v.sort()
    used = set()
    unique, ambiguous = Counter(), Counter()
    unmatched = units = carry_resolved = 0
    prev_t = None
    for e in events:
        if not e.n:
            continue
        groups = Counter(e.kinds) if by_kind else Counter({None: e.n})
        for kind, need in groups.items():
            if not need:
                continue
            units += need
            lst = pool.get(kind if by_kind else None, [])
            cands = [i for t, i in _window(lst, e.t_ms, tol) if i not in used]
            if len(cands) <= need:
                for i in cands:
                    used.add(i)
                    unique[instants[i].cat] += 1
                missing = need - len(cands)
                unmatched += missing
                if missing and prev_t is not None:
                    # Reported beside, not inside, the strict C2 numbers (H4 same-frame carry).
                    late = [i for t, i in _window(lst, prev_t, tol) if i not in used][:missing]
                    used.update(late)
                    carry_resolved += len(late)
            else:
                cands.sort(key=lambda i: abs(instants[i].t - e.t_ms))
                cats = {instants[i].cat for i in cands}
                cat = cats.pop() if len(cats) == 1 else 'mixed'
                for i in cands[:need]:
                    used.add(i)
                ambiguous[cat] += need
        prev_t = e.t_ms
    return dict(units=units, unique=dict(unique), ambiguous=dict(ambiguous), unmatched=unmatched,
                carry_resolved=carry_resolved)


def _window(sorted_list, t, tol):
    lo = bisect.bisect_left(sorted_list, (t - tol, -1))
    hi = bisect.bisect_right(sorted_list, (t + tol, float('inf')))
    return sorted_list[lo:hi]


def f0_model(combo):
    return PARAMS['f0_base'] * math.log2(max(combo, PARAMS['f0_min_combo']))


# ---------- per play ----------

def _groups(mods, v2, ln_share):
    out = ['all_mania']
    judged = {k for k, bit in JUDGEMENT_MODS.items() if mods & bit}
    if not judged:
        ln = ln_share >= PARAMS['ln_group_min_share']
        out.append(('v2_' if v2 else 'v1_') + ('ln' if ln else 'rice_nm'))
    if judged & {'dt', 'nc'}:
        out.append('dt_nc')
    for k in ('ht', 'hr', 'ez', 'mirror'):
        if k in judged:
            out.append(k)
    return out


def analyse_play(task):
    osg_path, osr_path, osu_path, beatmap_md5 = task
    try:
        f = osg.decode(Path(osg_path).read_bytes())
    except osg.OsgError:
        return dict(file=Path(osg_path).name, skip='osg_decode_error')
    try:
        if hashlib.md5(Path(osu_path).read_bytes()).hexdigest() != beatmap_md5:
            return dict(file=Path(osg_path).name, skip='osu_md5_mismatch')
        r, mi, res, cnt = rejudge.rejudge(osr_path, osu_path)
    except FileNotFoundError:
        return dict(file=Path(osg_path).name, skip='osu_missing')
    except ValueError as e:
        return dict(file=Path(osg_path).name, skip='random' if 'Random' in str(e) else 'rejudge_error')
    except Exception:  # noqa: BLE001 - oracle keeps going per file (spec 006 per-item failure policy)
        return dict(file=Path(osg_path).name, skip='rejudge_error')
    evs = osg.events(f.records)
    v2 = mi['v2']
    instants = judgement_instants(res, mi['W'], v2)
    tol = PARAMS['join_tolerance_ms']

    header_total = sum(r['counts'].values())
    expected = mi['n_obj'] + (mi['n_ln'] if v2 else 0)
    split_ln = (not v2) and mi['n_ln'] > 0 and header_total == mi['n_obj'] + mi['n_ln']
    failed = header_total < expected

    c1_input = [x for x in instants if x.source == 'input']
    c3 = Counter()
    for e in evs:
        if e.n and 'miss' not in e.kinds and e.combo_delta >= 0:
            extra = e.combo_delta - e.n
            c3['records'] += 1
            c3['records_extra_gt0'] += extra > 0
            c3['records_extra_lt0'] += extra < 0
            c3['extra_sum'] += extra
    lr = f.records[-1] if f.records else None
    if lr is not None:
        c3['final_max_combo'] = lr.max_combo
        c3['final_judgements'] = sum(getattr(lr, c) for c in osg.COUNT_FIELDS)

    c5 = dict(ratios=defaultdict(list), f1_nonzero=0, records=0)
    if v2:
        prev_f0 = 0.0
        for e, rec in zip(evs, f.records):
            c5['records'] += 1
            c5['f1_nonzero'] += rec.f1 != 0
            if e.n == 1 and e.kinds[0] != 'miss':
                c5['ratios'][e.kinds[0]].append((rec.f0 - prev_f0) / f0_model(rec.combo))
            prev_f0 = rec.f0

    hp = [rec.hp_raw for rec in f.records]
    return dict(
        file=Path(osg_path).name, groups=_groups(r['mods'], v2, mi['n_ln'] / mi['n_obj'] if mi['n_obj'] else 0.0),
        v2=v2, n_obj=mi['n_obj'], n_ln=mi['n_ln'], failed=failed, split_ln=split_ln,
        rejudge_exact=all(cnt[j] == r['counts'][j] for j in rejudge.J),
        c1=exact_share(evs, c1_input, by_kind=True),
        c1_time=exact_share(evs, c1_input, by_kind=False),
        c1_incl_deadline=exact_share(evs, instants, by_kind=True),
        c1_sample=exact_share(evs[:PARAMS['sanity_events']], c1_input, by_kind=True),
        c1_sample_time=exact_share(evs[:PARAMS['sanity_events']], c1_input, by_kind=False),
        c2=join(evs, instants, tol, by_kind=True),
        c2_time=join(evs, instants, tol, by_kind=False),
        c3=dict(c3),
        c4=dict(min_hp=min(hp) if hp else None, last_hp=hp[-1] if hp else None, touches_zero=0 in hp),
        c5=dict(ratios={k: v for k, v in c5['ratios'].items()}, f1_nonzero=c5['f1_nonzero'], records=c5['records']),
    )


# ---------- aggregation ----------

def _sum_dicts(dst, src):
    for k, v in src.items():
        if isinstance(v, dict):
            _sum_dicts(dst.setdefault(k, {}), v)
        else:
            dst[k] = dst.get(k, 0) + v


def _share(num, den):
    return round(num / den, 6) if den else None


def _c1_view(d):
    return dict(d, event_share=_share(d.get('events_matched', 0), d.get('events', 0)),
                unit_share=_share(d.get('units_matched', 0), d.get('units', 0)))


def _c2_view(d):
    uniq, amb = d.get('unique', {}), d.get('ambiguous', {})
    per_cat = {}
    for cat in sorted(set(uniq) | set(amb)):
        u, a = uniq.get(cat, 0), amb.get(cat, 0)
        per_cat[cat] = dict(unique=u, ambiguous=a, unique_share_of_matched=_share(u, u + a))
    units = d.get('units', 0)
    return dict(units=units, unmatched=d.get('unmatched', 0), carry_resolved=d.get('carry_resolved', 0),
                unique_share=_share(sum(uniq.values()), units),
                unmatched_share=_share(d.get('unmatched', 0), units), per_category=per_cat)


def _hp_view(rows):
    if not rows:
        return dict(n=0)
    last = [r['last_hp'] for r in rows]
    mins = [r['min_hp'] for r in rows]
    return dict(n=len(rows), last_hp_median=statistics.median(last), last_hp_zero=sum(v == 0 for v in last),
                min_hp_median=statistics.median(mins), min_hp_zero=sum(v == 0 for v in mins),
                touches_zero=sum(r['touches_zero'] for r in rows))


def _c5_view(ratios, f1_nonzero, records):
    tol = PARAMS['f0_ratio_tolerance']
    per_kind = {}
    for kind, vals in sorted(ratios.items()):
        med = statistics.median(vals)
        per_kind[kind] = dict(n=len(vals), median_ratio=round(med, 9),
                              share_within_tol_of_median=_share(sum(abs(v - med) <= tol for v in vals), len(vals)))
    return dict(records=records, f1_nonzero=f1_nonzero, ratio_to_model_by_kind=per_kind,
                model='delta_f0 = median_ratio(kind) * f0_base * log2(max(combo_after, f0_min_combo))')


def aggregate(plays):
    groups = {}
    for g in ALL_GROUPS:
        rows = [p for p in plays if g in p['groups']]
        acc = {}
        for key in ('c1', 'c1_time', 'c1_incl_deadline', 'c2', 'c2_time', 'c3'):
            acc[key] = {}
            for p in rows:
                _sum_dicts(acc[key], p[key])
        ratios = defaultdict(list)
        f1_nonzero = records = 0
        for p in rows:
            for k, v in p['c5']['ratios'].items():
                ratios[k].extend(v)
            f1_nonzero += p['c5']['f1_nonzero']
            records += p['c5']['records']
        c3 = acc['c3']
        groups[g] = dict(
            n=len(rows), n_v2=sum(p['v2'] for p in rows), failed=sum(p['failed'] for p in rows),
            split_ln=sum(p['split_ln'] for p in rows), rejudge_exact=sum(p['rejudge_exact'] for p in rows),
            C1=_c1_view(acc['c1']), C1_time_only=_c1_view(acc['c1_time']),
            C1_incl_deadline_misses=_c1_view(acc['c1_incl_deadline']),
            C2=_c2_view(acc['c2']), C2_time_only=_c2_view(acc['c2_time']),
            C3=dict(c3, extra_per_record=_share(c3.get('extra_sum', 0), c3.get('records', 0)),
                    share_records_extra_gt0=_share(c3.get('records_extra_gt0', 0), c3.get('records', 0)),
                    plays_max_combo_gt_judgements=sum(
                        p['c3'].get('final_max_combo', 0) > p['c3'].get('final_judgements', 0) for p in rows)),
            C4=dict(failed=_hp_view([p['c4'] for p in rows if p['failed'] and p['c4']['last_hp'] is not None]),
                    passed=_hp_view([p['c4'] for p in rows if not p['failed'] and p['c4']['last_hp'] is not None])),
            C5=_c5_view(ratios, f1_nonzero, records) if records else None,
        )
    return groups


# ---------- driver ----------

def _refuse_inside(out, root):
    out, root = Path(out).resolve(), Path(root).resolve()
    if out == root or root in out.parents:
        raise SystemExit(f'refusing to write inside the corpus: {out}')


def _default_out():
    base = os.environ.get('SCRATCH') or tempfile.gettempdir()
    return str(Path(base) / 'osg-correlate.json')


def _chart_counts(osu_path):
    _k, _od, _fmt, objs = rejudge.read_osu(osu_path)
    return len(objs), sum(1 for o in objs if o[2] is not None)


def collect_tasks(corpus, songs, bymd5):
    rdir = corpus / 'Data' / 'r'
    stems = defaultdict(set)
    for p in rdir.iterdir():
        parsed = osg.parse_name(p.name)
        if parsed:
            stems[p.stem].add(parsed[2])
    tasks, skipped, no_osg = [], Counter(), []
    pairs = 0
    for stem in sorted(stems):
        exts = stems[stem]
        if 'osr' not in exts:
            if 'osg' in exts:
                skipped['orphan_osg'] += 1
            continue
        osr = rdir / f'{stem}.osr'
        hdr = osg.read_osr_header(osr)
        if hdr['mode'] != osg.MANIA_MODE:
            skipped['non_mania'] += 'osg' in exts
            continue
        m = bymd5.get(stem[:32])
        osu = songs / m['folder'].replace('\\', '/') / m['file'] if m and m['folder'] and m['file'] else None
        if 'osg' not in exts:
            no_osg.append((stem, hdr, m, osu))
            continue
        pairs += 1
        if m is None:
            skipped['no_chart'] += 1
        elif m['mode'] != osg.MANIA_MODE:
            skipped['convert'] += 1
        elif hdr['mods'] & rejudge.M_RD:
            skipped['random'] += 1
        else:
            tasks.append((str(rdir / f'{stem}.osg'), str(osr), str(osu), stem[:32]))
    return tasks, skipped, no_osg, pairs


def failed_without_osg(no_osg):
    """Coverage side of C4: saved failed plays (R4) that have no .osg at all."""
    out = Counter()
    for _stem, hdr, m, osu in no_osg:
        if m is None or osu is None or m['mode'] != osg.MANIA_MODE or not osu.is_file():
            out['unknown_chart'] += 1
            continue
        n_obj, n_ln = _chart_counts(str(osu))
        v2 = bool(hdr['mods'] & osg.MOD_SCORE_V2)
        out['failed' if sum(hdr['counts']) < n_obj + (n_ln if v2 else 0) else 'complete'] += 1
    return dict(out)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--corpus', required=True)
    ap.add_argument('--out', default=_default_out(), help='JSON report path (default: $SCRATCH or the temp dir)')
    ap.add_argument('--songs', help='Songs directory (default: <corpus>/Songs)')
    ap.add_argument('--max-per-group', type=int, default=0, help='cap plays per base/mod group, 0 = all')
    ap.add_argument('--workers', type=int, default=os.cpu_count() or 1)
    a = ap.parse_args(argv)
    corpus = Path(a.corpus)
    _refuse_inside(a.out, corpus)
    songs = Path(a.songs) if a.songs else corpus / 'Songs'

    _ver, maps = osudb.parse(str(corpus / 'osu!.db'))
    bymd5 = {m['md5']: m for m in maps if m['md5']}
    tasks, skipped, no_osg, pairs = collect_tasks(corpus, songs, bymd5)

    with ProcessPoolExecutor(max_workers=max(a.workers, 1)) as ex:
        results = list(ex.map(analyse_play, tasks, chunksize=8))
    plays = []
    for r in results:
        if 'skip' in r:
            skipped[r['skip']] += 1
        else:
            plays.append(r)
    if a.max_per_group:
        kept, per = [], Counter()
        for p in plays:
            specific = [g for g in p['groups'] if g != 'all_mania']
            if any(per[g] < a.max_per_group for g in specific):
                kept.append(p)
                per.update(specific)
        plays = kept

    by_name = {p['file'][:-len('.osg')]: p for p in plays}
    sanity = []
    for stem in SANITY_FILES:
        p = by_name.get(stem)
        sanity.append(dict(file=stem, found=p is not None,
                           c1=_c1_view(p['c1_sample']) if p else None,
                           c1_time_only=_c1_view(p['c1_sample_time']) if p else None))

    report = dict(
        params=PARAMS, corpus=str(corpus), osg_pairs_mania=pairs,
        analysed=len(plays), skipped=dict(skipped), failed_without_osg=failed_without_osg(no_osg),
        sanity=sanity, groups=aggregate(plays),
        note='C1/C2 compare against rejudge.py, whose own parity is imperfect for LN (research 03); '
                       'the *_time_only variants separate timing from judgement-kind disagreement.',
    )
    with open(a.out, 'w', encoding='utf-8') as fh:
        json.dump(report, fh, indent=1, sort_keys=True)
    _print(report, a.out)
    return 0


def _print(rep, out):
    print(f"analysed {rep['analysed']}  skipped {rep['skipped']}  failed_without_osg {rep['failed_without_osg']}")
    for s in rep['sanity']:
        c1, ct = s['c1'] or {}, s['c1_time_only'] or {}
        print(f"sanity {s['file']}: C1 {c1.get('event_share')} time-only {ct.get('event_share')}")
    hdr = 'group          n   v2  fail  exact   C1ev   C1t   C2    C2t  C2unm  C3x>0  C4fail(n,last0)  C4pass(n,last0)'
    print(hdr)
    for g, d in rep['groups'].items():
        c4f, c4p = d['C4']['failed'], d['C4']['passed']
        print(f"{g:<12} {d['n']:>4} {d['n_v2']:>4} {d['failed']:>5} {d['rejudge_exact']:>6} "
              f"{d['C1']['event_share']!s:>6} {d['C1_time_only']['event_share']!s:>6} "
              f"{d['C2']['unique_share']!s:>6} {d['C2_time_only']['unique_share']!s:>6} "
              f"{d['C2']['unmatched_share']!s:>6} {d['C3']['share_records_extra_gt0']!s:>6} "
              f"({c4f['n']},{c4f.get('last_hp_zero')}) ({c4p['n']},{c4p.get('last_hp_zero')})")
    print(f'report written to {out}')


if __name__ == '__main__':
    sys.exit(main())
