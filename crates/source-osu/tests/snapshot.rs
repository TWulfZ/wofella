#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};
use wolluf_core::ErrorCode;
use wolluf_source_osu::SourceError;
use wolluf_source_osu::snapshot::{SnapshotPolicy, read_stable, read_stable_with};

fn recording_policy() -> (SnapshotPolicy, Arc<Mutex<Vec<Duration>>>) {
    let slept = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&slept);
    let policy = SnapshotPolicy {
        sleep: Arc::new(move |d| log.lock().unwrap().push(d)),
        ..SnapshotPolicy::default()
    };
    (policy, slept)
}

fn append(path: &Path, bytes: &[u8]) {
    File::options()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

#[test]
fn unchanged_file_single_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scores.db");
    fs::write(&path, b"stable bytes").unwrap();
    let (policy, slept) = recording_policy();
    let reads = AtomicUsize::new(0);
    let snap = read_stable_with(&path, &policy, |p| {
        reads.fetch_add(1, Ordering::SeqCst);
        fs::read(p)
    })
    .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert!(slept.lock().unwrap().is_empty());
    assert_eq!(snap.bytes, b"stable bytes");
    assert_eq!(snap.size, 12);
    assert_eq!(
        snap.sha256.0,
        <[u8; 32]>::from(Sha256::digest(b"stable bytes"))
    );
    assert_eq!(snap.mtime, fs::metadata(&path).unwrap().modified().unwrap());
    assert_eq!(
        read_stable(&path, &SnapshotPolicy::default())
            .unwrap()
            .bytes,
        snap.bytes
    );
}

#[test]
fn retries_when_size_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scores.db");
    fs::write(&path, b"v1").unwrap();
    let (policy, slept) = recording_policy();
    let reads = AtomicUsize::new(0);
    // osu! appends while we read on the first two attempts, then stops.
    let snap = read_stable_with(&path, &policy, |p| {
        let bytes = fs::read(p);
        if reads.fetch_add(1, Ordering::SeqCst) < 2 {
            append(p, b"+more");
        }
        bytes
    })
    .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), 3);
    assert_eq!(
        *slept.lock().unwrap(),
        vec![Duration::from_millis(250), Duration::from_millis(500)]
    );
    assert_eq!(snap.bytes, b"v1+more+more");
    assert_eq!(snap.size, snap.bytes.len() as u64);
}

#[test]
fn gives_up_after_3() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scores.db");
    fs::write(&path, b"v").unwrap();
    let (policy, slept) = recording_policy();
    let reads = AtomicUsize::new(0);
    let err = read_stable_with(&path, &policy, |p| {
        reads.fetch_add(1, Ordering::SeqCst);
        let bytes = fs::read(p);
        append(p, b"x");
        bytes
    })
    .unwrap_err();
    assert!(matches!(err, SourceError::Changing { .. }), "{err:?}");
    assert_eq!(err.code(), ErrorCode::OsuRunning);
    // The first read plus one retry after each of the 3 delays.
    assert_eq!(reads.load(Ordering::SeqCst), 4);
    assert_eq!(slept.lock().unwrap().len(), 3);
}

#[test]
fn missing_file_is_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let err = read_stable(&dir.path().join("nope.db"), &SnapshotPolicy::default()).unwrap_err();
    assert!(matches!(
        err,
        SourceError::Io {
            kind: std::io::ErrorKind::NotFound,
            ..
        }
    ));
}
