#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Env, FIXTURE_PLAYS};

#[test]
fn list_shows_finished_sync() {
    let env = Env::new();
    env.set_install(&env.install());
    let synced = env.json(&["sync"]);

    let jobs = env.json(&["jobs", "list"]);
    let jobs = jobs.as_array().unwrap();
    let sync = jobs
        .iter()
        .find(|j| j["id"] == synced["id"])
        .unwrap_or_else(|| panic!("{jobs:?}"));
    assert_eq!(sync["kind"], "sync_plays");
    assert_eq!(sync["status"], "ok");
    assert_eq!(sync["summary"]["counters"]["playsNew"], FIXTURE_PLAYS);
    assert!(
        jobs.iter()
            .any(|j| j["kind"] == "refresh_identity" && j["status"] == "ok"),
        "the chained identity refresh finished before sync returned: {jobs:?}"
    );

    let newest = env.json(&["jobs", "list", "--limit", "1"]);
    assert_eq!(newest.as_array().unwrap().len(), 1);

    let out = env.wolluf().args(["jobs", "list"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("ID"), "{stdout}");
    let row = stdout
        .lines()
        .find(|l| l.starts_with(synced["id"].as_str().unwrap()))
        .unwrap_or_else(|| panic!("{stdout}"));
    assert!(row.contains("sync_plays") && row.contains("ok"), "{row}");
    assert!(row.contains(&format!("new={FIXTURE_PLAYS}")), "{row}");
}

#[test]
fn empty_history_is_an_empty_array() {
    let env = Env::new();
    assert_eq!(env.json(&["jobs", "list"]), serde_json::json!([]));
}
