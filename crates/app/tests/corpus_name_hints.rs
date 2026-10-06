#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Feature `name-hints`: how far the pattern engine agrees with the pilot's pack, folder and
//! difficulty names. A measurement, not a calibration gate: it prints the agreement table and
//! the most frequent hint tokens, and asserts only that hints were written and read back.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use wolluf_app::clock::SystemClock;
use wolluf_app::context::{AppContext, AppPaths};
use wolluf_app::jobs::JobStatusDto;
use wolluf_app::jobs::dto::JobKindDto;
use wolluf_core::Clock;
use wolluf_engine::labels::{scale, source};
use wolluf_engine::stage::chart_label;
use wolluf_store::open_cache_db;
use wolluf_store::repo::cache::{LabelFilter, chart_label as label_repo};

const PERCENT: f64 = 100.0;
const TOP_TOKENS: usize = 10;

fn pct(share: Option<f64>) -> String {
    share.map_or_else(|| "-".to_owned(), |s| format!("{:.1}", s * PERCENT))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
async fn corpus_name_hints() {
    let root = common::corpus();
    let tree_before = common::tree_state(&root);

    let data = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_data_dir(data.path().join("data"));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ctx = AppContext::open(paths.clone(), clock).unwrap();
    let install = ctx.register_install(root.clone(), None).await.unwrap();
    let sync = ctx.plays().sync_and_wait(install).await.unwrap();
    assert_eq!(sync.status, JobStatusDto::Ok, "{sync:?}");
    ctx.jobs().wait_idle().await;
    let jobs = ctx.jobs().list(None).await.unwrap();
    let index = jobs
        .iter()
        .find(|j| j.kind == JobKindDto::IndexLibrary)
        .expect("SyncPlays chains an index");
    assert_eq!(index.status, JobStatusDto::Ok, "{index:?}");
    let rows = ctx.library().hint_agreement().await.unwrap();
    drop(ctx);

    let cache = open_cache_db(&paths.cache_db()).unwrap();
    let key = chart_label::vkey().unwrap();
    let mut hints = Vec::new();
    for scale in [scale::HINT_AXIS, scale::HINT_PATTERN] {
        let filter = LabelFilter {
            scale: Some(scale.to_owned()),
            ..LabelFilter::default()
        };
        hints.extend(
            cache
                .read(|c| label_repo::list_filtered(c, key, &filter, u32::MAX))
                .unwrap(),
        );
    }
    drop(cache);
    hints.retain(|(_, l)| l.source == source::NAME_HINT);
    let mut charts: Vec<_> = hints.iter().map(|(md5, _)| *md5).collect();
    charts.sort_unstable();
    charts.dedup();
    let variants = hints.iter().filter(|(_, l)| l.is_variant).count();

    println!(
        "corpus_name_hints: {} hint rows on {} charts ({variants} rows on variants)",
        hints.len(),
        charts.len()
    );
    println!(
        "{:<4} {:<12} {:<34} {:>7} {:>9} {:>7} {:>7} {:>6}",
        "keys", "scale", "target", "hinted", "segmented", "mean%", "base%", "lift"
    );
    for r in &rows {
        println!(
            "{:<4} {:<12} {:<34} {:>7} {:>9} {:>7} {:>7} {:>6}",
            r.keymode,
            r.scale,
            r.target_id,
            r.hinted_charts,
            r.segmented_charts,
            pct(r.mean_share),
            pct(Some(r.baseline_share)),
            r.lift.map_or_else(|| "-".to_owned(), |l| format!("{l:.2}"))
        );
    }
    let mut tokens: BTreeMap<(&str, &str), u64> = BTreeMap::new();
    for (_, l) in &hints {
        let token = l.skill_tag.as_deref().unwrap_or("");
        *tokens.entry((token, l.level_text.as_str())).or_default() += 1;
    }
    let mut top: Vec<_> = tokens.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("top {TOP_TOKENS} tokens (rows, token -> target):");
    for ((token, target), n) in top.iter().take(TOP_TOKENS) {
        println!("  {n:>6}  {token:<20} -> {target}");
    }

    assert!(!hints.is_empty(), "the pilot's names carry no hint");
    assert!(!rows.is_empty(), "no agreement row");
    assert!(
        rows.iter().any(|r| r.segmented_charts > 0),
        "no hinted chart is segmented"
    );
    common::assert_unchanged("sync and index", &tree_before, &common::tree_state(&root));
}
