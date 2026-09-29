#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Env, FIXTURE_PLAYS};

fn counters(job: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(job["kind"], "sync_plays", "{job}");
    assert_eq!(job["summary"]["kind"], "sync_plays", "{job}");
    &job["summary"]["counters"]
}

#[test]
fn first_sync_reports_new_plays() {
    let env = Env::new();
    env.set_install(&env.install());
    let job = env.json(&["sync"]);
    assert_eq!(job["status"], "ok", "{job}");
    let c = counters(&job);
    assert_eq!(c["playsNew"], FIXTURE_PLAYS, "{job}");
    assert_eq!(c["playsExisting"], 0, "{job}");
}

#[test]
fn second_sync_adds_zero() {
    let env = Env::new();
    env.set_install(&env.install());
    env.json(&["sync"]);
    let job = env.json(&["sync"]);
    assert_eq!(job["status"], "ok", "{job}");
    let c = counters(&job);
    assert_eq!(c["playsNew"], 0, "{job}");
    assert_eq!(c["playsReplayOnly"], 0, "{job}");
    assert_eq!(c["conflicts"], 0, "{job}");
}

#[test]
fn text_summary_without_tty_has_no_progress() {
    let env = Env::new();
    env.set_install(&env.install());
    let out = env.wolluf().arg("sync").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("status"), "{stdout}");
    assert!(
        stdout
            .lines()
            .any(|l| l.starts_with("plays new") && l.ends_with(&FIXTURE_PLAYS.to_string())),
        "{stdout}"
    );
    // Progress lines are for a terminal only; a pipe gets nothing on stderr.
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
