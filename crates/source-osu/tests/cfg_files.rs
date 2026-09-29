#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs::{self, File};
use std::time::{Duration, SystemTime};

use wolluf_source_osu::cfg_files::{list_user_cfgs, read_user_cfg};

fn touch(path: &std::path::Path, contents: &[u8], mtime: SystemTime) {
    fs::write(path, contents).unwrap();
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
}

#[test]
fn lists_account_cfgs_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    touch(
        &dir.path().join("osu!.old.cfg"),
        b"Username = old\r\n",
        base,
    );
    touch(
        &dir.path().join("osu!.twulfz.cfg"),
        b"Username = new\r\n",
        base + Duration::from_secs(60),
    );
    touch(
        &dir.path().join("osu!.mid.CFG"),
        b"",
        base + Duration::from_secs(30),
    );
    let cfgs = list_user_cfgs(dir.path()).unwrap();
    let accounts: Vec<&str> = cfgs.iter().map(|c| c.account.as_str()).collect();
    assert_eq!(accounts, vec!["twulfz", "mid", "old"]);
    let (cfg, _) = read_user_cfg(&cfgs[0].path).unwrap();
    assert_eq!(cfg.username.as_deref(), Some("new"));
}

#[test]
fn ignores_global_cfg() {
    let dir = tempfile::tempdir().unwrap();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    touch(&dir.path().join("osu!.cfg"), b"", now);
    touch(&dir.path().join("osu!..cfg"), b"", now);
    touch(&dir.path().join("osu!.x.cfg.bak"), b"", now);
    touch(&dir.path().join("notosu!.x.cfg"), b"", now);
    fs::create_dir(dir.path().join("osu!.dir.cfg")).unwrap();
    touch(&dir.path().join("osu!.me.cfg"), b"", now);
    let accounts: Vec<String> = list_user_cfgs(dir.path())
        .unwrap()
        .into_iter()
        .map(|c| c.account)
        .collect();
    assert_eq!(accounts, vec!["me".to_owned()]);
}

#[test]
fn missing_root_is_io_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(list_user_cfgs(&dir.path().join("nope")).is_err());
    assert!(read_user_cfg(&dir.path().join("osu!.x.cfg")).is_err());
}
