#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! F1 acceptance: the pilot's 7K catalog is parsed and labelled, and a second index parses
//! nothing. The label reference is the local-calibration audit (research 03 l.296–318), which
//! counted `.osu` files under `Songs/`; the index counts unique md5 of the osu!.db catalog, so
//! every delta against it is bounded and explained below instead of compared for equality.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use wolluf_app::clock::SystemClock;
use wolluf_app::context::{AppContext, AppPaths};
use wolluf_app::jobs::JobStatusDto;
use wolluf_app::jobs::dto::{IndexLibrarySummaryDto, JobDto, JobKindDto, JobSummaryDto};
use wolluf_core::Clock;
use wolluf_engine::stage::{chart_label, chart_parse};
use wolluf_store::open_cache_db;
use wolluf_store::repo::cache::{
    ChartLabel, DerivationStatus, catalog_chart, chart_label as label_repo, chart_parsed,
    derivation,
};
use wolluf_store::time::parse_rfc3339_ms;

const K7: u8 = 7;
/// Audit: 18,589 unique md5 among 18,905 7K `.osu` files on disk.
const AUDIT_UNIQUE_7K: u64 = 18_589;
/// osu!.db and the audit's `Songs/` walk disagree on the margins: files osu! has not rescanned
/// since they were added or regenerated in place, and nested folders the walk skipped. On the
/// pilot (2026-09-29) the 7K catalog is 18,530 md5 and 18,298 match their file; the other 232 are
/// O2Jam (131) and BMS (100) converts rewritten since the last scan, plus one chart.
const CATALOG_DRIFT: f64 = 0.02;
/// Acceptance: parse failures < 0.5% of the 7K catalog.
const MAX_FAILED_SHARE: f64 = 0.005;
/// Generous for a debug build over drvfs.
const INDEX_BUDGET: Duration = Duration::from_secs(10 * 60);
const AUDIT_ROWS: u64 = 8_755;
const AUDIT_CHARTS: u64 = 7_768;

/// One audit line (research 03 l.298–303) and how far the index may sit below or above it.
/// Labels come from osu!.db names, the audit's from the files, so the deltas are the files
/// osu!.db and the walk disagree on (reconciled chart by chart on 2026-09-29):
/// - BMS −627: 501 rows on files osu!.db has not scanned yet, and 126 on files edited since it
///   did. Merging identical tags on one chart (the store key) removes none on this corpus.
/// - O2Jam +432: the charts under `Songs/Normal/`, which osu!.db lists and the audit's one-level
///   walk never reached. Another 131 charts edited since the last scan keep their osu!.db level
///   (`[N]`, the files now say `[H]`): that moves rows between `o2jam_h` and `o2jam_n` only.
/// - Every other source is exact.
struct AuditLine {
    source: &'static str,
    rows: u64,
    below: u64,
    above: u64,
}

const fn exact(source: &'static str, rows: u64) -> AuditLine {
    AuditLine {
        source,
        rows,
        below: 0,
        above: 0,
    }
}

const AUDIT: &[AuditLine] = &[
    AuditLine {
        source: "bms_5ynt3ck",
        rows: 7_755,
        below: 627,
        above: 0,
    },
    AuditLine {
        source: "o2jam",
        rows: 745,
        below: 0,
        above: 432,
    },
    exact("komeijidove_practice", 146),
    exact("jinjin_dan_regular", 14),
    exact("jinjin_dan_ln", 14),
    exact("jinjin_dan_ln_v1", 11),
    exact("wild_dan", 9),
    exact("road_to_gamma", 15),
    exact("emperor_ln_dan", 1),
    exact("other_dan_practice", 45),
];

fn index_summary(job: &JobDto) -> IndexLibrarySummaryDto {
    assert_eq!(job.status, JobStatusDto::Ok, "{job:?}");
    let Some(JobSummaryDto::IndexLibrary(summary)) = &job.summary else {
        panic!("index finished without a summary: {job:?}");
    };
    summary.clone()
}

fn job_duration(job: &JobDto) -> Duration {
    let at = |t: &Option<String>| parse_rfc3339_ms(t.as_deref().unwrap()).unwrap().0;
    Duration::from_micros(u64::try_from(at(&job.ended) - at(&job.started)).unwrap())
}

fn share(n: u64, of: u64) -> f64 {
    n as f64 / of as f64
}

#[derive(Default)]
struct Tally {
    rows: u64,
    charts: u64,
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
async fn corpus_library_index() {
    let root = common::corpus();
    let tree_before = common::tree_state(&root);

    let data = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_data_dir(data.path().join("data"));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ctx = AppContext::open(paths.clone(), clock).unwrap();
    let install = ctx.register_install(root.clone(), None).await.unwrap();

    let sync = ctx.plays().sync_and_wait(install).await.unwrap();
    assert_eq!(sync.status, JobStatusDto::Ok, "{sync:?}");
    // SyncPlays chains IndexLibrary; waiting for idle lets that first index run to its end.
    let wait = Instant::now();
    ctx.jobs().wait_idle().await;
    let follow_ups = wait.elapsed();
    let jobs = ctx.jobs().list(None).await.unwrap();
    let chained: Vec<&JobDto> = jobs
        .iter()
        .filter(|j| j.kind == JobKindDto::IndexLibrary)
        .collect();
    assert_eq!(chained.len(), 1, "one chained index: {chained:?}");
    let first = index_summary(chained[0]);
    let first_time = job_duration(chained[0]);

    let second_job = ctx.library().index_and_wait().await.unwrap();
    let second = index_summary(&second_job);
    let second_time = job_duration(&second_job);
    let scales = ctx.library().counts_by_scale().await.unwrap();
    drop(ctx);

    let cache = open_cache_db(&paths.cache_db()).unwrap();
    let parse_key = chart_parse::vkey().unwrap();
    let label_key = chart_label::vkey().unwrap();
    let catalog = cache
        .read(|c| catalog_chart::list_by_keymode(c, K7))
        .unwrap();
    let parsed_rows = cache.read(|c| chart_parsed::count(c, parse_key)).unwrap();
    let parse_memo: Vec<_> = cache
        .read(derivation::list_all)
        .unwrap()
        .into_iter()
        .filter(|d| d.stage == chart_parse::STAGE && d.vkey == parse_key)
        .collect();
    let labels: Vec<Vec<ChartLabel>> = cache
        .read(|c| {
            catalog
                .iter()
                .map(|chart| label_repo::list_for(c, chart.md5, label_key))
                .collect()
        })
        .unwrap();
    drop(cache);

    let k7 = catalog.len() as u64;
    let parsed = u64::from(first.parsed_new);
    println!(
        "corpus_library_index: 7K catalog {k7}, parsed {parsed} (audit {AUDIT_UNIQUE_7K} unique \
         md5 on disk), chart_parsed rows {parsed_rows}"
    );
    println!(
        "first index {first_time:?} (sync follow-ups {follow_ups:?}): {first:?}\n\
         second index {second_time:?}: {second:?}"
    );

    println!("not parsed, by status and reason:");
    let mut by_reason: BTreeMap<(&str, String), (u64, Vec<&str>)> = BTreeMap::new();
    for d in parse_memo
        .iter()
        .filter(|d| d.status != DerivationStatus::Ok)
    {
        let code = d.error_code.map_or("-", |c| c.as_str());
        let msg = d.error_msg.as_deref().unwrap_or_default();
        let entry = by_reason
            .entry((d.status.as_str(), format!("{code} {}", mask_md5(msg))))
            .or_default();
        entry.0 += 1;
        if entry.1.len() < 5 {
            entry.1.push(&d.input_key);
        }
    }
    for ((status, reason), (n, examples)) in &by_reason {
        println!("  {status:<8} {n:>6}  {reason}  e.g. {examples:?}");
    }

    let mut per_source: BTreeMap<&str, Tally> = BTreeMap::new();
    let (mut rows, mut charts) = (0_u64, 0_u64);
    for chart_labels in &labels {
        let mut sources: Vec<&str> = chart_labels.iter().map(|l| l.source.as_str()).collect();
        rows += chart_labels.len() as u64;
        charts += u64::from(!chart_labels.is_empty());
        for s in &sources {
            per_source.entry(s).or_default().rows += 1;
        }
        sources.sort_unstable();
        sources.dedup();
        for s in sources {
            per_source.entry(s).or_default().charts += 1;
        }
    }
    println!("per scale (rows / charts):");
    for s in &scales {
        println!("  {:<24} {:>6} {:>6}", s.scale, s.rows, s.charts);
    }
    println!("per source (rows / charts) vs audit rows:");
    let tally = |source: &str| {
        per_source
            .get(source)
            .map_or((0, 0), |t| (t.rows, t.charts))
    };
    for line in AUDIT {
        let (r, c) = tally(line.source);
        println!(
            "  {:<22} {r:>6} {c:>6}  audit {:>6}  delta {:+}",
            line.source,
            line.rows,
            i128::from(r) - i128::from(line.rows)
        );
    }
    let unaudited: Vec<&&str> = per_source
        .keys()
        .filter(|s| !AUDIT.iter().any(|l| l.source == **s))
        .collect();
    println!(
        "  {:<22} {rows:>6} {charts:>6}  audit {AUDIT_ROWS:>6} rows / {AUDIT_CHARTS} charts; \
         sources outside the audit: {unaudited:?}",
        "total"
    );

    // The catalog, not the audit, is what the index accounts for, chart by chart.
    assert_eq!(u64::from(first.charts_total), k7);
    let failed = u64::from(first.failed_items);
    let unavailable = u64::from(first.skipped_unavailable);
    assert_eq!(
        parsed + unavailable + failed,
        k7,
        "every catalog chart is parsed, unavailable or failed"
    );
    assert_eq!(parsed, parsed_rows);
    assert_eq!(first.skipped_memoized, 0, "a fresh data dir has no memo");
    assert!(
        share(failed, k7) < MAX_FAILED_SHARE,
        "{failed} parse failures in {k7} charts"
    );
    for (what, n) in [("7K catalog", k7), ("parsed", parsed)] {
        assert!(
            share(n.abs_diff(AUDIT_UNIQUE_7K), AUDIT_UNIQUE_7K) < CATALOG_DRIFT,
            "{what} {n} vs audit {AUDIT_UNIQUE_7K}"
        );
    }

    assert_eq!(second.parsed_new, 0, "the second run parses nothing");
    assert_eq!(second.labels_written, 0, "labels are memoized");
    assert_eq!(second.failed_items, 0, "failures are memoized");
    assert_eq!(u64::from(second.skipped_memoized), parsed + failed);
    assert_eq!(
        second.skipped_unavailable, first.skipped_unavailable,
        "an unavailable chart is retried, and is still unavailable"
    );

    assert!(
        unaudited.is_empty(),
        "label sources the audit never produced"
    );
    for line in AUDIT {
        let (r, _) = tally(line.source);
        assert!(
            r + line.below >= line.rows && r <= line.rows + line.above,
            "{}: {r} rows vs audit {} (-{} / +{})",
            line.source,
            line.rows,
            line.below,
            line.above
        );
    }
    let below: u64 = AUDIT.iter().map(|l| l.below).sum();
    let above: u64 = AUDIT.iter().map(|l| l.above).sum();
    for (what, n, audit) in [("rows", rows, AUDIT_ROWS), ("charts", charts, AUDIT_CHARTS)] {
        assert!(
            n + below >= audit && n <= audit + above,
            "labelled {what} {n} vs audit {audit}"
        );
    }
    assert!(
        scales
            .iter()
            .all(|s| !s.scale.chars().any(|c| c.is_ascii_uppercase())),
        "scale ids are persisted StableIds, lowercase"
    );
    assert!(first_time < INDEX_BUDGET, "first index took {first_time:?}");

    common::assert_unchanged(
        "sync and two indexes",
        &tree_before,
        &common::tree_state(&root),
    );
}

/// Groups messages that differ only by the md5 they name.
fn mask_md5(msg: &str) -> String {
    msg.split(' ')
        .map(|w| {
            let hex = w.trim_end_matches(',');
            if hex.len() == 32 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                w.replacen(hex, "<md5>", 1)
            } else {
                w.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
