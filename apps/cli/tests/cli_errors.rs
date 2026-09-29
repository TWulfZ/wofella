#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;

use common::{Env, text};
use predicates::str::starts_with;
use wolluf_app::context::{AppContext, AppPaths};
use wolluf_core::{FixedClock, UnixUs};

const T0: UnixUs = UnixUs(1_790_637_236_636_000);

#[test]
fn bad_flag_exits_2() {
    let env = Env::new();
    env.wolluf().arg("--no-such-flag").assert().code(2);
    env.wolluf().args(["setup", "bogus"]).assert().code(2);
}

#[test]
fn locked_data_dir_exits_1_conflict() {
    let env = Env::new();
    let _held = AppContext::open(
        AppPaths::from_data_dir(env.data_dir()),
        Arc::new(FixedClock::new(T0)),
    )
    .unwrap();
    env.wolluf()
        .args(["setup", "status"])
        .assert()
        .code(1)
        .stderr(starts_with("error[CONFLICT]: error.instance_running"));
}

#[test]
fn invalid_install_path_exits_2_with_path_arg() {
    let env = Env::new();
    let missing = env.dir.path().join("not-osu");
    env.wolluf()
        .args(["setup", "set"])
        .arg(&missing)
        .assert()
        .code(2)
        .stderr(starts_with(format!(
            "error[OSU_DIR_NOT_FOUND]: error.code.OSU_DIR_NOT_FOUND {{path={}}}",
            text(&missing)
        )));
}
