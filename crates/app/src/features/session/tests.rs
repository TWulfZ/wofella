use std::time::Duration;

use wolluf_core::{DotNetTicks, ErrorCode};
use wolluf_source_osu::testkit::{OsrBuilder, ScoreBuilder};
use wolluf_store::time::format_rfc3339_ms;

use super::dto::SessionPlaysDto;
use super::*;
use crate::errors::AppError;
use crate::events::{AppEvent, SessionPlayAddedDto};
use crate::features::labeling::dto::{
    AnchorDto, ChartPickDto, LabelSelectionDto, LabelSubmitDto, SelectionCountDto,
    SessionLabelSubmitDto, WindowPickDto,
};
use crate::features::library::testkit::{Map, install};
use crate::features::plays::testkit::{Fixture, T0, osr_name};
use crate::watch::{InstallWatcher, WatchParams};

const WAIT: Duration = Duration::from_secs(30);
/// Long enough for a tracker that would wrongly emit to have done so.
const QUIET: Duration = Duration::from_millis(300);
const DOTNET_TICKS_PER_US: i64 = 10;
const DOTNET_UNIX_EPOCH_TICKS: i64 = 621_355_968_000_000_000;
const US_PER_S: i64 = 1_000_000;
const SELF_NAME: &str = "TWulfZ";
const OTHER_NAME: &str = "Kovacs";

fn alpha() -> Map {
    Map::k7("alpha").rated(3.5)
}

fn beta() -> Map {
    Map::k7("beta").named("200 wolluf - beta", "Hard")
}

fn gamma() -> Map {
    Map::k7("gamma")
}

fn four() -> Map {
    Map::new(
        "four",
        4,
        crate::features::library::testkit::osu_text(4, "four", &[(0, 1_000), (1, 1_250)], &[]),
    )
}

/// A replay saved `secs` after the session started.
fn score(map: &Map, player: &str, secs: i64, nth: i64) -> ScoreBuilder {
    let us = T0.0 + secs * US_PER_S;
    ScoreBuilder::mania(&map.md5, player, nth).ticks(DotNetTicks(
        us * DOTNET_TICKS_PER_US + DOTNET_UNIX_EPOCH_TICKS,
    ))
}

fn save_replay(f: &Fixture, s: &ScoreBuilder) {
    f.write(
        std::path::Path::new("Data/r").join(osr_name(s).format()),
        &OsrBuilder::new(s.clone()).build(),
    );
}

/// The cfg login the identity rule matches `SELF_NAME` against (ADR 0005).
const SELF_CFG: &str = "Username = TWulfZ\r\n";

/// Synced once (the self profile holds `SELF_NAME`), with one play from before the session.
async fn session() -> Fixture {
    let maps = [alpha(), beta(), gamma(), four()];
    let f = Fixture::new(&install(&maps, &[&maps[0]]).cfg("fixture", SELF_CFG)).await;
    f.sync().await;
    f
}

fn play_id(s: &ScoreBuilder) -> String {
    let name = osr_name(s);
    wolluf_core::PlayId::derive(
        wolluf_core::Game::OsuStable,
        name.md5,
        s.header().player.as_bytes().unwrap(),
        name.filetime,
    )
    .to_string()
}

async fn next_session_event(rx: &mut tokio::sync::broadcast::Receiver<AppEvent>) -> AppEvent {
    tokio::time::timeout(WAIT, async {
        loop {
            if let e @ (AppEvent::SessionPlayAdded(_) | AppEvent::AttentionRequested) =
                rx.recv().await.unwrap()
            {
                return e;
            }
        }
    })
    .await
    .expect("a session event in time")
}

fn added(s: &ScoreBuilder, map: &Map) -> AppEvent {
    AppEvent::SessionPlayAdded(SessionPlayAddedDto {
        play_id: play_id(s),
        md5: map.md5.clone(),
    })
}

fn dominant(
    map: &Map,
    play: Option<&ScoreBuilder>,
    pattern: Option<&str>,
) -> SessionLabelSubmitDto {
    SessionLabelSubmitDto {
        keymode: map.keys,
        md5: map.md5.clone(),
        play_id: play.map(play_id),
        pattern: pattern.map(str::to_owned),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn session_plays_are_self_plays_since_start_newest_first() {
    let f = session().await;
    let (a, b, k4) = (alpha(), beta(), four());
    save_replay(&f, &score(&a, SELF_NAME, 10, 101));
    save_replay(&f, &score(&b, SELF_NAME, 20, 102));
    save_replay(&f, &score(&k4, SELF_NAME, 30, 103));
    // ADR 0005: another player's replay in the same `Data/r` never joins the session.
    save_replay(&f, &score(&b, OTHER_NAME, 40, 104));
    f.sync().await;

    let got = f.ctx.session().plays(7).await.unwrap();
    assert_eq!(got.started_at, format_rfc3339_ms(T0));
    let rows: Vec<(&str, &str, &str)> = got
        .plays
        .iter()
        .map(|p| (p.md5.as_str(), p.title.as_str(), p.version.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (b.md5.as_str(), "beta", "Hard"),
            (a.md5.as_str(), "alpha", "Normal")
        ]
    );
    let first = &got.plays[1];
    assert_eq!(first.play_id, play_id(&score(&a, SELF_NAME, 10, 101)));
    assert_eq!(
        first.played_at,
        format_rfc3339_ms(wolluf_core::UnixUs(T0.0 + 10 * US_PER_S))
    );
    assert_eq!(
        (first.keymode, first.stars, first.set_id),
        (7, Some(3.5), Some(100))
    );
    assert_eq!(
        (first.artist.clone(), first.creator.as_str()),
        (format!("artist-{}", a.md5), "wolluf")
    );
    assert!(first.label.is_none());

    let k4_rows = f.ctx.session().plays(4).await.unwrap().plays;
    assert_eq!(k4_rows.len(), 1);
    assert_eq!(k4_rows[0].md5, k4.md5);
    assert_eq!(
        f.ctx.session().plays(0).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_is_pending_before_a_self_profile_exists() {
    let maps = [alpha()];
    let f = Fixture::new(&install(&maps, &[]).cfg("fixture", SELF_CFG)).await;
    assert!(f.ctx.session().plays(7).await.unwrap().plays.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn session_label_is_stored_latest_wins_and_undo_restores() {
    let f = session().await;
    let (a, b) = (alpha(), beta());
    let mine = score(&b, SELF_NAME, 20, 102);
    let theirs = score(&b, OTHER_NAME, 40, 104);
    save_replay(&f, &mine);
    save_replay(&f, &theirs);
    f.sync().await;
    let labeling = f.ctx.labeling();

    let first = labeling
        .session_submit(dominant(&b, Some(&mine), Some("regular.stream.jumpstream")))
        .await
        .unwrap();
    let state = |plays: &SessionPlaysDto| plays.plays[0].label.clone().unwrap();
    let got = state(&f.ctx.session().plays(7).await.unwrap());
    assert_eq!(
        (got.event_id.as_str(), got.pattern.as_deref()),
        (first.id.as_str(), Some("regular.stream.jumpstream"))
    );
    assert_eq!(got.at, format_rfc3339_ms(T0));

    let second = labeling
        .session_submit(dominant(&b, None, None))
        .await
        .unwrap();
    let got = state(&f.ctx.session().plays(7).await.unwrap());
    assert_eq!(
        (got.event_id.as_str(), got.pattern),
        (second.id.as_str(), None)
    );

    labeling.session_undo(&second.id).await.unwrap();
    let got = state(&f.ctx.session().plays(7).await.unwrap());
    assert_eq!(got.event_id, first.id);
    assert_eq!(
        labeling.session_undo(&second.id).await.unwrap_err().code,
        ErrorCode::Conflict
    );
    assert_eq!(
        labeling.undo(&first.id).await.unwrap_err().code,
        ErrorCode::NotFound,
        "the gold undo never takes back a session label"
    );

    let code = |e: AppError| e.code;
    let unknown = dominant(&b, None, Some("regular.stream.nope"));
    assert_eq!(
        code(labeling.session_submit(unknown).await.unwrap_err()),
        ErrorCode::InvalidInput
    );
    let foreign = dominant(&b, Some(&theirs), Some("regular.stream.jumpstream"));
    assert_eq!(
        code(labeling.session_submit(foreign).await.unwrap_err()),
        ErrorCode::InvalidInput
    );
    let other_chart = dominant(&a, Some(&mine), None);
    assert_eq!(
        code(labeling.session_submit(other_chart).await.unwrap_err()),
        ErrorCode::InvalidInput
    );
    let wrong_keymode = SessionLabelSubmitDto {
        keymode: 7,
        ..dominant(&four(), None, None)
    };
    assert_eq!(
        code(labeling.session_submit(wrong_keymode).await.unwrap_err()),
        ErrorCode::NotFound
    );
    let not_in_catalog = SessionLabelSubmitDto {
        md5: "0123456789abcdef0123456789abcdef".to_owned(),
        ..dominant(&b, None, None)
    };
    assert_eq!(
        code(labeling.session_submit(not_in_catalog).await.unwrap_err()),
        ErrorCode::NotFound
    );
}

fn gold(map: &Map, t0_ms: i32, t1_ms: i32, patterns: &[&str]) -> LabelSubmitDto {
    LabelSubmitDto {
        anchor: AnchorDto {
            md5: map.md5.clone(),
            t0_ms,
            t1_ms,
            cols: vec![1, 2, 3, 4, 5, 6, 7],
        },
        patterns: patterns.iter().map(|p| (*p).to_owned()).collect(),
        no_pattern: patterns.is_empty(),
        mixed: false,
        unsure: false,
        thumb_pref: None,
        selection: SAMPLED,
    }
}

const SAMPLED: LabelSelectionDto = LabelSelectionDto {
    pick: ChartPickDto::Sampled,
    window: WindowPickDto::Sampled,
};

/// Written under the store, since the service only ever labels as the self profile.
fn append_other_profiles_gold_label(f: &Fixture, map: &Map) {
    use wolluf_core::{ColMask, Keymode, PatternId, SegmentAnchor, TimeUs, UnixUs};
    use wolluf_store::repo::labels::{
        ChartPick, GoldAnswer, NewGoldLabel, Selection, WindowPick, append_gold_label,
    };
    use wolluf_store::repo::players::{MergeMode, NewProfile, ProfileKind, profile};

    let md5: wolluf_core::ChartMd5 = map.md5.parse().unwrap();
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
                    created_at: UnixUs(0),
                },
            )?;
            append_gold_label(
                tx,
                &NewGoldLabel {
                    id: ulid::Ulid::from_parts(1, 1),
                    ts: UnixUs(0),
                    profile_id: other,
                    keymode: Keymode::K7,
                    anchor: SegmentAnchor::new(
                        md5,
                        TimeUs::from_ms(3_000),
                        TimeUs::from_ms(3_500),
                        ColMask::full(Keymode::K7),
                        Keymode::K7,
                    )
                    .unwrap(),
                    answer: GoldAnswer::Patterns(vec![PatternId::from_static(
                        "regular.jack.minijack",
                    )]),
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
}

/// ADR 0020: the gold set, its stats, its export and the sampler's exclusions never see a
/// session label.
#[tokio::test(flavor = "multi_thread")]
async fn session_labels_leave_gold_readers_unchanged() {
    let f = session().await;
    let a = alpha();
    let labeling = f.ctx.labeling();
    labeling
        .submit(gold(&a, 1_000, 2_000, &["regular.jack.minijack"]))
        .await
        .unwrap();
    let timeline = |md5: &str| crate::features::labeling::dto::ChartTimelineRequestDto {
        keymode: 7,
        md5: md5.to_owned(),
        buckets: 4,
    };
    let before = (
        labeling.stats().await.unwrap(),
        labeling.export_jsonl().await.unwrap(),
        labeling.chart_timeline(timeline(&a.md5)).await.unwrap(),
    );
    labeling
        .session_submit(dominant(&a, None, Some("regular.stream.jumpstream")))
        .await
        .unwrap();
    let after = (
        labeling.stats().await.unwrap(),
        labeling.export_jsonl().await.unwrap(),
        labeling.chart_timeline(timeline(&a.md5)).await.unwrap(),
    );
    assert_eq!(before, after);
}

/// T5: each session row shows how many gold windows its chart already has, undone ones aside.
#[tokio::test(flavor = "multi_thread")]
async fn session_rows_count_the_charts_gold_windows() {
    let f = session().await;
    let (a, b) = (alpha(), beta());
    save_replay(&f, &score(&a, SELF_NAME, 10, 101));
    save_replay(&f, &score(&b, SELF_NAME, 20, 102));
    f.sync().await;
    let labeling = f.ctx.labeling();
    for (t0, t1) in [(1_000, 1_500), (1_500, 2_000)] {
        labeling
            .submit(gold(&a, t0, t1, &["regular.jack.minijack"]))
            .await
            .unwrap();
    }
    let undone = labeling
        .submit(gold(&a, 2_000, 2_500, &["regular.jack.minijack"]))
        .await
        .unwrap();
    labeling.undo(&undone.id).await.unwrap();
    append_other_profiles_gold_label(&f, &a);

    let windows = |plays: &SessionPlaysDto| {
        plays
            .plays
            .iter()
            .map(|p| (p.md5.clone(), p.gold_windows))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        windows(&f.ctx.session().plays(7).await.unwrap()),
        [(b.md5.clone(), 0), (a.md5.clone(), 2)]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn progress_counts_gold_and_session_labels_by_local_day() {
    let f = session().await;
    let (a, b) = (alpha(), beta());
    let labeling = f.ctx.labeling();
    let empty = labeling.progress(7, 0).await.unwrap();
    assert_eq!((empty.gold_total, empty.session_labels), (0, 0));
    assert_eq!(empty.per_day.len(), 30);

    labeling
        .submit(gold(
            &a,
            1_000,
            2_000,
            &["regular.jack.minijack", "regular.stream.jumpstream"],
        ))
        .await
        .unwrap();
    let chosen = LabelSelectionDto {
        pick: ChartPickDto::NowPlaying,
        window: WindowPickDto::Sampled,
    };
    labeling
        .submit(LabelSubmitDto {
            selection: chosen,
            ..gold(&b, 1_000, 2_000, &[])
        })
        .await
        .unwrap();
    labeling
        .session_submit(dominant(&a, None, Some("regular.stream.jumpstream")))
        .await
        .unwrap();
    // Relabelling one map leaves one contribution.
    labeling
        .session_submit(dominant(&a, None, Some("regular.jack.minijack")))
        .await
        .unwrap();
    labeling
        .session_submit(dominant(&b, None, None))
        .await
        .unwrap();

    let p = labeling.progress(7, 0).await.unwrap();
    assert_eq!(
        (p.gold_total, p.gold_no_pattern, p.session_labels),
        (2, 1, 2)
    );
    assert_eq!(
        p.gold_blind, 1,
        "a now-playing pick is not blind (ADR 0021)"
    );
    assert_eq!(
        p.per_selection,
        [
            SelectionCountDto {
                selection: Some(SAMPLED),
                count: 1
            },
            SelectionCountDto {
                selection: Some(chosen),
                count: 1
            },
        ]
    );
    let keyed = |c: &[crate::features::labeling::dto::CountDto]| {
        c.iter()
            .map(|c| (c.key.clone(), c.count))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keyed(&p.per_pattern),
        [
            ("regular.jack.minijack".to_owned(), 1),
            ("regular.stream.jumpstream".to_owned(), 1)
        ]
    );
    assert_eq!(
        keyed(&p.per_axis),
        [
            ("7k.regular.jack".to_owned(), 1),
            ("7k.regular.stream".to_owned(), 1)
        ]
    );
    // Daily session counts add up to the contributions: the relabelled map counts once.
    let today = p.per_day.last().unwrap();
    assert_eq!(
        (today.day.as_str(), today.gold, today.session),
        ("2026-09-28", 2, 2)
    );
    assert_eq!(p.per_day[0].day, "2026-08-30");
    assert_eq!(p.recent.len(), 2);
    let newest = &p.recent[0];
    assert_eq!(
        (
            newest.md5.as_str(),
            newest.title.as_deref(),
            newest.version.as_deref()
        ),
        (b.md5.as_str(), Some("beta"), Some("Hard"))
    );
    assert!(newest.no_pattern && newest.patterns.is_empty());
    assert_eq!(newest.at, format_rfc3339_ms(T0));

    // T0 is 23:13 UTC: one hour east it is already the next day there.
    let east = labeling.progress(7, 60).await.unwrap();
    assert_eq!(east.per_day.last().unwrap().day, "2026-09-29");
    assert_eq!(east.per_day.last().unwrap().gold, 2);
    assert_eq!(
        labeling.progress(7, 15 * 60).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn tracker_emits_each_new_self_play_once_and_attention_only_when_notifying() {
    let f = session().await;
    let (a, b) = (alpha(), beta());
    let mut rx = f.ctx.subscribe();
    f.ctx.session().track().await.unwrap();

    let first = score(&a, SELF_NAME, 10, 101);
    save_replay(&f, &first);
    save_replay(&f, &score(&b, OTHER_NAME, 15, 102));
    f.sync().await;
    assert_eq!(next_session_event(&mut rx).await, added(&first, &a));

    // Notify is off by default: the next session event is the next play, never an attention
    // request, and neither the foreign play nor `first` again came in between.
    let second = score(&b, SELF_NAME, 20, 103);
    save_replay(&f, &second);
    f.sync().await;
    assert_eq!(next_session_event(&mut rx).await, added(&second, &b));

    f.ctx.settings().set_session_notify(true).await.unwrap();
    let g = gamma();
    let third = score(&g, SELF_NAME, 30, 104);
    save_replay(&f, &third);
    f.sync().await;
    assert_eq!(next_session_event(&mut rx).await, added(&third, &g));
    assert_eq!(
        next_session_event(&mut rx).await,
        AppEvent::AttentionRequested
    );

    // An unchanged install emits nothing.
    f.sync().await;
    tokio::time::sleep(QUIET).await;
    while let Ok(e) = rx.try_recv() {
        assert!(
            !matches!(
                e,
                AppEvent::SessionPlayAdded(_) | AppEvent::AttentionRequested
            ),
            "{e:?}"
        );
    }
}

/// The flash means "a map joined the list to label": a play the list never shows as a new
/// pending map (another keymode without a taxonomy, a chart osu!.db does not list yet, a map
/// already answered, a retry of a map already listed) is announced but never flashes.
#[tokio::test(flavor = "multi_thread")]
async fn tracker_requests_attention_only_for_a_new_pending_map() {
    let f = session().await;
    let (b, g, k4) = (beta(), gamma(), four());
    let unlisted = Map::k7("unlisted");
    f.ctx
        .labeling()
        .session_submit(dominant(&b, None, Some("regular.stream.jumpstream")))
        .await
        .unwrap();
    f.ctx.settings().set_session_notify(true).await.unwrap();
    let mut rx = f.ctx.subscribe();
    f.ctx.session().track().await.unwrap();

    let steps = [
        (score(&k4, SELF_NAME, 10, 101), &k4),
        (score(&unlisted, SELF_NAME, 20, 102), &unlisted),
        (score(&b, SELF_NAME, 30, 103), &b),
    ];
    for (play, map) in &steps {
        save_replay(&f, play);
        f.sync().await;
        assert_eq!(next_session_event(&mut rx).await, added(play, map));
    }

    let pending = score(&g, SELF_NAME, 40, 104);
    save_replay(&f, &pending);
    f.sync().await;
    assert_eq!(next_session_event(&mut rx).await, added(&pending, &g));
    assert_eq!(
        next_session_event(&mut rx).await,
        AppEvent::AttentionRequested
    );

    let retry = score(&g, SELF_NAME, 50, 105);
    save_replay(&f, &retry);
    f.sync().await;
    assert_eq!(next_session_event(&mut rx).await, added(&retry, &g));
    tokio::time::sleep(QUIET).await;
    while let Ok(e) = rx.try_recv() {
        assert_ne!(e, AppEvent::AttentionRequested);
    }
}

/// Acceptance: a self replay written while the app runs reaches the UI as an event, through
/// the install watcher, with no manual sync.
#[tokio::test(flavor = "multi_thread")]
async fn start_watches_the_selected_install_until_stopped() {
    let f = session().await;
    let a = alpha();
    let params = SessionParams {
        watch: WatchParams {
            debounce: Duration::from_millis(200),
            poll_interval: Duration::from_millis(50),
        },
    };
    std::fs::create_dir_all(f.root.join("Data/r")).unwrap();
    let mut rx = f.ctx.subscribe();
    let session = f.ctx.session();
    assert_eq!(session.watching(), None);
    session.start(params).await.unwrap();
    session.start(params).await.unwrap();
    assert_eq!(session.watching(), Some(f.install));

    let play = score(&a, SELF_NAME, 10, 101);
    save_replay(&f, &play);
    assert_eq!(next_session_event(&mut rx).await, added(&play, &a));

    session.stop().await;
    assert_eq!(session.watching(), None);
    let Fixture { ctx, dir, .. } = f;
    ctx.close().await;
    drop(dir);
}

/// `start` and `follow_selected_install` await (the tracker's seed read, the watcher's start,
/// slow on drvfs) before keeping what they built; a `stop` in that gap must win.
#[tokio::test(flavor = "multi_thread")]
async fn a_tracker_or_watcher_ready_only_after_stop_is_dropped_not_kept() {
    let f = session().await;
    std::fs::create_dir_all(f.root.join("Data/r")).unwrap();
    let state = f.ctx.session_state();
    let before_stop = state.epoch();
    state.stop();

    let task = f.ctx.runtime().spawn(std::future::pending::<()>());
    assert!(!state.adopt_tracker(before_stop, task.abort_handle()));
    assert!(task.await.unwrap_err().is_cancelled());

    let watcher = InstallWatcher::start(&f.ctx, f.install, WatchParams::default())
        .await
        .unwrap();
    let refused = state.adopt_watcher(before_stop, Some((f.install, watcher)));
    assert!(refused.is_some());
    assert_eq!(f.ctx.session().watching(), None);

    let current = state.epoch();
    let task = f.ctx.runtime().spawn(std::future::pending::<()>());
    assert!(state.adopt_tracker(current, task.abort_handle()));
    state.stop();
    assert!(task.await.unwrap_err().is_cancelled());
    let Fixture { ctx, dir, .. } = f;
    drop(refused);
    ctx.close().await;
    drop(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replay_written_after_stop_emits_no_session_event() {
    let f = session().await;
    let params = SessionParams {
        watch: WatchParams {
            debounce: Duration::from_millis(100),
            poll_interval: Duration::from_millis(50),
        },
    };
    std::fs::create_dir_all(f.root.join("Data/r")).unwrap();
    let session = f.ctx.session();
    session.start(params).await.unwrap();
    session.stop().await;
    let mut rx = f.ctx.subscribe();
    save_replay(&f, &score(&alpha(), SELF_NAME, 10, 101));
    tokio::time::sleep(QUIET + params.watch.debounce).await;
    while let Ok(e) = rx.try_recv() {
        assert!(!matches!(e, AppEvent::SessionPlayAdded(_)), "{e:?}");
    }
    let Fixture { ctx, dir, .. } = f;
    ctx.close().await;
    drop(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn start_without_an_install_tracks_and_a_registered_install_is_then_watched() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = crate::context::AppContext::open(
        crate::context::AppPaths::from_data_dir(dir.path().join("data")),
        std::sync::Arc::new(wolluf_core::FixedClock::new(T0)),
    )
    .unwrap();
    ctx.session().start(SessionParams::default()).await.unwrap();
    assert_eq!(ctx.session().watching(), None);
    let root = dir.path().join("osu!");
    std::fs::create_dir_all(root.join("Data/r")).unwrap();
    let id = ctx.register_install(root, None).await.unwrap();
    ctx.session().follow_selected_install().await.unwrap();
    assert_eq!(ctx.session().watching(), Some(id));
    ctx.close().await;
}

/// Shells (Tauri) need every service future to be `Send` for any borrow of the context.
#[test]
fn session_futures_are_send() {
    fn is_send<T: Send>(_: T) {}
    fn check(ctx: &crate::context::AppContext, req: SessionLabelSubmitDto) {
        is_send(async move { ctx.session().plays(7).await });
        is_send(async move { ctx.session().start(SessionParams::default()).await });
        is_send(async move { ctx.session().follow_selected_install().await });
        is_send(async move { ctx.labeling().session_submit(req).await });
        is_send(async move { ctx.labeling().progress(7, 0).await });
    }
    let _ = check;
}
