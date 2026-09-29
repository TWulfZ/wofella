//! Debounced watcher over `<root>/Data/r` and `<root>/scores.db` (spec 003 "Watcher"). Poll
//! mode exists because inotify on WSL drvfs does not see writes made by Windows processes.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use notify_debouncer_full::notify::event::{EventKind, MetadataKind, ModifyKind};
use notify_debouncer_full::notify::{self, PollWatcher, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, NoCache, RecommendedCache, new_debouncer, new_debouncer_opt,
};

use crate::error::SourceError;

const SCORES_DB: &str = "scores.db";
const DATA_DIR: &str = "Data";
const REPLAY_DIR: &str = "r";
/// Batches closer than `debounce / 2` are merged; see [`coalesce`].
const COALESCE_DIVISOR: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchMode {
    Native,
    Poll { interval: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceChange {
    ScoresDb,
    ReplayDir,
}

enum Inner {
    Native(Debouncer<RecommendedWatcher, RecommendedCache>),
    Poll(Debouncer<PollWatcher, NoCache>),
}

/// Dropping the handle stops watching; [`WatchHandle::stop`] also waits for the event thread.
pub struct WatchHandle {
    inner: Inner,
}

impl WatchHandle {
    pub fn stop(self) {
        match self.inner {
            Inner::Native(d) => d.stop(),
            Inner::Poll(d) => d.stop(),
        }
    }
}

impl std::fmt::Debug for WatchHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match self.inner {
            Inner::Native(_) => "native",
            Inner::Poll(_) => "poll",
        };
        f.debug_struct("WatchHandle").field("mode", &mode).finish()
    }
}

/// Paths the watcher cares about; everything else under the root is ignored.
#[derive(Debug, Clone)]
struct Targets {
    root: PathBuf,
    replay_dir: PathBuf,
}

impl Targets {
    fn classify(&self, path: &Path) -> Option<SourceChange> {
        let parent = path.parent()?;
        if parent == self.replay_dir {
            return Some(SourceChange::ReplayDir);
        }
        let is_scores = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(SCORES_DB));
        (parent == self.root && is_scores).then_some(SourceChange::ScoresDb)
    }
}

/// Our own reads during a sync show up as access events; reacting to them would loop.
fn is_write_like(kind: &EventKind) -> bool {
    !matches!(
        kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime))
    )
}

fn handler(targets: Targets, tx: Sender<SourceChange>) -> impl FnMut(DebounceEventResult) + Send {
    move |result| match result {
        Ok(events) => {
            let kinds: BTreeSet<SourceChange> = events
                .iter()
                .filter(|e| is_write_like(&e.kind))
                .flat_map(|e| e.paths.iter().filter_map(|p| targets.classify(p)))
                .collect();
            for kind in kinds {
                // The coalescer only disappears at shutdown; nothing is left to notify then.
                let _ = tx.send(kind);
            }
        }
        Err(errors) => {
            for e in errors {
                tracing::warn!(error = %e, "watch: notify error");
            }
        }
    }
}

/// debouncer-full expires each path on its own clock, so one burst that straddles two poll
/// scans comes out as two batches a tick apart. Merging batches that arrive within half the
/// debounce keeps "one burst → one message per kind" (spec 003 AC15) at the cost of that much
/// extra latency.
fn coalesce(raw: Receiver<SourceChange>, out: Sender<SourceChange>, quiet: Duration) {
    thread::spawn(move || {
        while let Ok(first) = raw.recv() {
            let mut pending = BTreeSet::from([first]);
            let disconnected = loop {
                match raw.recv_timeout(quiet) {
                    Ok(kind) => {
                        pending.insert(kind);
                    }
                    Err(RecvTimeoutError::Timeout) => break false,
                    Err(RecvTimeoutError::Disconnected) => break true,
                }
            };
            for kind in pending {
                // A dropped receiver means the app is shutting down; nothing to report to.
                let _ = out.send(kind);
            }
            if disconnected {
                return;
            }
        }
    });
}

fn watch_err(path: &Path, e: &notify::Error) -> SourceError {
    SourceError::Watch {
        path: path.to_path_buf(),
        detail: e.to_string(),
    }
}

/// Watches `<root>` (non-recursive, filtered to `scores.db`) and `<root>/Data/r`
/// (non-recursive). A missing `Data/r` is skipped: a fresh install gets it on its first saved
/// replay, and the next app start picks it up.
pub fn spawn(
    root: &Path,
    mode: WatchMode,
    debounce: Duration,
) -> Result<(WatchHandle, Receiver<SourceChange>), SourceError> {
    let targets = Targets {
        root: root.to_path_buf(),
        replay_dir: root.join(DATA_DIR).join(REPLAY_DIR),
    };
    let (raw_tx, raw_rx) = mpsc::channel();
    let (tx, rx) = mpsc::channel();
    coalesce(raw_rx, tx, debounce / COALESCE_DIVISOR);
    let on_event = handler(targets.clone(), raw_tx);
    let mut inner = match mode {
        WatchMode::Native => {
            Inner::Native(new_debouncer(debounce, None, on_event).map_err(|e| watch_err(root, &e))?)
        }
        WatchMode::Poll { interval } => Inner::Poll(
            new_debouncer_opt::<_, PollWatcher, NoCache>(
                debounce,
                None,
                on_event,
                NoCache,
                notify::Config::default().with_poll_interval(interval),
            )
            .map_err(|e| watch_err(root, &e))?,
        ),
    };
    let mut watch = |path: &Path| match &mut inner {
        Inner::Native(d) => d.watch(path, RecursiveMode::NonRecursive),
        Inner::Poll(d) => d.watch(path, RecursiveMode::NonRecursive),
    };
    watch(&targets.root).map_err(|e| watch_err(&targets.root, &e))?;
    if targets.replay_dir.is_dir() {
        watch(&targets.replay_dir).map_err(|e| watch_err(&targets.replay_dir, &e))?;
    } else {
        tracing::debug!("watch: Data/r missing, watching scores.db only");
    }
    Ok((WatchHandle { inner }, rx))
}
