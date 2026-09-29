#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Env, text};

#[test]
fn detect_json_lists_env_candidate() {
    let env = Env::new();
    let root = env.install();
    let out = env
        .wolluf()
        .env("WOLLUF_OSU_DIR", &root)
        .args(["--json", "setup", "detect"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let candidates: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let env_candidate = candidates
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["source"] == "env")
        .unwrap_or_else(|| panic!("no env candidate in {candidates}"));
    assert_eq!(env_candidate["path"], text(&root));
    assert_eq!(env_candidate["valid"], true);
    assert_eq!(env_candidate["osuDbVersion"], 20260924);
    assert_eq!(env_candidate["missing"], serde_json::json!([]));
}

#[test]
fn detect_text_is_a_table() {
    let env = Env::new();
    let root = env.install();
    let out = env
        .wolluf()
        .env("WOLLUF_OSU_DIR", &root)
        .args(["setup", "detect"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut lines = stdout.lines();
    assert!(lines.next().unwrap().starts_with("SOURCE"), "{stdout}");
    assert!(
        lines.any(|l| l.starts_with("env") && l.ends_with(&text(&root))),
        "{stdout}"
    );
}

#[test]
fn set_then_status_json() {
    let env = Env::new();
    let root = env.install();
    let set = env.json(&["setup", "set", &text(&root)]);
    assert_eq!(set["rootPath"], text(&root));
    assert_eq!(set["osuDbVersion"], 20260924);

    let status = env.json(&["setup", "status"]);
    assert_eq!(status["install"], set);
    assert_eq!(status["identityReady"], true, "no plays yet");
    assert_eq!(status["lastSync"], serde_json::Value::Null);
    assert_eq!(status["dataDir"], text(&env.data_dir()));
    assert_eq!(status["logsDir"], text(&env.data_dir().join("logs")));
}
