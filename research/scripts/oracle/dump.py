"""Oracle dump for the Rust codec parity tests (spec 002 T14/T15).

Usage: dump.py <corpus_root> <out_dir>

Writes osu_db.jsonl, scores_db.jsonl, osr_headers.jsonl and meta.json into <out_dir>.
It only wraps the existing readers (rejudge/osudb.py, audit/osudb.py); any parsing
change belongs there, so the oracle stays independent of the Rust port.
"""
import sys

# Importing the readers must not leave __pycache__ inside research/ (T14 verify).
sys.dont_write_bytecode = True

import importlib.util
import json
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent


def _load(name, rel):
    # Both readers are named osudb.py, so a plain import would shadow one of them.
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / rel)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _write_jsonl(path, rows):
    n = 0
    with open(path, 'w', encoding='utf-8') as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False, sort_keys=True))
            f.write('\n')
            n += 1
    return n


def _is_within(child, parent):
    try:
        child.relative_to(parent)
        return True
    except ValueError:
        return False


def main(argv):
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    corpus = Path(argv[1]).resolve()
    out = Path(argv[2]).resolve()
    if _is_within(out, corpus):
        print(f'refusing to write inside the corpus: {out}', file=sys.stderr)
        return 2
    out.mkdir(parents=True, exist_ok=True)

    osu_reader = _load('oracle_rejudge_osudb', 'rejudge/osudb.py')
    audit_reader = _load('oracle_audit_osudb', 'audit/osudb.py')

    osu_ver, maps = osu_reader.parse(str(corpus / 'osu!.db'))
    n_maps = _write_jsonl(out / 'osu_db.jsonl', ({'idx': i, **m} for i, m in enumerate(maps)))

    scores_path = corpus / 'scores.db'
    scores_ver = scores_beatmaps = None
    n_scores = 0
    if scores_path.is_file():
        scores_ver, scores_beatmaps, scores = audit_reader.read_scores_db(str(scores_path))
        n_scores = _write_jsonl(out / 'scores_db.jsonl', ({'idx': i, **s} for i, s in enumerate(scores)))
    else:
        _write_jsonl(out / 'scores_db.jsonl', ())

    replay_dir = corpus / 'Data' / 'r'
    osr_files = sorted(p for p in replay_dir.iterdir() if p.suffix == '.osr') if replay_dir.is_dir() else []

    def headers():
        for p in osr_files:
            yield {'file': p.name, **audit_reader.read_osr_header(str(p))}

    n_osr = _write_jsonl(out / 'osr_headers.jsonl', headers())

    meta = {
        'osu_db_version': osu_ver,
        'osu_db_entries': n_maps,
        'scores_db_version': scores_ver,
        'scores_db_beatmaps': scores_beatmaps,
        'scores_db_scores': n_scores,
        'osr_headers': n_osr,
    }
    with open(out / 'meta.json', 'w', encoding='utf-8') as f:
        json.dump(meta, f, indent=2, sort_keys=True)
    print(json.dumps(meta, sort_keys=True), file=sys.stderr)
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
