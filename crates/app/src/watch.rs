//! Turns osu! writes into `SyncPlays` submits (spec 003 "Watcher").

use std::path::Path;
use std::thread::JoinHandle;
use std::time::Duration;

use wolluf_source_osu::paths::is_drvfs_path;
use wolluf_source_osu::watch::{self as source_watch, SourceChange, WatchHandle, WatchMode};

use crate::context::{AppContext, InstallId, blocking_join_error};
use crate::errors::AppError;
use crate::features::plays::SyncPlaysJob;
use crate::jobs::JobSubmitter;

/// Spec 003: osu! writes the replay, the `.osg` and scores.db within a few seconds of each
/// other; 5 s folds them into one sync.
const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(5);
/// Spec 003: drvfs poll interval. Shorter only costs directory scans over the 9P bridge.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchParams {
    pub debounce: Duration,
    pub poll_interval: Duration,
}

impl Default for WatchParams {
    fn default() -> Self {
        Self {
            debounce: DEFAULT_DEBOUNCE,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

/// inotify on WSL drvfs (`/mnt/<letter>/…`) does not see writes made by Windows processes,
/// which is every write osu! makes, so those roots are polled.
pub fn watch_mode(root: &Path, params: &WatchParams) -> WatchMode {
    if is_drvfs_path(root) {
        WatchMode::Poll {
            interval: params.poll_interval,
        }
    } else {
        WatchMode::Native
    }
}

/// Watching lasts as long as this value; dropping it stops the watcher and its forwarder.
pub struct InstallWatcher {
    handle: Option<WatchHandle>,
    forwarder: Option<JoinHandle<()>>,
    mode: WatchMode,
}

impl std::fmt::Debug for InstallWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallWatcher")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl InstallWatcher {
    /// Every debounced change of `scores.db` or `Data/r` submits `SyncPlays(install_id)`; the
    /// runner's coalescing turns a burst during a running sync into one trailing re-run.
    pub async fn start(
        ctx: &AppContext,
        install_id: InstallId,
        params: WatchParams,
    ) -> Result<Self, AppError> {
        let install = ctx
            .installs()
            .await?
            .into_iter()
            .find(|i| i.id == install_id)
            .ok_or_else(|| AppError::not_found().with_arg("installId", install_id.0.to_string()))?;
        let mode = watch_mode(&install.root_path, &params);
        // A poll watcher scans the tree when it starts: over drvfs with thousands of replays that
        // would stall a runtime worker.
        let root = install.root_path.clone();
        let (handle, rx) =
            tokio::task::spawn_blocking(move || source_watch::spawn(&root, mode, params.debounce))
                .await
                .map_err(blocking_join_error)??;
        let submitter = ctx.jobs().submitter();
        let forwarder = std::thread::Builder::new()
            .name("wolluf-watch".into())
            .spawn(move || forward(&rx, &submitter, install_id))
            .map_err(|e| AppError::internal(format!("watch thread: {e}")))?;
        tracing::info!(install = install_id.0, ?mode, "watching osu! install");
        Ok(Self {
            handle: Some(handle),
            forwarder: Some(forwarder),
            mode,
        })
    }

    pub fn mode(&self) -> WatchMode {
        self.mode
    }
}

impl Drop for InstallWatcher {
    fn drop(&mut self) {
        // Stopping the source watcher closes the channel, which ends the forwarder.
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
        if let Some(forwarder) = self.forwarder.take() {
            let _ = forwarder.join();
        }
    }
}

fn forward(
    rx: &std::sync::mpsc::Receiver<SourceChange>,
    submitter: &JobSubmitter,
    install_id: InstallId,
) {
    while let Ok(change) = rx.recv() {
        match submitter.submit(Box::new(SyncPlaysJob::new(install_id))) {
            Some(job) => tracing::debug!(?change, %job, "osu! change, sync submitted"),
            None => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use wolluf_source_osu::testkit::{FakeInstall, ScoreBuilder};

    use super::*;
    use crate::events::AppEvent;
    use crate::features::plays::testkit::{Fixture, md5_hex, scores_db};
    use crate::jobs::JobKindDto;

    const WAIT: Duration = Duration::from_secs(30);

    #[test]
    fn drvfs_root_selects_poll_mode() {
        let params = WatchParams::default();
        assert_eq!(
            watch_mode(Path::new("/mnt/e/Games/osu!"), &params),
            WatchMode::Poll {
                interval: DEFAULT_POLL_INTERVAL
            }
        );
        assert_eq!(
            watch_mode(Path::new("/home/p/.local/share/osu!"), &params),
            WatchMode::Native
        );
        // A directory merely named `mnt` elsewhere is a normal Linux path.
        assert_eq!(
            watch_mode(Path::new("/srv/mnt/e/osu!"), &params),
            WatchMode::Native
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn change_submits_sync_plays() {
        let md5 = md5_hex(b"watched");
        let f = Fixture::new(&FakeInstall::new()).await;
        let params = WatchParams {
            debounce: Duration::from_millis(200),
            poll_interval: Duration::from_millis(50),
        };
        let watcher = InstallWatcher::start(&f.ctx, f.install, params)
            .await
            .unwrap();
        let mut rx = f.ctx.subscribe();
        f.write(
            "scores.db",
            &scores_db(&[ScoreBuilder::mania(&md5, "TWulfZ", 1)]),
        );
        let finished = tokio::time::timeout(WAIT, async {
            loop {
                if let AppEvent::JobFinished(fin) = rx.recv().await.unwrap() {
                    return fin;
                }
            }
        })
        .await
        .expect("the change submitted a sync");
        let job = f
            .ctx
            .jobs()
            .list(None)
            .await
            .unwrap()
            .into_iter()
            .find(|j| j.id == finished.job_id)
            .unwrap();
        assert_eq!(job.kind, JobKindDto::SyncPlays);
        let plays = f
            .ctx
            .user_db()
            .read(wolluf_store::repo::ledger::play::count)
            .unwrap();
        assert_eq!(plays, 1, "the sync saw the new score");
        drop(watcher);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unknown_install_is_not_found() {
        let f = Fixture::new(&FakeInstall::new()).await;
        let err = InstallWatcher::start(&f.ctx, InstallId(f.install.0 + 1), WatchParams::default())
            .await
            .unwrap_err();
        assert_eq!(err.code, wolluf_core::ErrorCode::NotFound);
    }
}
