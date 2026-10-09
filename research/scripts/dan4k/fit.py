"""Fit a 4K Overall-MSD -> dan table from dan-course and dan-practice charts in a local library.

Input: one or more JSON files from `wolluf --json library list --keys 4 --limit N [--offset K]`
(a list of LibraryChartDto; `msdOverallCenti` is MinaCalc Overall x 100 at rate 1.0).

Run: python3 -I research/scripts/dan4k/fit.py <library.json>... [--report out.json]

Method and results: docs/research/07-k4-dan-from-msd.md. Stdlib only. Uses no Daniel/Sunny code
and no third-party dan labels: the only labels are the dan names pack authors put in their charts.
"""

from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field

GREEK = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa"]

# Rice only: MinaCalc ignores holds, so an LN-heavy chart's MSD says little about its dan.
LN_MAX = 0.20

_DAN = (
    r"(?:(?P<ord>10th|[1-9](?:st|nd|rd|th))"
    r"|(?:extra-)?(?P<greek>" + "|".join(GREEK) + r"))"
)
# A dan token in a difficulty name counts only in the shapes pack authors use for it, so a song
# word ("Beta Jack trill") is not read as a tier.
VERSION_FORMS = [
    re.compile(r"~\s*" + _DAN + r"\s*~", re.I),  # course: "Flashes ~ 2nd ~ (Marathon)"
    re.compile(r"^\s*\[\s*" + _DAN + r"\b[^\]]*\]", re.I),  # tiered: "[Delta Low] Pluto"
    re.compile(r"^\s*" + _DAN + r"\s+-\s", re.I),  # thumb course: "1st - I (Chroma)"
]
# A pack's own tier is the token that opens its title ("10th Dan ...", "Alpha Jack ...");
# a token later in a title ("Dan ~ REFORM ~ 2nd Pack", "from Beta to Gamma") names something else.
TITLE_TIER = re.compile(r"^\s*" + _DAN + r"(?![a-z])", re.I)
ANY_DAN = re.compile(r"\b" + _DAN + r"(?![a-z])|\bdan\b", re.I)
PACK_WORD = re.compile(r"\b(dan|practice|pack|collection|journey)\b", re.I)
OTHER_KEYMODE = re.compile(r"\b(?:[5-9]|10)k\b", re.I)
THUMB = re.compile(r"\bthumb\b", re.I)
PRE_TIER = re.compile(r"\bpre-" + _DAN, re.I)
PLACEHOLDER = re.compile(r"\bdelete\b|do not play", re.I)
LN_WORD = re.compile(r"\bLN\b|long ?note", re.I)
COURSE = re.compile(r"\(marathon\)", re.I)
RATE = re.compile(r"\b\d+(?:\.\d+)?x\b|\bx\d+(?:\.\d+)?\b", re.I)

RULES = [
    "keys 4 only; a title naming another keymode (5K-10K) is skipped",
    "the title must look like a pack (dan|practice|pack|collection|journey)",
    "thumb-play packs are skipped from the fit (different input scale) and kept as an external check",
    "'Pre-<tier>' packs are skipped: they sit between two dans",
    "placeholder difficulties (delete / do not play) are skipped",
    f"LN-heavy charts (lnRatio > {LN_MAX} or LN in the title) are skipped",
    "unrated charts (no msdOverallCenti) are skipped",
    "dan from the difficulty name when it has '~ X ~', '[X ...]' or 'X - ', else from a tier that opens the pack title",
    "'(Marathon)' course charts with a rate in the name are rate variants and are skipped",
    "1st..10th = 1..10, Alpha = 11, Beta = 12, ... ; EXTRA-X counts as X; sub-tiers (Low/Mid/High) are ignored",
    "pack id = title (re-uploads by different creators fold together for leave-one-pack-out)",
]


def dan_index(m: re.Match) -> int:
    if m.group("ord"):
        return int(re.match(r"\d+", m.group("ord")).group())
    return 11 + GREEK.index(m.group("greek").lower())


def dan_label(dan: int) -> str:
    if dan <= 10:
        suffix = {1: "st", 2: "nd", 3: "rd"}.get(dan, "th")
        return f"{dan}{suffix}"
    return GREEK[dan - 11].capitalize()


def version_dan(version: str) -> int | None:
    for form in VERSION_FORMS:
        m = form.search(version)
        if m:
            return dan_index(m)
    return None


@dataclass
class Verdict:
    matched: bool
    reason: str = ""
    dan: int | None = None
    pack: str = ""


@dataclass
class Chart:
    md5: str
    pack: str
    dan: int
    msd: int
    version: str = ""


def classify(row: dict) -> Verdict:
    title, version = row["title"], row["version"]
    pack = title.strip()

    def skip(reason: str, dan: int | None = None) -> Verdict:
        return Verdict(False, reason, dan, pack)

    if OTHER_KEYMODE.search(title):
        return skip("other keymode")
    if not PACK_WORD.search(title):
        return skip("not a pack")
    if THUMB.search(title):
        return skip("thumb scale", version_dan(version))
    if PRE_TIER.search(title):
        return skip("pre-tier")
    if PLACEHOLDER.search(version):
        return skip("placeholder")
    if (row.get("lnRatio") or 0.0) > LN_MAX or LN_WORD.search(title):
        return skip("LN-heavy")
    if row.get("msdOverallCenti") is None:
        return skip("no MSD")
    dan = version_dan(version)
    if dan is None:
        m = TITLE_TIER.search(title)
        dan = dan_index(m) if m else None
    if dan is None:
        return skip("no dan token")
    if COURSE.search(version) and RATE.search(version):
        return skip("course rate variant", dan)
    return Verdict(True, "", dan, pack)


def is_candidate(row: dict) -> bool:
    return bool(ANY_DAN.search(row["title"]) or ANY_DAN.search(row["version"]))


def pav(values: list[float], weights: list[float]) -> list[float]:
    """Weighted pool-adjacent-violators: the non-decreasing fit closest to `values`."""
    blocks: list[list[float]] = []  # [mean, weight, count]
    for v, w in zip(values, weights):
        blocks.append([v, w, 1])
        while len(blocks) > 1 and blocks[-2][0] > blocks[-1][0]:
            v2, w2, n2 = blocks.pop()
            v1, w1, n1 = blocks.pop()
            blocks.append([(v1 * w1 + v2 * w2) / (w1 + w2), w1 + w2, n1 + n2])
    out: list[float] = []
    for v, _, n in blocks:
        out.extend([v] * n)
    return out


@dataclass
class Table:
    bounds: list[tuple[int, int]]  # (dan, lower_bound_centi), ascending
    top_span: int
    means: dict[int, float] = field(default_factory=dict)


def table_from_means(means: dict[int, float]) -> Table:
    dans = sorted(means)
    m = [means[d] for d in dans]
    raw = [m[0] - (m[1] - m[0]) / 2] + [(a + b) / 2 for a, b in zip(m, m[1:])]
    bounds: list[tuple[int, int]] = []
    for d, r in zip(dans, raw):
        lower = int(round(r))
        # Pooled (tied) means give equal midpoints; an equal bound would make a dan unreachable.
        if bounds and lower <= bounds[-1][1]:
            lower = bounds[-1][1] + 1
        bounds.append((d, lower))
    top_span = bounds[-1][1] - bounds[-2][1]
    return Table(bounds, top_span, dict(means))


def predict(table: Table, msd: int) -> int:
    dan = 0
    for d, lower in table.bounds:
        if lower <= msd:
            dan = d
    return dan


def dan_means(charts: list[Chart]) -> tuple[dict[int, float], dict[int, int]]:
    """Pack-balanced: each pack's mean at a dan counts once, so a 30-chart practice pack does not
    outweigh one course chart per dan."""
    per: dict[int, dict[str, list[int]]] = defaultdict(lambda: defaultdict(list))
    for c in charts:
        per[c.dan][c.pack].append(c.msd)
    means, packs = {}, {}
    for d, by_pack in per.items():
        pack_means = [sum(v) / len(v) for v in by_pack.values()]
        means[d] = sum(pack_means) / len(pack_means)
        packs[d] = len(pack_means)
    return means, packs


def fit(charts: list[Chart]) -> Table:
    means, packs = dan_means(charts)
    dans = sorted(means)
    pooled = pav([means[d] for d in dans], [packs[d] for d in dans])
    return table_from_means(dict(zip(dans, pooled)))


def score(pairs: list[tuple[int, int]]) -> dict:
    n = len(pairs)
    if n == 0:
        return {"charts": 0, "exact_pct": None, "within1_pct": None, "mae": None, "bias": None}
    errs = [abs(p - t) for t, p in pairs]
    return {
        "charts": n,
        "exact_pct": round(100.0 * sum(e == 0 for e in errs) / n, 1),
        "within1_pct": round(100.0 * sum(e <= 1 for e in errs) / n, 1),
        "mae": round(sum(errs) / n, 3),
        "bias": round(sum(p - t for t, p in pairs) / n, 3),
    }


def leave_one_pack_out(charts: list[Chart]) -> dict:
    pairs_all: list[tuple[int, int]] = []
    baseline: list[tuple[int, int]] = []
    per_pack = {}
    for pack in sorted({c.pack for c in charts}):
        train = [c for c in charts if c.pack != pack]
        held = [c for c in charts if c.pack == pack]
        if len({c.dan for c in train}) < 2:
            continue
        table = fit(train)
        pairs = [(c.dan, predict(table, c.msd)) for c in held]
        pairs_all.extend(pairs)
        per_pack[pack] = score(pairs)
        # Constant-guess reference: what an MSD-blind estimate scores on the same folds.
        median = sorted(c.dan for c in train)[len(train) // 2]
        baseline.extend((c.dan, median) for c in held)
    result = score(pairs_all)
    result["per_pack"] = per_pack
    result["baseline_median_dan"] = score(baseline)
    return result


def load(paths: list[str]) -> list[dict]:
    rows, seen = [], set()
    for path in paths:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        for r in data:
            if r.get("keymode", 4) == 4 and r["md5"] not in seen:
                seen.add(r["md5"])
                rows.append(r)
    return rows


def main(argv: list[str]) -> int:
    report_path = None
    if "--report" in argv:
        i = argv.index("--report")
        report_path = argv[i + 1]
        argv = argv[:i] + argv[i + 2 :]
    if not argv:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    rows = load(argv)

    charts: list[Chart] = []
    skipped: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    thumb: list[tuple[int, int, str]] = []
    for r in rows:
        v = classify(r)
        if v.matched:
            charts.append(Chart(r["md5"], v.pack, v.dan, r["msdOverallCenti"], r["version"]))
            continue
        if v.reason == "thumb scale" and v.dan is not None and r.get("msdOverallCenti") is not None:
            if not PLACEHOLDER.search(r["version"]):
                thumb.append((v.dan, r["msdOverallCenti"], r["version"]))
        if is_candidate(r):
            skipped[v.reason][v.pack] += 1

    print(f"# 4K dan fit: {len(rows)} 4K charts read, {len(charts)} matched\n")
    print("## Rules")
    for rule in RULES:
        print(f"- {rule}")

    print("\n## Matched (pack: charts per dan)")
    by_pack: dict[str, dict[int, int]] = defaultdict(lambda: defaultdict(int))
    for c in charts:
        by_pack[c.pack][c.dan] += 1
    for pack in sorted(by_pack):
        dans = ", ".join(f"{dan_label(d)}x{n}" for d, n in sorted(by_pack[pack].items()))
        print(f"- {pack} [{sum(by_pack[pack].values())}]: {dans}")

    print("\n## Skipped candidates (reason: pack [charts])")
    for reason in sorted(skipped):
        packs = "; ".join(f"{p} [{n}]" for p, n in sorted(skipped[reason].items()))
        print(f"- {reason}: {packs}")

    means, packs = dan_means(charts)
    chart_level: dict[int, list[int]] = defaultdict(list)
    for c in charts:
        chart_level[c.dan].append(c.msd)
    table = fit(charts)
    pooled = table.means

    print("\n## Per-dan Overall MSD (1.0x)")
    print("| dan | charts | packs | chart mean | pack-balanced mean | after PAV | min | max |")
    print("|---|---|---|---|---|---|---|---|")
    per_dan = []
    for d in sorted(means):
        v = chart_level[d]
        cm = sum(v) / len(v)
        print(
            f"| {dan_label(d)} | {len(v)} | {packs[d]} | {cm / 100:.2f} | {means[d] / 100:.2f}"
            f" | {pooled[d] / 100:.2f} | {min(v) / 100:.2f} | {max(v) / 100:.2f} |"
        )
        per_dan.append({
            "dan": d, "label": dan_label(d), "charts": len(v), "packs": packs[d],
            "chart_mean_centi": round(cm), "pack_mean_centi": round(means[d]),
            "pooled_mean_centi": round(pooled[d]), "min_centi": min(v), "max_centi": max(v),
        })

    print("\n## Table (lower bound; Low/Mid/High thirds start at)")
    print("| dan | lower | mid from | high from |")
    print("|---|---|---|---|")
    rows_out = []
    for i, (d, lower) in enumerate(table.bounds):
        span = table.bounds[i + 1][1] - lower if i + 1 < len(table.bounds) else table.top_span
        mid, high = lower + -(-span // 3), lower + -(-2 * span // 3)
        print(f"| {dan_label(d)} | {lower / 100:.2f} | {mid / 100:.2f} | {high / 100:.2f} |")
        rows_out.append({"label": dan_label(d), "lower_bound_centi": lower, "span_centi": span})
    print(f"\ntop_span_centi = {table.top_span}")

    print("\n## Rust (DanTable4k default)")
    for d, lower in table.bounds:
        print(f'    ("{dan_label(d)}", {lower}),')
    print(f"    top_span_centi: {table.top_span},")

    lopo = leave_one_pack_out(charts)
    in_sample = score([(c.dan, predict(table, c.msd)) for c in charts])
    print("\n## Validation")
    print(f"- leave-one-pack-out: {fmt(lopo)}")
    print(f"  - reference, constant median dan of the fold: {fmt(lopo['baseline_median_dan'])}")
    for pack, s in lopo["per_pack"].items():
        print(f"  - {pack}: {fmt(s)}")
    print(f"- in-sample (fit on all): {fmt(in_sample)}")
    thumb_score = score([(d, predict(table, m)) for d, m, _ in thumb])
    print(f"- external, thumb-play course (not fitted): {fmt(thumb_score)}")
    for d, m, ver in sorted(thumb):
        print(f"  - {dan_label(d)} {m / 100:.2f} -> {dan_label(predict(table, m)) if predict(table, m) else 'below 1st'}  ({ver})")

    if report_path:
        report = {
            "input_charts": len(rows),
            "msd": "msdOverallCenti: MinaCalc Overall x 100 at 1.0x (calc 527)",
            "rules": RULES,
            "matched": [c.__dict__ for c in charts],
            "skipped": {r: dict(p) for r, p in skipped.items()},
            "per_dan": per_dan,
            "table": rows_out,
            "top_span_centi": table.top_span,
            "leave_one_pack_out": lopo,
            "in_sample": in_sample,
            "external_thumb": thumb_score,
        }
        with open(report_path, "w", encoding="utf-8") as f:
            json.dump(report, f, indent=2, ensure_ascii=False)
            f.write("\n")
        print(f"\nreport -> {report_path}")
    return 0


def fmt(s: dict) -> str:
    if not s["charts"]:
        return "no charts"
    return f"n={s['charts']} exact {s['exact_pct']}% within-1 {s['within1_pct']}% MAE {s['mae']} bias {s['bias']:+}"


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
