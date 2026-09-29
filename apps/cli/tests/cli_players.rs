#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Env, FIXTURE_PLAYER};

#[test]
fn list_json_shape() {
    let env = Env::new();
    env.set_install(&env.install());
    env.json(&["sync"]);
    let list = env.json(&["players", "list"]);
    // Every played_at comes from the fixture's FILETIMEs, so nothing here depends on the wall
    // clock and no field needs redacting.
    insta::assert_snapshot!(serde_json::to_string_pretty(&list).unwrap());
}

#[test]
fn list_text_ticks_the_session_user() {
    let env = Env::new();
    env.set_install(&env.install());
    env.json(&["sync"]);
    let out = env.wolluf().args(["players", "list"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut lines = stdout.lines();
    assert!(lines.next().unwrap().starts_with("SEL"), "{stdout}");
    let me = lines
        .clone()
        .find(|l| l.ends_with(FIXTURE_PLAYER))
        .unwrap_or_else(|| panic!("{stdout}"));
    assert!(me.starts_with('x'), "{me}");
    assert!(me.contains("cfg_username/equal"), "{me}");
    let empty = lines
        .find(|l| l.ends_with("(empty)"))
        .unwrap_or_else(|| panic!("{stdout}"));
    assert!(!empty.starts_with('x'), "{empty}");
}
