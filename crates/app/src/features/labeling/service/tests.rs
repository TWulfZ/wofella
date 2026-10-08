use std::collections::BTreeSet;

use wolluf_core::ErrorCode;

use super::*;
use crate::features::labeling::dto::{
    AnchorDto, ChartPickDto, ChartTimelineDto, ChartTimelineRequestDto, LabelSelectionDto,
    LabelSubmitDto, MoveWindowRequestDto, NowPlayingRequestDto, NowPlayingSourceDto,
    RandomRequestDto, ResizeWindowRequestDto, SampleRequestDto, SelectionCountDto, SpanDto,
    WindowAtRequestDto, WindowOpDto, WindowPickDto,
};
use crate::features::library::testkit::{Map, install, osu_text, synced};
use crate::features::plays::testkit::Fixture;

const REGULAR_DAN: &str = "1 7K Dan Course - Regular Dan Phase";
const ALL_COLS: [u8; 7] = [1, 2, 3, 4, 5, 6, 7];

/// A 7K chart with a tap every 125 ms over 20 s; the title keeps the bytes unique.
fn long_map(title: &str) -> Map {
    let taps: Vec<(u8, i32)> = (0..160).map(|i| ((i % 7) as u8, i * 125)).collect();
    Map::new(title, 7, osu_text(7, title, &taps, &[]))
}

fn maps() -> Vec<Map> {
    vec![
        long_map("three").named(REGULAR_DAN, "3rd Dan"),
        long_map("eight").named(REGULAR_DAN, "8th Dan"),
        long_map("plain"),
    ]
}

fn request(seed: &str, round: u32, exclude: Vec<AnchorDto>) -> SampleRequestDto {
    SampleRequestDto {
        keymode: 7,
        seed: seed.to_owned(),
        round,
        window_ms: None,
        scale: None,
        level_min: None,
        level_max: None,
        exclude,
    }
}

fn submit(anchor: &AnchorDto, patterns: &[&str]) -> LabelSubmitDto {
    LabelSubmitDto {
        anchor: anchor.clone(),
        patterns: patterns.iter().map(|p| (*p).to_owned()).collect(),
        no_pattern: false,
        mixed: false,
        unsure: false,
        thumb_pref: None,
        selection: picked(ChartPickDto::Sampled, WindowPickDto::Sampled),
    }
}

fn picked(pick: ChartPickDto, window: WindowPickDto) -> LabelSelectionDto {
    LabelSelectionDto { pick, window }
}

fn anchor(md5: &str, t0_ms: i32, t1_ms: i32) -> AnchorDto {
    AnchorDto {
        md5: md5.to_owned(),
        t0_ms,
        t1_ms,
        cols: ALL_COLS.to_vec(),
    }
}

fn overlaps(a: &AnchorDto, b: &AnchorDto) -> bool {
    a.md5 == b.md5 && a.t0_ms < b.t1_ms && b.t0_ms < a.t1_ms
}

async fn library() -> (Fixture, Vec<Map>) {
    let maps = maps();
    let (f, _) = synced(&maps, &[]).await;
    (f, maps)
}

#[tokio::test(flavor = "multi_thread")]
async fn pattern_examples_give_one_synthetic_window_per_pattern() {
    let (f, _) = library().await;
    let svc = f.ctx.labeling();
    let examples = svc.pattern_examples(7, None).await.unwrap();
    let ids: Vec<&str> = examples.iter().map(|e| e.id.as_str()).collect();
    let taxonomy: Vec<String> = svc.taxonomy(7).unwrap().into_iter().map(|p| p.id).collect();
    assert_eq!(ids, taxonomy);
    for ex in &examples {
        let w = &ex.window;
        assert_eq!(
            (w.md5.as_str(), w.keymode, w.layout.id.as_str()),
            ("00000000000000000000000000000000", 7, "k7.313_right_thumb"),
            "{}",
            ex.id
        );
        assert_eq!(w.audio_filename, None);
        assert!(w.from_ms < w.to_ms, "{}", ex.id);
        assert!(
            w.notes
                .iter()
                .any(|n| w.from_ms <= n.t_ms && n.t_ms <= w.to_ms),
            "{}",
            ex.id
        );
    }
    let inverse = examples.iter().find(|e| e.id == "ln.inverse.gap").unwrap();
    assert!(inverse.window.notes.iter().all(|n| n.end_ms.is_some()));
    assert_eq!(
        svc.pattern_examples(4, None).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn taxonomy_lists_the_keymode_patterns() {
    let (f, _) = library().await;
    let svc = f.ctx.labeling();
    let k7 = svc.taxonomy(7).unwrap();
    assert_eq!(k7.len(), 26);
    let minijack = k7.iter().find(|p| p.id == "regular.jack.minijack").unwrap();
    assert_eq!(
        (minijack.axis.as_str(), minijack.key.as_str()),
        ("7k.regular.jack", "mj")
    );
    assert_eq!(svc.taxonomy(4).unwrap_err().code, ErrorCode::InvalidInput);
}

/// Shells (Tauri) need every service future to be `Send` for any borrow of the context.
#[test]
fn service_futures_are_send() {
    fn is_send<T: Send>(_: T) {}
    fn check(
        ctx: &crate::context::AppContext,
        req: SampleRequestDto,
        at: WindowAtRequestDto,
        random: RandomRequestDto,
        playing: NowPlayingRequestDto,
    ) {
        is_send(async move { ctx.labeling().sample(req).await });
        is_send(async move { ctx.labeling().window_at(at).await });
        is_send(async move { ctx.labeling().random(random).await });
        is_send(async move { ctx.labeling().now_playing(playing).await });
    }
    let _ = check;
}

#[tokio::test(flavor = "multi_thread")]
async fn sample_is_deterministic_and_skips_labelled_windows() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let first = svc.sample(request("42", 0, vec![])).await.unwrap().unwrap();
    assert_eq!(
        svc.sample(request("42", 0, vec![])).await.unwrap(),
        Some(first.clone())
    );
    assert_eq!(first.anchor.t1_ms - first.anchor.t0_ms, 4_000);
    assert_eq!(first.anchor.cols, ALL_COLS);
    let map = maps.iter().find(|m| m.md5 == first.anchor.md5).unwrap();
    assert_eq!(first.title, map.title);
    assert!(first.stratum.contains("/nps_"), "{first:?}");

    svc.submit(submit(&first.anchor, &["regular.stream.single"]))
        .await
        .unwrap();
    for round in 0..6 {
        let w = svc
            .sample(request("42", round, vec![]))
            .await
            .unwrap()
            .unwrap();
        assert!(
            !overlaps(&w.anchor, &first.anchor),
            "stored labels are avoided without being passed in: {w:?}"
        );
    }
    let mut seen = vec![first.anchor.clone()];
    for round in 0..9 {
        let Some(w) = svc
            .sample(request("42", round, seen.clone()))
            .await
            .unwrap()
        else {
            break;
        };
        assert!(
            !overlaps(&w.anchor, &first.anchor),
            "round {round} reuses a labelled window: {w:?}"
        );
        assert!(!seen.iter().any(|s| overlaps(s, &w.anchor)), "{w:?}");
        seen.push(w.anchor);
    }
    let strata: BTreeSet<String> = futures_strata(&svc).await;
    assert!(strata.len() >= 3, "dan 3, dan 8 and unlevelled: {strata:?}");
}

async fn futures_strata(svc: &LabelingService<'_>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for round in 0..3 {
        if let Some(w) = svc.sample(request("7", round, vec![])).await.unwrap() {
            out.insert(w.stratum);
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn sample_filters_by_label_and_window() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let eight = &maps[1];
    for round in 0..3 {
        let w = svc
            .sample(SampleRequestDto {
                scale: Some("jinjin_dan_regular".into()),
                level_min: Some(5.0),
                window_ms: Some(2_000),
                ..request("1", round, vec![])
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(w.anchor.md5, eight.md5);
        assert_eq!(w.anchor.t1_ms - w.anchor.t0_ms, 2_000);
        assert_eq!(w.level.as_deref(), Some("jinjin_dan_regular:8th"));
    }
    let bad_seed = svc.sample(request("-1", 0, vec![])).await.unwrap_err();
    assert_eq!(bad_seed.code, ErrorCode::InvalidInput);
    let zero = svc
        .sample(SampleRequestDto {
            window_ms: Some(0),
            ..request("1", 0, vec![])
        })
        .await
        .unwrap_err();
    assert_eq!(zero.code, ErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn submit_undo_and_stats_roundtrip() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let (three, eight) = (&maps[0].md5, &maps[1].md5);
    let a = svc
        .submit(LabelSubmitDto {
            mixed: true,
            ..submit(
                &anchor(three, 1_000, 5_000),
                &["regular.stream.single", "regular.jack.minijack"],
            )
        })
        .await
        .unwrap();
    let b = svc
        .submit(LabelSubmitDto {
            unsure: true,
            ..submit(&anchor(eight, 0, 4_000), &["regular.jack.minijack"])
        })
        .await
        .unwrap();
    assert!(b.id > a.id, "ids follow append order");

    let stats = svc.stats().await.unwrap();
    assert_eq!((stats.total, stats.mixed, stats.unsure), (2, 1, 1));
    let count = |list: &[crate::features::labeling::dto::CountDto], key: &str| {
        list.iter().find(|c| c.key == key).map(|c| c.count)
    };
    assert_eq!(count(&stats.per_pattern, "regular.jack.minijack"), Some(2));
    assert_eq!(count(&stats.per_pattern, "regular.stream.single"), Some(1));
    assert_eq!(count(&stats.per_axis, "7k.regular.jack"), Some(2));
    assert_eq!(count(&stats.per_axis, "7k.regular.stream"), Some(1));
    let strata: u32 = stats.per_stratum.iter().map(|c| c.count).sum();
    assert_eq!(strata, 2);
    assert!(
        stats
            .per_stratum
            .iter()
            .any(|c| c.key.starts_with("dan_08/")),
        "{stats:?}"
    );

    svc.undo(&a.id).await.unwrap();
    let stats = svc.stats().await.unwrap();
    assert_eq!((stats.total, stats.mixed), (1, 0));
    assert_eq!(svc.undo(&a.id).await.unwrap_err().code, ErrorCode::Conflict);
    let unknown = ulid::Ulid::from_parts(1, 1).to_string();
    assert_eq!(
        svc.undo(&unknown).await.unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        svc.undo("not a ulid").await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn submit_rejects_bad_input() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let md5 = &maps[0].md5;
    let ok = anchor(md5, 0, 4_000);
    let cases = [
        (submit(&ok, &[]), ErrorCode::InvalidInput),
        (submit(&ok, &["regular.jack.nope"]), ErrorCode::InvalidInput),
        (submit(&ok, &["mj"]), ErrorCode::InvalidInput),
        (
            submit(&anchor(md5, 4_000, 4_000), &["regular.jack.minijack"]),
            ErrorCode::InvalidInput,
        ),
        (
            submit(
                &AnchorDto {
                    cols: vec![0],
                    ..ok.clone()
                },
                &["regular.jack.minijack"],
            ),
            ErrorCode::InvalidInput,
        ),
        (
            submit(
                &AnchorDto {
                    cols: vec![8],
                    ..ok.clone()
                },
                &["regular.jack.minijack"],
            ),
            ErrorCode::InvalidInput,
        ),
        (
            submit(&anchor("nope", 0, 4_000), &["regular.jack.minijack"]),
            ErrorCode::InvalidInput,
        ),
        (
            submit(
                &anchor(&"0".repeat(32), 0, 4_000),
                &["regular.jack.minijack"],
            ),
            ErrorCode::NotFound,
        ),
    ];
    for (req, code) in cases {
        let err = svc.submit(req.clone()).await.unwrap_err();
        assert_eq!(err.code, code, "{req:?}");
    }
    assert_eq!(svc.stats().await.unwrap().total, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn export_is_sorted_anchors_and_ids_only() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let mut md5s: Vec<&str> = maps.iter().map(|m| m.md5.as_str()).collect();
    md5s.sort_unstable();
    svc.submit(LabelSubmitDto {
        unsure: true,
        mixed: true,
        ..submit(&anchor(md5s[1], 8_000, 12_000), &["regular.stream.roll"])
    })
    .await
    .unwrap();
    // Neither append order nor its reverse is the export order.
    svc.submit(submit(
        &anchor(md5s[0], 4_000, 8_000),
        &["regular.stream.trill", "regular.jack.anchor"],
    ))
    .await
    .unwrap();
    let undone = svc
        .submit(submit(
            &anchor(md5s[2], 0, 4_000),
            &["regular.stream.trill"],
        ))
        .await
        .unwrap();
    svc.submit(submit(
        &anchor(md5s[1], 0, 4_000),
        &["regular.stream.trill"],
    ))
    .await
    .unwrap();
    svc.undo(&undone.id).await.unwrap();

    let text = svc.export_jsonl().await.unwrap();
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3, "{text}");
    let keys: Vec<(String, i64)> = lines
        .iter()
        .map(|l| {
            (
                l["md5"].as_str().unwrap().to_owned(),
                l["t0_us"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        keys,
        [
            (md5s[0].to_owned(), 4_000_000),
            (md5s[1].to_owned(), 0),
            (md5s[1].to_owned(), 8_000_000)
        ]
    );
    assert_eq!(
        lines[0],
        serde_json::json!({
            "md5": md5s[0],
            "t0_us": 4_000_000,
            "t1_us": 8_000_000,
            "cols": ALL_COLS,
            "patterns": ["regular.jack.anchor", "regular.stream.trill"],
            "no_pattern": false,
            "flags": [],
            "thumb_pref": null,
            "labelled_at": "2026-09-28T23:13:56.636Z",
            "selection": {"pick": "sampled", "window": "sampled"},
        })
    );
    assert_eq!(lines[2]["flags"], serde_json::json!(["mixed", "unsure"]));
    for m in &maps {
        assert!(!text.contains(&m.title), "no titles: {text}");
    }
    assert!(
        !text.contains("TWulfZ") && !text.contains("wolluf -"),
        "{text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn export_to_writes_the_file_outside_the_osu_folder() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    svc.submit(submit(
        &anchor(&maps[0].md5, 0, 4_000),
        &["regular.stream.trill"],
    ))
    .await
    .unwrap();
    let out = f.dir.path().join("out/labels/gold.jsonl");
    let done = svc.export_to(out.clone()).await.unwrap();
    assert_eq!(done.rows, 1);
    assert_eq!(done.path, out.to_string_lossy());
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        svc.export_jsonl().await.unwrap()
    );
    let inside = f.root.join("labels.jsonl");
    let err = svc.export_to(inside.clone()).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(!inside.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_patterns_takes_keys_or_ids() {
    let (f, _) = library().await;
    let svc = f.ctx.labeling();
    let tokens = |t: &[&str]| t.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let got = svc
        .resolve_patterns(7, &tokens(&["CJ", "regular.stream.jumpstream", "cj", "a"]))
        .unwrap();
    let ids: Vec<&str> = got.iter().map(|p| p.as_str()).collect();
    assert_eq!(
        ids,
        [
            "regular.jack.chordjack",
            "regular.stream.jumpstream",
            "regular.jack.anchor"
        ],
        "input order, deduplicated"
    );
    let unknown = svc.resolve_patterns(7, &tokens(&["js", "zz"])).unwrap_err();
    assert_eq!(unknown.code, ErrorCode::InvalidInput);
    assert_eq!(unknown.message_key, super::super::keys::UNKNOWN_PATTERN);
    assert_eq!(unknown.args["pattern"], "zz");
    assert_eq!(
        svc.resolve_patterns(7, &[]).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        svc.resolve_patterns(4, &tokens(&["js"])).unwrap_err().code,
        ErrorCode::InvalidInput
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn reshape_clamps_to_the_chart_rows() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let md5 = &maps[0].md5;
    // Rows run from 0 to 19.875 s, so the chart span ends at 19.876 s.
    let at = |a: AnchorDto| (a.t0_ms, a.t1_ms);
    let reshape = |a: AnchorDto, op| svc.reshape(a, op);
    assert_eq!(
        at(reshape(anchor(md5, 1_000, 5_000), WindowOpDto::Prev)
            .await
            .unwrap()),
        (0, 4_000)
    );
    assert_eq!(
        at(reshape(anchor(md5, 15_000, 19_000), WindowOpDto::Next)
            .await
            .unwrap()),
        (15_876, 19_876)
    );
    assert_eq!(
        at(reshape(anchor(md5, 15_876, 19_876), WindowOpDto::Widen)
            .await
            .unwrap()),
        (14_876, 19_876),
        "at the end, widening grows backwards"
    );
    assert_eq!(
        at(reshape(anchor(md5, 0, 2_000), WindowOpDto::Narrow)
            .await
            .unwrap()),
        (0, 1_000)
    );
    let short = reshape(anchor(md5, 0, 1_000), WindowOpDto::Narrow)
        .await
        .unwrap_err();
    assert_eq!(short.code, ErrorCode::InvalidInput);
    assert_eq!(short.message_key, super::super::keys::WINDOW_TOO_SHORT);
    let unknown = reshape(anchor(&"0".repeat(32), 0, 4_000), WindowOpDto::Next).await;
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn submit_rejects_windows_outside_the_chart_and_overlaps() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let md5 = &maps[0].md5;
    for (t0, t1) in [(-500, 3_500), (17_000, 21_000)] {
        let err = svc
            .submit(submit(&anchor(md5, t0, t1), &["regular.stream.trill"]))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{t0}..{t1}");
    }
    let first = svc
        .submit(submit(
            &anchor(md5, 4_000, 8_000),
            &["regular.stream.trill"],
        ))
        .await
        .unwrap();
    let overlap = svc
        .submit(submit(
            &anchor(md5, 7_999, 11_999),
            &["regular.stream.roll"],
        ))
        .await
        .unwrap_err();
    assert_eq!(overlap.code, ErrorCode::Conflict);
    svc.submit(submit(
        &anchor(md5, 8_000, 12_000),
        &["regular.stream.roll"],
    ))
    .await
    .expect("touching windows do not overlap");
    svc.submit(submit(
        &anchor(&maps[1].md5, 4_000, 8_000),
        &["regular.stream.roll"],
    ))
    .await
    .expect("another chart");
    svc.undo(&first.id).await.unwrap();
    svc.submit(submit(&anchor(md5, 4_000, 8_000), &["regular.stream.roll"]))
        .await
        .expect("an undone label frees its window");
}

#[tokio::test(flavor = "multi_thread")]
async fn submit_and_undo_need_the_self_profile() {
    let f = Fixture::new(&wolluf_source_osu::testkit::FakeInstall::new()).await;
    let svc = f.ctx.labeling();
    let err = svc
        .submit(submit(
            &anchor(&"0".repeat(32), 0, 4_000),
            &["regular.stream.trill"],
        ))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(err.args["profile"], "self");
    let err = svc
        .undo(&ulid::Ulid::from_parts(1, 1).to_string())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(err.args["profile"], "self");
    assert_eq!(svc.stats().await.unwrap().total, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn another_profiles_label_is_neither_undone_nor_counted() {
    use wolluf_core::{ColMask, Keymode, PatternId, SegmentAnchor, TimeUs};
    use wolluf_store::repo::labels::{NewGoldLabel, append_gold_label};
    use wolluf_store::repo::players::{MergeMode, NewProfile, ProfileKind, profile};

    let (f, maps) = library().await;
    let md5: wolluf_core::ChartMd5 = maps[0].md5.parse().unwrap();
    let id = ulid::Ulid::from_parts(1, 1);
    f.ctx
        .user_db()
        .write(move |tx| {
            let other = profile::insert(
                tx,
                &NewProfile {
                    kind: ProfileKind::Other,
                    label: "Rival".into(),
                    is_default: false,
                    merge_mode: MergeMode::Merged,
                    created_at: wolluf_core::UnixUs(0),
                },
            )?;
            append_gold_label(
                tx,
                &NewGoldLabel {
                    id,
                    ts: wolluf_core::UnixUs(0),
                    profile_id: other,
                    keymode: Keymode::K7,
                    anchor: SegmentAnchor::new(
                        md5,
                        TimeUs::from_ms(0),
                        TimeUs::from_ms(4_000),
                        ColMask::full(Keymode::K7),
                        Keymode::K7,
                    )
                    .unwrap(),
                    answer: wolluf_store::repo::labels::GoldAnswer::Patterns(vec![
                        PatternId::from_static("regular.stream.trill"),
                    ]),
                    mixed: false,
                    unsure: false,
                    thumb_pref: None,
                    selection: wolluf_store::repo::labels::Selection {
                        pick: wolluf_store::repo::labels::ChartPick::Sampled,
                        window: wolluf_store::repo::labels::WindowPick::Sampled,
                    },
                    app_version: "test".into(),
                },
            )
        })
        .unwrap();
    let svc = f.ctx.labeling();
    assert_eq!(
        svc.undo(&id.to_string()).await.unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(svc.stats().await.unwrap().total, 0);
    assert!(svc.export_jsonl().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn played_means_played_by_the_self_profile() {
    use crate::features::library::testkit::install;
    use crate::features::plays::testkit::scores_db;
    use wolluf_source_osu::testkit::ScoreBuilder;

    let maps = maps();
    let (three, eight) = (&maps[0], &maps[1]);
    let inst = install(&maps, &[])
        .cfg("fixture", "Username = TWulfZ\r\n")
        .scores_db(scores_db(&[
            ScoreBuilder::mania(&three.md5, "TWulfZ", 1),
            ScoreBuilder::mania(&eight.md5, "Kovacs", 2),
        ]));
    let f = Fixture::new(&inst).await;
    f.sync().await;
    let svc = f.ctx.labeling();
    let window = |min: Option<f64>, max: Option<f64>| {
        svc.sample(SampleRequestDto {
            scale: Some("jinjin_dan_regular".into()),
            level_min: min,
            level_max: max,
            ..request("3", 0, vec![])
        })
    };
    let mine = window(None, Some(5.0)).await.unwrap().unwrap();
    assert_eq!(
        (mine.anchor.md5.as_str(), mine.played),
        (three.md5.as_str(), true)
    );
    let theirs = window(Some(5.0), None).await.unwrap().unwrap();
    assert_eq!(
        (theirs.anchor.md5.as_str(), theirs.played),
        (eight.md5.as_str(), false),
        "another player's play is not the user's"
    );
}

#[test]
fn export_target_resolves_relative_paths_and_guards_installs() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("work");
    let osu = dir.path().join("osu!");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&osu).unwrap();
    let roots = [osu.clone()];
    let canon = |p: &std::path::Path| std::fs::canonicalize(p).unwrap();

    let rel = export_target(
        std::path::Path::new("fixtures/labels/g.jsonl"),
        &cwd,
        &roots,
    )
    .unwrap();
    assert_eq!(rel, cwd.join("fixtures/labels/g.jsonl"));

    for bad in ["../osu!/x.jsonl", "a/../../osu!/x.jsonl", ".."] {
        let err = export_target(std::path::Path::new(bad), &cwd, &roots).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{bad}");
    }
    let inside_rel = export_target(std::path::Path::new("sub/new/x.jsonl"), &osu, &roots);
    assert_eq!(inside_rel.unwrap_err().code, ErrorCode::InvalidInput);
    let inside_abs = export_target(&canon(&osu).join("x.jsonl"), &cwd, &roots);
    assert_eq!(inside_abs.unwrap_err().code, ErrorCode::InvalidInput);
    assert!(
        !osu.join("sub").exists(),
        "nothing created inside the install"
    );

    #[cfg(unix)]
    {
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&osu, &link).unwrap();
        let via_link = export_target(&link.join("new/x.jsonl"), &cwd, &roots);
        assert_eq!(via_link.unwrap_err().code, ErrorCode::InvalidInput);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn export_to_rejects_parent_components() {
    let (f, _) = library().await;
    let err = f
        .ctx
        .labeling()
        .export_to(f.dir.path().join("out/../x.jsonl"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(!f.dir.path().join("x.jsonl").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn no_pattern_answers_and_thumb_sides_are_stored_counted_and_exported() {
    use crate::features::labeling::dto::ThumbPrefDto;

    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let md5 = &maps[0].md5;
    svc.submit(LabelSubmitDto {
        no_pattern: true,
        thumb_pref: Some(ThumbPrefDto::Left),
        ..submit(&anchor(md5, 0, 4_000), &[])
    })
    .await
    .unwrap();
    svc.submit(LabelSubmitDto {
        thumb_pref: Some(ThumbPrefDto::Right),
        ..submit(&anchor(md5, 4_000, 8_000), &["regular.tech.thumb"])
    })
    .await
    .unwrap();
    for bad in [
        LabelSubmitDto {
            no_pattern: true,
            ..submit(&anchor(md5, 8_000, 12_000), &["regular.stream.trill"])
        },
        submit(&anchor(md5, 8_000, 12_000), &[]),
    ] {
        assert_eq!(
            svc.submit(bad.clone()).await.unwrap_err().code,
            ErrorCode::InvalidInput,
            "{bad:?}"
        );
    }

    let stats = svc.stats().await.unwrap();
    assert_eq!(
        (
            stats.total,
            stats.no_pattern,
            stats.thumb_left,
            stats.thumb_right
        ),
        (2, 1, 1, 1)
    );
    assert_eq!(stats.per_pattern.len(), 1, "{stats:?}");
    assert_eq!(stats.per_stratum.iter().map(|c| c.count).sum::<u32>(), 2);

    let text = svc.export_jsonl().await.unwrap();
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["patterns"], serde_json::json!([]));
    assert_eq!(lines[0]["no_pattern"], true);
    assert_eq!(lines[0]["thumb_pref"], "left");
    assert_eq!(lines[1]["no_pattern"], false);
    assert_eq!(lines[1]["thumb_pref"], "right");
}

/// A `segment_label` exactly as written before ADR 0021, under the self profile.
async fn store_v1_label(f: &Fixture, md5: &str, t0_ms: i64, t1_ms: i64) {
    use wolluf_store::repo::labels::SEGMENT_LABEL_KIND;
    use wolluf_store::repo::ledger::NewFeedbackEvent;

    let me = f.ctx.players().self_profile_id().await.unwrap().unwrap();
    let md5 = md5.to_owned();
    f.ctx
        .user_db()
        .write(move |tx| {
            feedback_event::append(
                tx,
                &NewFeedbackEvent {
                    id: ulid::Ulid::from_parts(1, 1),
                    ts: wolluf_core::UnixUs(0),
                    profile_id: Some(me),
                    kind: SEGMENT_LABEL_KIND.to_owned(),
                    subject: serde_json::json!({"anchor": {
                        "chart_md5": md5, "t0_us": t0_ms * 1_000, "t1_us": t1_ms * 1_000,
                        "cols": [0, 1, 2, 3, 4, 5, 6], "keymode": 7,
                    }}),
                    payload: serde_json::json!({
                        "v": 1, "action": "assert_set", "origin": "gold",
                        "patterns": ["regular.stream.trill"],
                        "flags": {"mixed": false, "unsure": false},
                    }),
                    context: serde_json::json!({"app_version": "0.1.0"}),
                },
            )
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn submit_records_the_selection_and_stats_and_export_report_it() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let (three, eight, plain) = (&maps[0].md5, &maps[1].md5, &maps[2].md5);
    let random = picked(ChartPickDto::Random, WindowPickDto::Sampled);
    let moved = picked(ChartPickDto::NowPlaying, WindowPickDto::Moved);
    svc.submit(submit(&anchor(three, 0, 4_000), &["regular.stream.trill"]))
        .await
        .unwrap();
    svc.submit(LabelSubmitDto {
        selection: random,
        ..submit(&anchor(eight, 0, 4_000), &["regular.stream.trill"])
    })
    .await
    .unwrap();
    svc.submit(LabelSubmitDto {
        selection: moved,
        ..submit(&anchor(plain, 0, 4_000), &["regular.stream.trill"])
    })
    .await
    .unwrap();
    store_v1_label(&f, three, 8_000, 12_000).await;

    let stats = svc.stats().await.unwrap();
    assert_eq!((stats.total, stats.blind), (4, 2), "{stats:?}");
    let count = |selection, count| SelectionCountDto { selection, count };
    assert_eq!(
        stats.per_selection,
        [
            count(None, 1),
            count(
                Some(picked(ChartPickDto::Sampled, WindowPickDto::Sampled)),
                1
            ),
            count(Some(random), 1),
            count(Some(moved), 1),
        ]
    );

    let text = svc.export_jsonl().await.unwrap();
    let selection_of = |md5: &str, t0_us: i64| {
        text.lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|r| r["md5"] == md5 && r["t0_us"] == t0_us)
            .unwrap()["selection"]
            .clone()
    };
    assert_eq!(
        selection_of(three, 0),
        serde_json::json!({"pick": "sampled", "window": "sampled"})
    );
    assert_eq!(
        selection_of(plain, 0),
        serde_json::json!({"pick": "now_playing", "window": "moved"})
    );
    assert_eq!(
        selection_of(three, 8_000_000),
        serde_json::json!({"pick": "unknown", "window": "unknown"}),
        "a label stored before ADR 0021 exports with an unknown selection"
    );
}

/// ADR 0021: the service checks what it can know; a session pick names a chart the user played.
#[tokio::test(flavor = "multi_thread")]
async fn a_session_pick_needs_a_chart_the_self_profile_played() {
    use crate::features::plays::testkit::scores_db;
    use wolluf_source_osu::testkit::ScoreBuilder;

    let maps = maps();
    let (three, eight) = (&maps[0], &maps[1]);
    let inst = install(&maps, &[])
        .cfg("fixture", "Username = TWulfZ\r\n")
        .scores_db(scores_db(&[
            ScoreBuilder::mania(&three.md5, "TWulfZ", 1),
            ScoreBuilder::mania(&eight.md5, "Kovacs", 2),
        ]));
    let f = Fixture::new(&inst).await;
    f.sync().await;
    let svc = f.ctx.labeling();
    let session = |md5: &str| LabelSubmitDto {
        selection: picked(ChartPickDto::Session, WindowPickDto::Moved),
        ..submit(&anchor(md5, 0, 4_000), &["regular.stream.trill"])
    };
    let err = svc.submit(session(&eight.md5)).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput, "{err:?}");
    assert_eq!(err.args.get("pick").map(String::as_str), Some("session"));
    svc.submit(session(&three.md5)).await.unwrap();
    let stats = svc.stats().await.unwrap();
    assert_eq!((stats.total, stats.blind), (1, 0));
}

/// The gold protocol stays blind: name hints never reach the sampler or the window it shows.
#[test]
fn chart_facts_drop_name_hints() {
    use crate::features::library::dto::{ChartLabelDto, LibraryChartDto};
    let label = |source: &str, scale: &str, text: &str| ChartLabelDto {
        source: source.to_owned(),
        scale: scale.to_owned(),
        level_ord: None,
        level_text: text.to_owned(),
        skill_tag: None,
        is_variant: false,
    };
    let chart = LibraryChartDto {
        md5: "1".repeat(32),
        title: String::new(),
        artist: String::new(),
        version: String::new(),
        creator: String::new(),
        keymode: 7,
        n_notes: 1,
        n_ln: 0,
        ln_ratio: 0.0,
        length_ms: 1,
        nps: 1.0,
        stars: None,
        labels: vec![
            label("name_hint", "hint_axis", "7k.regular.jack"),
            label("road_to_gamma", "jinjin_dan", "gamma_entry"),
            label("name_hint", "hint_pattern", "regular.jack.minijack"),
        ],
    };
    let facts = chart_facts(&[chart], &BTreeSet::new());
    let scales: Vec<&str> = facts[0].labels.iter().map(|l| l.scale.as_str()).collect();
    assert_eq!(scales, ["jinjin_dan"]);
}

fn at(md5: &str, seed: &str, exclude: Vec<AnchorDto>) -> WindowAtRequestDto {
    WindowAtRequestDto {
        keymode: 7,
        md5: md5.to_owned(),
        seed: seed.to_owned(),
        window_ms: None,
        exclude,
    }
}

fn random(seed: &str, round: u32, exclude: Vec<AnchorDto>) -> RandomRequestDto {
    RandomRequestDto {
        keymode: 7,
        seed: seed.to_owned(),
        round,
        window_ms: None,
        exclude,
    }
}

/// Rows run from 0 to 19.875 s, so a window must end by 19.876 s.
fn inside_long_map(a: &AnchorDto) -> bool {
    0 <= a.t0_ms && a.t1_ms <= 19_876
}

#[tokio::test(flavor = "multi_thread")]
async fn window_at_picks_a_free_window_inside_the_given_chart() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let three = &maps[0];
    let w = svc.window_at(at(&three.md5, "5", vec![])).await.unwrap();
    assert_eq!(
        svc.window_at(at(&three.md5, "5", vec![])).await.unwrap(),
        w,
        "deterministic for the seed"
    );
    assert_eq!(w.anchor.md5, three.md5);
    assert_eq!(w.anchor.t1_ms - w.anchor.t0_ms, 4_000);
    assert_eq!(w.anchor.cols, ALL_COLS);
    assert!(inside_long_map(&w.anchor), "{w:?}");
    assert_eq!(
        (w.title.as_str(), w.version.as_str()),
        (three.title.as_str(), three.version.as_str())
    );
    assert_eq!(w.level.as_deref(), Some("jinjin_dan_regular:3rd"));
    assert!(w.stratum.starts_with("dan_03/nps_"), "{w:?}");

    svc.submit(submit(&w.anchor, &["regular.stream.single"]))
        .await
        .unwrap();
    let mut seen = Vec::new();
    for seed in 0..3 {
        let o = svc
            .window_at(at(&three.md5, &seed.to_string(), seen.clone()))
            .await
            .unwrap();
        assert_eq!(o.anchor.md5, three.md5);
        assert!(!overlaps(&o.anchor, &w.anchor), "labelled: {o:?}");
        assert!(!seen.iter().any(|s| overlaps(s, &o.anchor)), "{o:?}");
        seen.push(o.anchor);
    }
    let short = svc
        .window_at(WindowAtRequestDto {
            window_ms: Some(2_000),
            ..at(&three.md5, "5", vec![])
        })
        .await
        .unwrap();
    assert_eq!(short.anchor.t1_ms - short.anchor.t0_ms, 2_000);
}

#[tokio::test(flavor = "multi_thread")]
async fn window_at_rejects_unknown_unparsed_and_full_charts() {
    let maps = vec![long_map("here"), long_map("gone").missing()];
    let (f, _) = synced(&maps, &[]).await;
    let svc = f.ctx.labeling();
    let here = &maps[0].md5;
    let cases = [
        (at("nope", "1", vec![]), ErrorCode::InvalidInput),
        (at(&"0".repeat(32), "1", vec![]), ErrorCode::NotFound),
        (at(&maps[1].md5, "1", vec![]), ErrorCode::NotFound),
        (at(here, "-1", vec![]), ErrorCode::InvalidInput),
        (
            WindowAtRequestDto {
                keymode: 4,
                ..at(here, "1", vec![])
            },
            ErrorCode::InvalidInput,
        ),
        (
            WindowAtRequestDto {
                window_ms: Some(0),
                ..at(here, "1", vec![])
            },
            ErrorCode::InvalidInput,
        ),
    ];
    for (req, code) in cases {
        let err = svc.window_at(req.clone()).await.unwrap_err();
        assert_eq!(err.code, code, "{req:?}");
    }
    let whole = |exclude| WindowAtRequestDto {
        window_ms: Some(19_000),
        ..at(here, "1", exclude)
    };
    let first = svc.window_at(whole(vec![])).await.unwrap();
    let full = svc.window_at(whole(vec![first.anchor])).await.unwrap_err();
    assert_eq!(full.code, ErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn random_takes_any_chart_deterministically() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let first = svc.random(random("9", 0, vec![])).await.unwrap().unwrap();
    assert_eq!(
        svc.random(random("9", 0, vec![])).await.unwrap(),
        Some(first.clone())
    );
    assert!(maps.iter().any(|m| m.md5 == first.anchor.md5));
    assert_eq!(first.anchor.t1_ms - first.anchor.t0_ms, 4_000);
    assert!(first.stratum.contains("/nps_"), "{first:?}");

    let mut seen: Vec<AnchorDto> = Vec::new();
    let mut charts = BTreeSet::new();
    for round in 0..6 {
        let w = svc
            .random(random("9", round, seen.clone()))
            .await
            .unwrap()
            .unwrap();
        assert!(inside_long_map(&w.anchor), "{w:?}");
        assert!(!seen.iter().any(|s| overlaps(s, &w.anchor)), "{w:?}");
        charts.insert(w.anchor.md5.clone());
        seen.push(w.anchor);
    }
    assert!(charts.len() >= 2, "rounds move between charts: {charts:?}");

    assert_eq!(
        svc.random(random("x", 0, vec![])).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        svc.random(RandomRequestDto {
            keymode: 4,
            ..random("1", 0, vec![])
        })
        .await
        .unwrap_err()
        .code,
        ErrorCode::InvalidInput
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn random_is_none_once_every_window_is_taken() {
    let (f, _) = synced(&[long_map("solo")], &[]).await;
    let svc = f.ctx.labeling();
    let whole = |round, exclude| RandomRequestDto {
        window_ms: Some(19_000),
        ..random("4", round, exclude)
    };
    let first = svc.random(whole(0, vec![])).await.unwrap().unwrap();
    assert_eq!(
        svc.random(whole(1, vec![first.anchor.clone()]))
            .await
            .unwrap(),
        None
    );
    svc.submit(submit(&first.anchor, &["regular.stream.single"]))
        .await
        .unwrap();
    assert_eq!(
        svc.random(whole(0, vec![])).await.unwrap(),
        None,
        "labelled windows are avoided without being passed in"
    );
}

fn playing(title: &str) -> std::sync::Arc<wolluf_source_osu::testkit::FakeProbe> {
    use wolluf_source_osu::probe::ProbeResult;
    std::sync::Arc::new(
        wolluf_source_osu::testkit::FakeProbe::new(ProbeResult::Running { pids: vec![1] })
            .with_title(title),
    )
}

fn np(exclude: Vec<AnchorDto>) -> NowPlayingRequestDto {
    NowPlayingRequestDto {
        keymode: 7,
        exclude,
    }
}

fn now_playing<'a>(f: &'a Fixture, title: &str) -> LabelingService<'a> {
    LabelingService::with_probe(&f.ctx, playing(title))
}

/// A `Data/r` replay of `md5` by `player`; `nth` orders replays in time.
fn replay(
    inst: wolluf_source_osu::testkit::FakeInstall,
    md5: &str,
    player: &str,
    nth: i64,
) -> wolluf_source_osu::testkit::FakeInstall {
    let osr = wolluf_source_osu::testkit::OsrBuilder::new(
        wolluf_source_osu::testkit::ScoreBuilder::mania(md5, player, nth),
    );
    let name = osr.file_name().unwrap();
    inst.replay(&name, osr.build())
}

/// The osu!.db artist the library testkit gives a chart.
fn artist(map: &Map) -> String {
    format!("artist-{}", map.md5)
}

#[tokio::test(flavor = "multi_thread")]
async fn now_playing_resolves_the_osu_window_title() {
    let (f, maps) = library().await;
    let eight = &maps[1];
    let artist = artist(eight);
    for title in [
        format!("osu!  - {artist} - eight [8th Dan]"),
        format!("osu! - {artist}  -  eight [8th Dan]"),
        format!("osu!  - {} - Eight [8th dan]", artist.to_uppercase()),
    ] {
        let got = now_playing(&f, &title)
            .now_playing(np(vec![]))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.source, NowPlayingSourceDto::OsuWindow, "{title}");
        assert_eq!(got.window.anchor.md5, eight.md5, "{title}");
        assert_eq!(got.window.anchor.t1_ms - got.window.anchor.t0_ms, 4_000);
        assert_eq!(got.window.version, "8th Dan");
    }
    for title in [
        "osu!".to_owned(),
        format!("osu!  - {artist} - nothing [8th Dan]"),
        String::new(),
    ] {
        assert_eq!(
            now_playing(&f, &title)
                .now_playing(np(vec![]))
                .await
                .unwrap(),
            None,
            "{title:?}"
        );
    }
    let title = format!("osu!  - {artist} - eight [8th Dan]");
    assert_eq!(
        now_playing(&f, &title)
            .now_playing(NowPlayingRequestDto {
                keymode: 4,
                ..np(vec![])
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}

/// Two charts with the same artist, title and difficulty name, as duplicated sets have.
fn twins() -> (wolluf_source_osu::testkit::FakeInstall, [String; 2]) {
    use wolluf_source_osu::codec::OsuString;
    use wolluf_source_osu::testkit::{BeatmapBuilder, FakeInstall, OsuDbBuilder};

    let mut db = OsuDbBuilder::new();
    let mut inst = FakeInstall::new().cfg("fixture", "Username = TWulfZ\r\n");
    let mut md5s = Vec::new();
    for (i, which) in ["twin a", "twin b"].into_iter().enumerate() {
        let map = long_map(which);
        let mut b = BeatmapBuilder::mania(&map.md5, 7)
            .folder(&map.folder)
            .osu_file(&map.file)
            .title("twin")
            .ids(1, 100 + i as i32)
            .build();
        b.artist = OsuString::present(*b"Twin Artist");
        b.difficulty = OsuString::present(*b"Normal");
        db = db.beatmap(b);
        inst = inst.song(map.rel_path(), map.bytes.clone().unwrap());
        md5s.push(map.md5);
    }
    let md5s = [md5s[0].clone(), md5s[1].clone()];
    (inst.osu_db(db.encode()), md5s)
}

const TWIN_TITLE: &str = "osu!  - Twin Artist - twin [Normal]";

#[tokio::test(flavor = "multi_thread")]
async fn now_playing_breaks_title_ties_by_the_newest_self_replay() {
    let (inst, [a, b]) = twins();
    let inst = replay(replay(inst, &b, "TWulfZ", 1), &a, "Kovacs", 2);
    let f = Fixture::new(&inst).await;
    f.sync().await;
    let got = now_playing(&f, TWIN_TITLE)
        .now_playing(np(vec![]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (got.window.anchor.md5.as_str(), got.source),
        (b.as_str(), NowPlayingSourceDto::OsuWindow)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn now_playing_never_uses_another_players_replay() {
    let (inst, [a, b]) = twins();
    let inst = replay(replay(inst, &a, "Kovacs", 1), &b, "Kovacs", 2);
    let f = Fixture::new(&inst).await;
    f.sync().await;
    for title in [TWIN_TITLE, "osu!"] {
        assert_eq!(
            now_playing(&f, title)
                .now_playing(np(vec![]))
                .await
                .unwrap(),
            None,
            "an ambiguous title and someone else's replays: {title}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn now_playing_falls_back_to_the_newest_self_replay_of_the_keymode() {
    let mut maps = maps();
    let four = Map::new("four", 4, osu_text(4, "four", &[(0, 0), (1, 125)], &[]));
    maps.push(four.clone());
    let (three, eight, plain) = (&maps[0].md5, &maps[1].md5, &maps[2].md5);
    let unknown = "f".repeat(32);
    let mut inst = install(&maps, &[]).cfg("fixture", "Username = TWulfZ\r\n");
    for (md5, player, nth) in [
        (three.as_str(), "TWulfZ", 1),
        (eight.as_str(), "TWulfZ", 3),
        (plain.as_str(), "Kovacs", 4),
        (four.md5.as_str(), "TWulfZ", 5),
        (unknown.as_str(), "TWulfZ", 6),
    ] {
        inst = replay(inst, md5, player, nth);
    }
    let f = Fixture::new(&inst).await;
    f.sync().await;
    let got = now_playing(&f, "osu!")
        .now_playing(np(vec![]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (got.window.anchor.md5.as_str(), got.source),
        (eight.as_str(), NowPlayingSourceDto::LastReplay)
    );

    // Read live from Data/r: a replay saved after the last sync counts.
    let newer = wolluf_source_osu::testkit::OsrBuilder::new(
        wolluf_source_osu::testkit::ScoreBuilder::mania(plain, "TWulfZ", 7),
    );
    let name = newer.file_name().unwrap();
    f.write(
        std::path::Path::new("Data/r").join(name.format()),
        &newer.build(),
    );
    let got = now_playing(&f, "osu!")
        .now_playing(np(vec![]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.window.anchor.md5, *plain);
}

#[tokio::test(flavor = "multi_thread")]
async fn now_playing_moves_past_the_windows_already_shown() {
    let (f, maps) = library().await;
    let eight = &maps[1];
    let title = format!("osu!  - {} - eight [8th Dan]", artist(eight));
    let svc = now_playing(&f, &title);
    let first = svc.now_playing(np(vec![])).await.unwrap().unwrap();
    // Skip keeps the window in the session's shown list, so the next press must not repeat it.
    let second = svc
        .now_playing(np(vec![first.window.anchor.clone()]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.source, NowPlayingSourceDto::OsuWindow);
    assert_eq!(second.window.anchor.md5, eight.md5);
    assert!(
        !overlaps(&first.window.anchor, &second.window.anchor),
        "{first:?} {second:?}"
    );
    assert_eq!(
        svc.now_playing(np(vec![first.window.anchor.clone()]))
            .await
            .unwrap()
            .unwrap(),
        second,
        "deterministic for the shown windows"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn now_playing_falls_back_to_the_last_replay_once_the_title_chart_is_full() {
    let maps = maps();
    let (three, eight) = (&maps[0].md5, &maps[1].md5);
    let mut inst = install(&maps, &[]).cfg("fixture", "Username = TWulfZ\r\n");
    // The title chart is also the newest replay: it cannot stand in for itself.
    for (md5, nth) in [(three.as_str(), 1), (eight.as_str(), 2)] {
        inst = replay(inst, md5, "TWulfZ", nth);
    }
    let f = Fixture::new(&inst).await;
    f.sync().await;
    let title = format!("osu!  - {} - eight [8th Dan]", artist(&maps[1]));
    let whole_eight = anchor(eight, 0, 19_876);
    let got = now_playing(&f, &title)
        .now_playing(np(vec![whole_eight]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (got.window.anchor.md5.as_str(), got.source),
        (three.as_str(), NowPlayingSourceDto::LastReplay)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn now_playing_ignores_replays_without_a_self_identity() {
    let maps = maps();
    let mut inst = install(&maps, &[]);
    for (nth, map) in (1..).zip(&maps) {
        inst = replay(inst, &map.md5, "TWulfZ", nth);
    }
    let f = Fixture::new(&inst).await;
    assert_eq!(f.ctx.players().self_profile_id().await.unwrap(), None);
    assert_eq!(
        now_playing(&f, "osu!")
            .now_playing(np(vec![]))
            .await
            .unwrap(),
        None,
        "before the first sync there is no self profile"
    );
    // Sync creates the self profile, but without a cfg login no alias joins it.
    f.sync().await;
    assert_eq!(f.ctx.players().self_alias_ids().await.unwrap(), []);
    assert_eq!(
        now_playing(&f, "osu!")
            .now_playing(np(vec![]))
            .await
            .unwrap(),
        None,
        "ADR 0005: another name's replays never stand for the user"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn windows_carry_the_creator_and_stables_star_rating() {
    let rated = long_map("rated").rated(4.5);
    let unrated = long_map("unrated");
    let (f, _) = synced(&[rated.clone(), unrated.clone()], &[]).await;
    let svc = f.ctx.labeling();
    let w = svc.window_at(at(&rated.md5, "1", vec![])).await.unwrap();
    assert_eq!((w.creator.as_str(), w.stars), ("wolluf", Some(4.5)));
    let w = svc.window_at(at(&unrated.md5, "1", vec![])).await.unwrap();
    assert_eq!(w.stars, None);
    let listed = f.ctx.library().overview(Keymode::K7).await.unwrap();
    let stars: BTreeSet<String> = listed
        .iter()
        .map(|c| format!("{}={:?}", c.title, c.stars))
        .collect();
    assert_eq!(
        stars,
        ["rated=Some(4.5)".to_owned(), "unrated=None".to_owned()].into()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn move_window_keeps_its_length_inside_the_chart() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let md5 = &maps[0].md5;
    let moved =
        |anchor: AnchorDto, t0_ms: i32| svc.move_window(MoveWindowRequestDto { anchor, t0_ms });
    let at_ms = |a: AnchorDto| (a.t0_ms, a.t1_ms);
    assert_eq!(
        at_ms(moved(anchor(md5, 1_000, 5_000), 10_000).await.unwrap()),
        (10_000, 14_000)
    );
    assert_eq!(
        at_ms(moved(anchor(md5, 1_000, 5_000), -500).await.unwrap()),
        (0, 4_000)
    );
    assert_eq!(
        at_ms(moved(anchor(md5, 1_000, 5_000), 18_000).await.unwrap()),
        (15_876, 19_876),
        "the end stops one past the last row"
    );
    assert_eq!(
        at_ms(moved(anchor(md5, 0, 30_000), 2_000).await.unwrap()),
        (0, 19_876),
        "a window longer than the chart becomes the chart"
    );
    let narrow = AnchorDto {
        cols: vec![1, 2],
        ..anchor(md5, 1_000, 5_000)
    };
    let recomputed = moved(narrow, 2_000).await.unwrap();
    assert_eq!(
        recomputed,
        AnchorDto {
            md5: md5.clone(),
            t0_ms: 2_000,
            t1_ms: 6_000,
            cols: ALL_COLS.to_vec(),
        }
    );
    let unknown = moved(anchor(&"0".repeat(32), 0, 4_000), 0).await;
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
    let bad = moved(anchor("nope", 0, 4_000), 0).await;
    assert_eq!(bad.unwrap_err().code, ErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn resize_window_clamps_to_the_chart_and_the_length_bounds() {
    let taps: Vec<(u8, i32)> = (0..560).map(|i| ((i % 7) as u8, i * 125)).collect();
    let seventy = Map::new("seventy", 7, osu_text(7, "seventy", &taps, &[]));
    let (f, _) = synced(std::slice::from_ref(&seventy), &[]).await;
    let svc = f.ctx.labeling();
    let md5 = &seventy.md5;
    let resized = |anchor: AnchorDto, t0_ms: i32, t1_ms: i32| {
        svc.resize_window(ResizeWindowRequestDto {
            anchor,
            t0_ms,
            t1_ms,
        })
    };
    let at_ms = |a: AnchorDto| (a.t0_ms, a.t1_ms);
    assert_eq!(
        at_ms(
            resized(anchor(md5, 1_000, 5_000), 1_000, 35_000)
                .await
                .unwrap()
        ),
        (1_000, 35_000)
    );
    assert_eq!(
        at_ms(
            resized(anchor(md5, 1_000, 5_000), 1_000, 69_000)
                .await
                .unwrap()
        ),
        (1_000, 61_000),
        "the end stops at the 60 s cap"
    );
    assert_eq!(
        at_ms(
            resized(anchor(md5, 30_000, 34_000), 30_200, 34_000)
                .await
                .unwrap()
        ),
        (30_200, 34_000)
    );
    assert_eq!(
        at_ms(
            resized(anchor(md5, 30_000, 34_000), 33_800, 34_000)
                .await
                .unwrap()
        ),
        (33_000, 34_000),
        "the start stops at the 1 s floor"
    );
    assert_eq!(
        at_ms(
            resized(anchor(md5, 65_000, 69_000), 65_000, 90_000)
                .await
                .unwrap()
        ),
        (65_000, 69_876),
        "the end stops one past the last row"
    );
    assert_eq!(
        at_ms(
            resized(anchor(md5, 2_000, 6_000), -3_000, 6_000)
                .await
                .unwrap()
        ),
        (0, 6_000)
    );
    let narrow = AnchorDto {
        cols: vec![1, 2],
        ..anchor(md5, 1_000, 5_000)
    };
    assert_eq!(
        resized(narrow, 1_000, 8_000).await.unwrap().cols,
        ALL_COLS.to_vec(),
        "columns are recomputed"
    );
    let unknown = resized(anchor(&"0".repeat(32), 0, 4_000), 0, 5_000).await;
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
    let bad = resized(anchor("nope", 0, 4_000), 0, 5_000).await;
    assert_eq!(bad.unwrap_err().code, ErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn widen_stops_at_the_longest_window() {
    let taps: Vec<(u8, i32)> = (0..560).map(|i| ((i % 7) as u8, i * 125)).collect();
    let seventy = Map::new("seventy", 7, osu_text(7, "seventy", &taps, &[]));
    let (f, _) = synced(std::slice::from_ref(&seventy), &[]).await;
    let widened = f
        .ctx
        .labeling()
        .reshape(anchor(&seventy.md5, 0, 59_500), WindowOpDto::Widen)
        .await
        .unwrap();
    assert_eq!((widened.t0_ms, widened.t1_ms), (0, 60_000));
}

fn timeline(md5: &str, buckets: u16) -> ChartTimelineRequestDto {
    ChartTimelineRequestDto {
        keymode: 7,
        md5: md5.to_owned(),
        buckets,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn chart_timeline_counts_notes_and_lists_own_labels() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let md5 = &maps[0].md5;
    let empty = svc.chart_timeline(timeline(md5, 4)).await.unwrap();
    assert_eq!(
        empty,
        ChartTimelineDto {
            first_ms: 0,
            end_ms: 19_876,
            density: vec![40, 40, 40, 40],
            labelled: vec![],
        }
    );

    let later = svc
        .submit(submit(
            &anchor(md5, 8_000, 12_000),
            &["regular.stream.roll"],
        ))
        .await
        .unwrap();
    svc.submit(submit(
        &anchor(md5, 1_000, 5_000),
        &["regular.stream.trill"],
    ))
    .await
    .unwrap();
    let undone = svc
        .submit(submit(
            &anchor(md5, 14_000, 18_000),
            &["regular.stream.roll"],
        ))
        .await
        .unwrap();
    svc.undo(&undone.id).await.unwrap();
    svc.submit(submit(
        &anchor(&maps[1].md5, 0, 4_000),
        &["regular.stream.roll"],
    ))
    .await
    .unwrap();
    let _ = later;
    let full = svc.chart_timeline(timeline(md5, 1)).await.unwrap();
    assert_eq!(full.density, vec![160]);
    assert_eq!(
        full.labelled,
        vec![
            SpanDto {
                t0_ms: 1_000,
                t1_ms: 5_000
            },
            SpanDto {
                t0_ms: 8_000,
                t1_ms: 12_000
            },
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn chart_timeline_rejects_bad_requests() {
    let lns = Map::new(
        "lns",
        7,
        osu_text(
            7,
            "lns",
            &[(0, 1_000)],
            &[(3, 1_000, 3_000), (5, 2_000, 2_500)],
        ),
    );
    let (f, _) = synced(std::slice::from_ref(&lns), &[]).await;
    let svc = f.ctx.labeling();
    let t = svc.chart_timeline(timeline(&lns.md5, 2)).await.unwrap();
    assert_eq!((t.first_ms, t.end_ms), (1_000, 3_001));
    assert_eq!(t.density, vec![3, 0], "taps and heads count, tails do not");
    let code = |e: Result<ChartTimelineDto, AppError>| e.unwrap_err().code;
    assert_eq!(
        code(svc.chart_timeline(timeline(&lns.md5, 0)).await),
        ErrorCode::InvalidInput
    );
    assert_eq!(
        code(svc.chart_timeline(timeline(&lns.md5, u16::MAX)).await),
        ErrorCode::InvalidInput
    );
    assert_eq!(
        code(svc.chart_timeline(timeline(&"0".repeat(32), 4)).await),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(
            svc.chart_timeline(ChartTimelineRequestDto {
                keymode: 4,
                ..timeline(&lns.md5, 4)
            })
            .await
        ),
        ErrorCode::InvalidInput
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pattern_examples_draw_with_the_preferred_hand_layout() {
    use crate::features::library::dto::HandDto;
    let (f, _) = library().await;
    f.ctx
        .settings()
        .set_hand_layout(7, "k7.313_left_thumb")
        .await
        .unwrap();
    let examples = f.ctx.labeling().pattern_examples(7, None).await.unwrap();
    assert!(!examples.is_empty());
    for ex in &examples {
        assert_eq!(ex.window.layout.id, "k7.313_left_thumb", "{}", ex.id);
        assert_eq!(ex.window.layout.columns[3].hand, HandDto::Left, "{}", ex.id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn pattern_examples_honour_an_explicit_layout_over_the_preference() {
    let (f, _) = library().await;
    f.ctx
        .settings()
        .set_hand_layout(7, "k7.313_left_thumb")
        .await
        .unwrap();
    let svc = f.ctx.labeling();
    let examples = svc
        .pattern_examples(7, Some("k7.313_right_thumb"))
        .await
        .unwrap();
    assert!(
        examples
            .iter()
            .all(|e| e.window.layout.id == "k7.313_right_thumb")
    );
    for bad in ["nope", "k4.generic", ""] {
        let err = svc.pattern_examples(7, Some(bad)).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{bad:?}");
    }
}

/// `T0` of the fixture clock in the export file stamp.
const T0_STAMP: &str = "20260928T231356Z";

#[tokio::test(flavor = "multi_thread")]
async fn label_export_writes_a_stamped_file_under_exports() {
    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    let md5 = &maps[0].md5;
    svc.submit(submit(&anchor(md5, 0, 4_000), &["regular.stream.trill"]))
        .await
        .unwrap();
    svc.submit(LabelSubmitDto {
        selection: picked(ChartPickDto::NowPlaying, WindowPickDto::Moved),
        ..submit(&anchor(md5, 4_000, 8_000), &["regular.stream.trill"])
    })
    .await
    .unwrap();
    store_v1_label(&f, md5, 8_000, 12_000).await;

    let out = svc.export_to_data_dir(7).await.unwrap();

    let expected = f
        .dir
        .path()
        .join("data")
        .join("exports")
        .join(format!("gold-7k-{T0_STAMP}.jsonl"));
    assert_eq!(std::path::PathBuf::from(&out.path), expected);
    assert_eq!(out.rows, 3);
    // The screen and `wolluf label export` share one exporter: same bytes, ADR 0021 fields included.
    let cli = f.dir.path().join("cli").join("gold-7k.jsonl");
    svc.export_to(cli.clone()).await.unwrap();
    let text = std::fs::read_to_string(&expected).unwrap();
    assert_eq!(text, std::fs::read_to_string(&cli).unwrap());
    let selections: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["selection"].clone())
        .collect();
    assert_eq!(
        selections,
        [
            serde_json::json!({"pick": "sampled", "window": "sampled"}),
            serde_json::json!({"pick": "now_playing", "window": "moved"}),
            serde_json::json!({"pick": "unknown", "window": "unknown"}),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn label_export_with_no_labels_still_writes_an_empty_file() {
    let (f, _) = library().await;
    let out = f.ctx.labeling().export_to_data_dir(7).await.unwrap();
    assert_eq!(out.rows, 0);
    assert_eq!(std::fs::read_to_string(&out.path).unwrap(), "");
}

#[tokio::test(flavor = "multi_thread")]
async fn label_export_keeps_only_the_requested_keymode() {
    use wolluf_core::{ColMask, Keymode, PatternId, SegmentAnchor, TimeUs};
    use wolluf_store::repo::labels::{
        ChartPick, GoldAnswer, NewGoldLabel, Selection, WindowPick, append_gold_label,
    };

    let (f, maps) = library().await;
    let svc = f.ctx.labeling();
    svc.submit(submit(
        &anchor(&maps[0].md5, 0, 4_000),
        &["regular.stream.trill"],
    ))
    .await
    .unwrap();
    let me = f.ctx.players().self_profile_id().await.unwrap().unwrap();
    let md5: wolluf_core::ChartMd5 = maps[1].md5.parse().unwrap();
    f.ctx
        .user_db()
        .write(move |tx| {
            append_gold_label(
                tx,
                &NewGoldLabel {
                    id: ulid::Ulid::from_parts(1, 1),
                    ts: wolluf_core::UnixUs(0),
                    profile_id: me,
                    keymode: Keymode::K4,
                    anchor: SegmentAnchor::new(
                        md5,
                        TimeUs::from_ms(0),
                        TimeUs::from_ms(4_000),
                        ColMask::full(Keymode::K4),
                        Keymode::K4,
                    )
                    .unwrap(),
                    answer: GoldAnswer::Patterns(vec![PatternId::from_static("four.k.only")]),
                    mixed: false,
                    unsure: false,
                    thumb_pref: None,
                    selection: Selection {
                        pick: ChartPick::Sampled,
                        window: WindowPick::Sampled,
                    },
                    app_version: "test".into(),
                },
            )
        })
        .unwrap();
    assert_eq!(svc.export_jsonl().await.unwrap().lines().count(), 2);

    let out = svc.export_to_data_dir(7).await.unwrap();
    let text = std::fs::read_to_string(&out.path).unwrap();
    assert_eq!(out.rows, 1);
    assert!(
        text.contains(&maps[0].md5) && !text.contains(&maps[1].md5),
        "{text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn label_export_refuses_a_keymode_without_taxonomy() {
    let (f, _) = library().await;
    let err = f.ctx.labeling().export_to_data_dir(5).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(!f.dir.path().join("data").join("exports").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn label_export_dir_is_created_on_demand() {
    let (f, _) = library().await;
    let dir = f.ctx.labeling().exports_dir().await.unwrap();
    assert_eq!(dir, f.dir.path().join("data").join("exports"));
    assert!(dir.is_dir());
}
