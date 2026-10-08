#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::Env;

const SSR_JOB: &str = "compute_play_ssr";

#[test]
fn a_malformed_scope_is_a_usage_error() {
    let env = Env::new();
    env.wolluf()
        .args(["preview", "skill", "--scope", "me"])
        .assert()
        .code(2);
}

#[test]
fn self_before_any_sync_is_not_found() {
    let env = Env::new();
    let out = env.wolluf().args(["preview", "skill"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("error[NOT_FOUND]"), "{stderr}");
}

#[test]
fn self_after_sync_is_one_settled_preview() {
    let env = Env::new();
    env.set_install(&env.install());
    env.json(&["sync"]);

    let previews = env.json(&["preview", "skill", "--keys", "7"]);
    let previews = previews.as_array().unwrap();
    assert_eq!(previews.len(), 1, "one self alias, one scope: {previews:?}");
    let p = &previews[0];
    assert_eq!(p["keymode"], 7);
    assert_eq!(p["method"], "preview.etterna_rating@1");
    assert!(p["calcVersion"].is_i64(), "{p}");
    assert_eq!(p["scopeHash"].as_str().unwrap().len(), 64, "{p}");
    // `sync` waits for its chain, which ends with ComputePlaySsr, so nothing is left pending.
    assert_ne!(p["state"], "computing", "{p}");
    let warnings = p["warnings"].as_array().unwrap();
    for code in ["uncalibrated", "k7_less_validated"] {
        assert!(warnings.iter().any(|w| w == code), "{code}: {p}");
    }
    for field in [
        "overallCenti",
        "skillsets",
        "dan",
        "evidence",
        "topPlays",
        "trend",
    ] {
        assert!(p.get(field).is_some(), "{field}: {p}");
    }
}

#[test]
fn all_players_text_starts_with_the_beta_banner() {
    let env = Env::new();
    env.set_install(&env.install());
    env.json(&["sync"]);

    let out = env
        .wolluf()
        .args(["preview", "skill", "--scope", "all", "--merge", "merged"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.starts_with("Skill preview 4K · Beta · uncalibrated · MinaCalc "),
        "{stdout}"
    );
    for key in ["scope", "state", "overall", "evidence", "warnings"] {
        assert!(
            stdout.lines().any(|l| l.starts_with(&format!("{key} "))),
            "{key}: {stdout}"
        );
    }
}

#[test]
fn library_index_waits_for_the_chained_ssr_job() {
    let env = Env::new();
    env.set_install(&env.install());
    env.json(&["sync"]);
    let index = env.json(&["library", "index"]);
    assert_eq!(index["status"], "ok", "{index}");

    let jobs = env.json(&["jobs", "list"]);
    let ssr: Vec<_> = jobs
        .as_array()
        .unwrap()
        .iter()
        .filter(|j| j["kind"] == SSR_JOB)
        .collect();
    assert!(!ssr.is_empty(), "{jobs}");
    // Closing the context cancels whatever the CLI did not wait for.
    assert!(ssr.iter().all(|j| j["status"] == "ok"), "{jobs}");
}
