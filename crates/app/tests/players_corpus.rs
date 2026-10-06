#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Spec 004 AC15 (F0 exit): on the pilot's real data, only the session user is auto-selected and
//! the self profile holds exactly those aliases.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use wolluf_app::clock::SystemClock;
use wolluf_app::context::{AppContext, AppPaths};
use wolluf_app::features::players::identity::{AliasRow, EntryKind};
use wolluf_app::features::players::names::SessionMatchKind;
use wolluf_app::features::players::selection::{AutoMatch, MatchSource};
use wolluf_app::jobs::JobStatusDto;
use wolluf_core::{AliasId, Keymode};
use wolluf_source_osu::cfg_files::{list_user_cfgs, read_user_cfg};

/// The pilot's session user: equal to the cfg login, or a normalized prefix of it while the login is
/// garbage (spec 004 pilot outcome). The live cfg decides which.
const SESSION_ALIAS: &[u8] = b"TWulfZ";
/// The garbage login the pilot once had; auto-matched only while the cfg still holds it.
const CFG_LOGIN_ALIAS: &[u8] = b"TWulfZasdasdasd d jSS||";
/// Spec 004 AC15: listed, never suggested.
const UNSELECTED_ALIASES: [&[u8]; 8] = [
    b"",
    b"W",
    b"w",
    b"Wulf",
    b"s",
    b"Madeline",
    b"Klinsx",
    b"StevenS",
];

fn row<'a>(rows: &'a [AliasRow], raw_name: &[u8]) -> &'a AliasRow {
    rows.iter()
        .find(|r| r.raw_name == raw_name)
        .unwrap_or_else(|| panic!("alias {:?} missing", String::from_utf8_lossy(raw_name)))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
async fn players_corpus_selection() {
    let root = common::corpus();
    let tree_before = common::tree_state(&root);

    let data = tempfile::tempdir().unwrap();
    let ctx = AppContext::open(
        AppPaths::from_data_dir(data.path().join("data")),
        Arc::new(SystemClock),
    )
    .unwrap();
    let install = ctx.register_install(root.clone(), None).await.unwrap();
    let job = ctx.plays().sync_and_wait(install).await.unwrap();
    assert_eq!(job.status, JobStatusDto::Ok, "{job:?}");
    ctx.players().refresh().await.unwrap();

    let list = ctx.players().list_aliases().await.unwrap();
    assert!(list.cfg_username_available, "the pilot has an osu! cfg");
    for r in &list.rows {
        println!(
            "{:?} plays={} auto={:?} selected={}",
            String::from_utf8_lossy(&r.raw_name),
            r.stats.n_plays,
            r.auto_match,
            r.selected
        );
    }

    let cfgs = list_user_cfgs(&root).unwrap();
    let newest = cfgs.first().expect("no osu!.<account>.cfg in the corpus");
    let login = read_user_cfg(&newest.path).unwrap().0.username;
    let garbage_login = login.as_deref().map(str::as_bytes) == Some(CFG_LOGIN_ALIAS);

    let session = row(&list.rows, SESSION_ALIAS);
    assert!(session.selected);
    let expected_kind = if login.as_deref().map(str::as_bytes) == Some(SESSION_ALIAS) {
        SessionMatchKind::Equal
    } else {
        SessionMatchKind::Prefix
    };
    assert_eq!(
        session.auto_match,
        Some(AutoMatch {
            source: MatchSource::CfgUsername,
            kind: expected_kind,
        })
    );
    if let Some(old_login) = list.rows.iter().find(|r| r.raw_name == CFG_LOGIN_ALIAS) {
        assert_eq!(old_login.selected, garbage_login, "{old_login:?}");
        let expected = garbage_login.then_some(SessionMatchKind::Equal);
        assert_eq!(
            old_login.auto_match.map(|m| m.kind),
            expected,
            "{old_login:?}"
        );
    }
    for name in UNSELECTED_ALIASES {
        let r = row(&list.rows, name);
        assert!(!r.selected, "{r:?}");
        assert_eq!(r.auto_match, None, "{r:?}");
    }
    let auto: BTreeSet<AliasId> = list
        .rows
        .iter()
        .filter(|r| r.auto_match.is_some())
        .inspect(|r| {
            assert!(
                r.raw_name == SESSION_ALIAS || (garbage_login && r.raw_name == CFG_LOGIN_ALIAS),
                "only the session user may be auto-matched: {r:?}"
            );
            assert!(r.selected && r.in_self_profile, "{r:?}");
        })
        .map(|r| r.alias_id)
        .collect();

    let profiles = ctx.players().list_profiles(Keymode::K7).await.unwrap();
    let own = profiles
        .iter()
        .find(|p| p.kind == EntryKind::SelfProfile)
        .unwrap_or_else(|| panic!("no self profile among {profiles:?}"));
    let own_aliases: BTreeSet<AliasId> = own.alias_ids.iter().copied().collect();
    assert_eq!(
        own_aliases, auto,
        "the self profile holds exactly the auto-selected aliases"
    );
    drop(ctx);

    common::assert_unchanged("sync and refresh", &tree_before, &common::tree_state(&root));
}
