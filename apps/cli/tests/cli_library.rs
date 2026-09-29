#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Env, FIXTURE_7K_TITLE, osu_7k};
use predicates::str::starts_with;

/// Rows at 0, 0.25, 0.5, 0.75 (LN head), 1 (tap under the LN body), 1.5 (LN tail), 1.75 and
/// 3 s.
fn chart() -> Vec<u8> {
    osu_7k(
        &[
            (0, 0),
            (3, 250),
            (6, 500),
            (5, 1_000),
            (4, 1_750),
            (1, 3_000),
        ],
        &[(2, 750, 1_500)],
    )
}

/// A synced env: `sync` chains `IndexLibrary` and waits for it.
fn synced() -> (Env, String) {
    let env = Env::new();
    let (root, md5) = env.install_with_chart(&chart());
    env.set_install(&root);
    env.json(&["sync"]);
    (env, md5)
}

fn stdout(env: &Env, args: &[&str]) -> String {
    let out = env.wolluf().args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn index_prints_the_index_library_job() {
    let (env, _) = synced();
    let job = env.json(&["library", "index"]);
    assert_eq!(job["kind"], "index_library", "{job}");
    assert_eq!(job["status"], "ok", "{job}");
    let c = &job["summary"]["counters"];
    assert_eq!(
        c["parsedNew"], 0,
        "the chained index already parsed it: {job}"
    );
    assert_eq!(c["skippedMemoized"], 1, "{job}");

    let text = stdout(&env, &["library", "index"]);
    assert!(
        text.lines()
            .any(|l| l.starts_with("status") && l.ends_with("ok")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("skipped memoized") && l.ends_with('1')),
        "{text}"
    );
}

#[test]
fn list_shows_the_synthetic_chart() {
    let (env, md5) = synced();
    let list = env.json(&["library", "list"]);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0]["md5"], md5.as_str());
    assert_eq!(list[0]["title"], FIXTURE_7K_TITLE);
    assert_eq!(list[0]["keymode"], 7);
    assert_eq!(list[0]["nLn"], 1);

    assert_eq!(
        env.json(&["library", "list", "--keys", "4"]),
        serde_json::json!([])
    );
    assert_eq!(
        env.json(&["library", "list", "--offset", "1"]),
        serde_json::json!([])
    );
    assert_eq!(
        env.json(&["library", "list", "--text", "no-such-title"]),
        serde_json::json!([])
    );

    let table = stdout(&env, &["library", "list"]);
    let mut lines = table.lines();
    assert!(lines.next().unwrap().starts_with("MD5"), "{table}");
    let row = lines.next().unwrap_or_else(|| panic!("{table}"));
    assert!(row.starts_with(&md5), "{table}");
    assert!(row.contains(FIXTURE_7K_TITLE), "{table}");
}

#[test]
fn scales_is_an_array() {
    let (env, _) = synced();
    assert!(env.json(&["library", "scales"]).is_array());
    assert!(stdout(&env, &["library", "scales"]).starts_with("SCALE"));
}

#[test]
fn chart_show_prints_the_window_and_key_row() {
    let (env, md5) = synced();
    let out = stdout(&env, &["chart", "show", &md5, "--from", "0", "--to", "2"]);
    // Rows only at events, earliest at the bottom; the 3 s tap is past `--to`.
    assert_eq!(
        out,
        "00:01.750  ...|.+o..\n\
         00:01.500  ..T|.+...\n\
         00:01.000  ..:|.+.o.\n\
         00:00.750  ..H|.+...\n\
         00:00.500  ...|.+..o\n\
         00:00.250  ...|o+...\n\
         00:00.000  o..|.+...\n\
         \x20          rmi|t+imr  k7.313_right_thumb\n"
    );

    let mmss = stdout(
        &env,
        &["chart", "show", &md5, "--from", "0:00", "--to", "0:02"],
    );
    assert_eq!(mmss, out);

    let left = stdout(
        &env,
        &[
            "chart",
            "show",
            &md5,
            "--to",
            "2",
            "--layout",
            "k7.313_left_thumb",
        ],
    );
    assert!(
        left.lines().last().unwrap().ends_with("k7.313_left_thumb"),
        "{left}"
    );
}

#[test]
fn chart_info_prints_the_detail() {
    let (env, md5) = synced();
    let detail = env.json(&["chart", "info", &md5]);
    assert_eq!(detail["chart"]["md5"], md5.as_str(), "{detail}");
    assert_eq!(detail["diagnostics"], 0, "{detail}");
    let text = stdout(&env, &["chart", "info", &md5]);
    assert!(
        text.lines()
            .any(|l| l.starts_with("title") && l.ends_with(FIXTURE_7K_TITLE)),
        "{text}"
    );
}

#[test]
fn bad_input_exits_2_invalid_input() {
    let (env, md5) = synced();
    for args in [
        vec!["chart", "show", "not-an-md5"],
        vec!["chart", "info", "not-an-md5"],
        vec!["chart", "show", &md5, "--layout", "k7.no_such_layout"],
        vec!["chart", "show", &md5, "--from", "5", "--to", "5"],
    ] {
        env.wolluf()
            .args(&args)
            .assert()
            .code(2)
            .stderr(starts_with("error[INVALID_INPUT]: "));
    }
    // A malformed time never reaches the app: clap rejects it.
    env.wolluf()
        .args(["chart", "show", &md5, "--from", "1:75"])
        .assert()
        .code(2);
}

#[test]
fn unknown_chart_exits_2_not_found() {
    let (env, _) = synced();
    env.wolluf()
        .args(["chart", "show", &"0".repeat(32)])
        .assert()
        .code(2)
        .stderr(starts_with("error[NOT_FOUND]: "));
}
