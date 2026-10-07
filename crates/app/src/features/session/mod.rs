//! The play session (ADR 0020): self plays saved since the app started, the tracker that tells
//! the UI about new ones, and the install watcher that makes osu! writes trigger a sync.

pub mod dto;
mod service;

use std::sync::{Mutex, PoisonError};

use tokio::task::AbortHandle;
use wolluf_core::UnixUs;

use crate::context::InstallId;
use crate::watch::{InstallWatcher, WatchParams};

pub use service::SessionService;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionParams {
    pub watch: WatchParams,
}

/// Owned by the context: the session starts with it and its live parts stop with it.
pub(crate) struct SessionState {
    started_at: UnixUs,
    live: Mutex<Live>,
    /// Held across a whole follow, so setup re-pointing the watcher while the startup follow is
    /// still starting one cannot leave the older install watched. `stop` never takes it: closing
    /// the app must not wait for a slow drvfs watcher start.
    following: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct Live {
    /// `Some` once a shell started the session; until then nothing is watched.
    params: Option<SessionParams>,
    tracker: Option<AbortHandle>,
    watcher: Option<(InstallId, InstallWatcher)>,
    /// Bumped by every stop: a tracker or watcher whose start began in an older epoch arrives
    /// after a stop and is dropped instead of kept.
    epoch: u64,
}

impl SessionState {
    pub(crate) fn new(started_at: UnixUs) -> Self {
        Self {
            started_at,
            live: Mutex::new(Live::default()),
            following: tokio::sync::Mutex::new(()),
        }
    }

    fn live(&self) -> std::sync::MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[cfg(test)]
    pub(crate) fn epoch(&self) -> u64 {
        self.live().epoch
    }

    /// Keeps `task` unless a stop came after `epoch` or a tracker is already kept; otherwise
    /// aborts it. True when kept.
    pub(crate) fn adopt_tracker(&self, epoch: u64, task: AbortHandle) -> bool {
        let mut live = self.live();
        if live.epoch != epoch || live.tracker.is_some() {
            task.abort();
            return false;
        }
        live.tracker = Some(task);
        true
    }

    /// Swaps in `next` unless a stop came after `epoch`. Returns the watcher to drop: the one
    /// replaced, or `next` itself when refused.
    pub(crate) fn adopt_watcher(
        &self,
        epoch: u64,
        next: Option<(InstallId, InstallWatcher)>,
    ) -> Option<InstallWatcher> {
        let mut live = self.live();
        if live.epoch != epoch {
            return next.map(|(_, w)| w);
        }
        std::mem::replace(&mut live.watcher, next).map(|(_, w)| w)
    }

    /// Aborts the tracker and hands back the watcher, whose drop joins its threads; the caller
    /// decides where that blocking drop runs. Idempotent.
    pub(crate) fn stop(&self) -> Option<InstallWatcher> {
        let mut live = self.live();
        live.params = None;
        live.epoch = live.epoch.wrapping_add(1);
        if let Some(tracker) = live.tracker.take() {
            tracker.abort();
        }
        live.watcher.take().map(|(_, w)| w)
    }
}

#[cfg(test)]
mod tests;
