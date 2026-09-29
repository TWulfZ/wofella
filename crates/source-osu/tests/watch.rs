#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::Duration;

use wolluf_source_osu::watch::{SourceChange, WatchMode, spawn};

const POLL: Duration = Duration::from_millis(50);
const DEBOUNCE: Duration = Duration::from_millis(300);
/// Long enough for the poll watcher to take its baseline scan before the test writes.
const SETTLE: Duration = Duration::from_millis(250);
const WAIT: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_millis(1_200);

#[test]
fn burst_debounced_to_one() {
    let root = tempfile::tempdir().unwrap();
    let data_r = root.path().join("Data").join("r");
    fs::create_dir_all(&data_r).unwrap();
    let (handle, rx) = spawn(root.path(), WatchMode::Poll { interval: POLL }, DEBOUNCE).unwrap();
    thread::sleep(SETTLE);
    for i in 0..20 {
        fs::write(
            data_r.join(format!("0123456789abcdef0123456789abcdef-{i}.osr")),
            b"x",
        )
        .unwrap();
    }
    assert_eq!(rx.recv_timeout(WAIT).unwrap(), SourceChange::ReplayDir);
    assert_eq!(rx.recv_timeout(QUIET), Err(RecvTimeoutError::Timeout));
    handle.stop();
}

#[test]
fn unrelated_root_file_ignored() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("Data").join("r")).unwrap();
    let (handle, rx) = spawn(root.path(), WatchMode::Poll { interval: POLL }, DEBOUNCE).unwrap();
    thread::sleep(SETTLE);
    fs::write(root.path().join("osu!.db"), b"x").unwrap();
    fs::write(root.path().join("notes.txt"), b"x").unwrap();
    assert_eq!(rx.recv_timeout(QUIET), Err(RecvTimeoutError::Timeout));
    fs::write(root.path().join("scores.db"), b"x").unwrap();
    assert_eq!(rx.recv_timeout(WAIT).unwrap(), SourceChange::ScoresDb);
    handle.stop();
}

#[test]
fn native_mode_sees_scores_db() {
    let root = tempfile::tempdir().unwrap();
    let (handle, rx) = spawn(root.path(), WatchMode::Native, DEBOUNCE).unwrap();
    thread::sleep(SETTLE);
    fs::write(root.path().join("scores.db"), b"x").unwrap();
    assert_eq!(rx.recv_timeout(WAIT).unwrap(), SourceChange::ScoresDb);
    handle.stop();
}
