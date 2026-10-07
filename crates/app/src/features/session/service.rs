//! `SessionService`: the session feature's only entry point for shells and other features (D12).

use std::collections::BTreeSet;

use tokio::sync::broadcast::{self, error::RecvError};
use wolluf_core::{ChartMd5, Keymode, PlayId, UnixUs};
use wolluf_store::DbHandle;
use wolluf_store::repo::ledger::Play;
use wolluf_store::time::format_rfc3339_ms;

use super::SessionParams;
use super::dto::{SessionPlayDto, SessionPlaysDto};
use crate::context::{AppContext, InstallId, blocking_join_error};
use crate::errors::AppError;
use crate::events::{AppEvent, SessionPlayAddedDto};
use crate::features::{labeling, library, players, plays, settings};
use crate::watch::InstallWatcher;

/// Query-key roots whose change can add a session play: a sync adds plays, an identity refresh
/// can make an existing alias a self alias.
const DOMAIN_PLAYS: &str = "plays";
const DOMAIN_PLAYERS: &str = "players";

pub struct SessionService<'a> {
    ctx: &'a AppContext,
}

impl<'a> SessionService<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub fn started_at(&self) -> UnixUs {
        self.ctx.session_state().started_at
    }

    /// Self plays (ADR 0005) of charts the catalog lists under `keymode`, saved since the app
    /// started, newest first. Empty until a self profile exists.
    pub async fn plays(&self, keymode: u8) -> Result<SessionPlaysDto, AppError> {
        let keymode = Keymode::new(keymode)
            .map_err(|_| AppError::invalid_input().with_arg("keymode", keymode.to_string()))?;
        let since = self.started_at();
        let plays = self.self_plays_since(since).await?;
        let charts: BTreeSet<ChartMd5> = plays.iter().map(|p| p.chart_md5).collect();
        let catalog = self
            .ctx
            .library()
            .catalog_charts(charts.into_iter().collect())
            .await?;
        let labels = self.ctx.labeling().session_label_states().await?;
        let gold_windows = self.ctx.labeling().gold_windows().await?;
        let rows = plays
            .into_iter()
            .filter_map(|p| {
                let chart = catalog
                    .get(&p.chart_md5)
                    .filter(|c| c.keymode == keymode.columns())?;
                let md5 = p.chart_md5.to_string();
                Some(SessionPlayDto {
                    play_id: p.id.to_string(),
                    played_at: format_rfc3339_ms(p.played_at),
                    title: chart.title.clone(),
                    artist: chart.artist.clone(),
                    version: chart.version.clone(),
                    creator: chart.creator.clone(),
                    // The library DTOs carry stars as f32 too; the f64 is only osu!.db's width.
                    stars: chart.stars.map(|s| s as f32),
                    keymode: chart.keymode,
                    set_id: chart.set_id,
                    label: labels.get(&md5).cloned(),
                    gold_windows: gold_windows.get(&md5).copied().unwrap_or(0),
                    md5,
                })
            })
            .collect();
        Ok(SessionPlaysDto {
            started_at: format_rfc3339_ms(since),
            plays: rows,
        })
    }

    /// Starts telling the bus about new session plays (`SessionPlayAdded`, and
    /// `AttentionRequested` when the user opted in). Plays already in the ledger are not
    /// announced. Idempotent.
    pub async fn track(&self) -> Result<(), AppError> {
        let state = self.ctx.session_state();
        let epoch = {
            let live = state.live();
            if live.tracker.is_some() {
                return Ok(());
            }
            live.epoch
        };
        // Subscribed before the seed is read, so a sync finishing in between is not missed.
        let rx = self.ctx.subscribe();
        let since = self.started_at();
        let seed = self.self_plays_since(since).await?;
        let tracker = Tracker {
            user: self.ctx.user_db().clone(),
            cache: self.ctx.cache_db().clone(),
            events: self.ctx.event_sender(),
            since,
            seen: seed.iter().map(|p| p.id).collect(),
            listed: seed.iter().map(|p| p.chart_md5).collect(),
        };
        let task = self.ctx.runtime().spawn(tracker.run(rx)).abort_handle();
        state.adopt_tracker(epoch, task);
        Ok(())
    }

    /// Tracks, and watches the selected install (the newest registered one, as setup reports
    /// it) until [`Self::stop`] or the context closes. Idempotent.
    pub async fn start(&self, params: SessionParams) -> Result<(), AppError> {
        self.ctx.session_state().live().params = Some(params);
        self.track().await?;
        self.follow_selected_install().await
    }

    /// Re-points the watcher at the selected install after setup changed it; a no-op unless the
    /// session was started.
    pub async fn follow_selected_install(&self) -> Result<(), AppError> {
        let state = self.ctx.session_state();
        let _following = state.following.lock().await;
        let (params, epoch) = {
            let live = state.live();
            let Some(params) = live.params else {
                return Ok(());
            };
            (params, live.epoch)
        };
        let selected = self.ctx.installs().await?.into_iter().map(|i| i.id).max();
        if self.watching() == selected {
            return Ok(());
        }
        let watcher = match selected {
            Some(id) => Some((id, InstallWatcher::start(self.ctx, id, params.watch).await?)),
            None => None,
        };
        drop_watcher(state.adopt_watcher(epoch, watcher)).await;
        Ok(())
    }

    /// Self plays (ADR 0005) saved at or after `since`, newest first.
    async fn self_plays_since(&self, since: UnixUs) -> Result<Vec<Play>, AppError> {
        let aliases = self.ctx.players().self_alias_ids().await?;
        self.ctx.plays().since(&aliases, since).await
    }

    pub fn watching(&self) -> Option<InstallId> {
        self.ctx
            .session_state()
            .live()
            .watcher
            .as_ref()
            .map(|(id, _)| *id)
    }

    /// Stops the tracker and the watcher; the session's plays stay listed. Idempotent.
    pub async fn stop(&self) {
        drop_watcher(self.ctx.session_state().stop()).await;
    }
}

/// Dropping a watcher joins its threads, which must not stall a runtime worker.
async fn drop_watcher(watcher: Option<InstallWatcher>) {
    if let Some(watcher) = watcher
        && let Err(e) = tokio::task::spawn_blocking(move || drop(watcher)).await
    {
        tracing::warn!(error = %e, "stopping the install watcher failed");
    }
}

/// Owns clones rather than the context, so it never keeps the context alive; the context
/// aborts it on close.
struct Tracker {
    user: DbHandle,
    cache: DbHandle,
    events: broadcast::Sender<AppEvent>,
    since: UnixUs,
    /// Plays already announced, or present when tracking began.
    seen: BTreeSet<PlayId>,
    /// Charts of `seen`: a retry of one adds no map to label, so it never flashes.
    listed: BTreeSet<ChartMd5>,
}

impl Tracker {
    async fn run(mut self, mut rx: broadcast::Receiver<AppEvent>) {
        loop {
            match rx.recv().await {
                Ok(AppEvent::DataChanged(d))
                    if d.domains
                        .iter()
                        .any(|d| d == DOMAIN_PLAYS || d == DOMAIN_PLAYERS) => {}
                Ok(_) => continue,
                // A lost `DataChanged` is recovered by diffing now.
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            }
            if let Err(e) = self.announce_new().await {
                tracing::warn!(
                    code = e.code.as_str(),
                    details = e.details.as_deref(),
                    "session diff failed"
                );
            }
        }
    }

    async fn announce_new(&mut self) -> Result<(), AppError> {
        let (user, since) = (self.user.clone(), self.since);
        // `answered` is `Some` only when notifying: the flash is the only thing that needs it.
        let (plays, answered) = tokio::task::spawn_blocking(move || {
            user.read(|c| {
                let aliases = players::self_alias_ids(c)?;
                let answered = if settings::session_notify(c)? {
                    Some(labeling::answered_charts(c)?)
                } else {
                    None
                };
                Ok((plays::plays_since(c, &aliases, since)?, answered))
            })
        })
        .await
        .map_err(blocking_join_error)??;
        let mut fresh = BTreeSet::new();
        // Oldest first, so the UI's newest-first list grows at the top in play order.
        for p in plays.iter().rev() {
            if self.seen.insert(p.id) {
                // Every new play is announced: the UI only refetches on it.
                let _ = self
                    .events
                    .send(AppEvent::SessionPlayAdded(SessionPlayAddedDto {
                        play_id: p.id.to_string(),
                        md5: p.chart_md5.to_string(),
                    }));
                if let Some(answered) = &answered
                    && !self.listed.contains(&p.chart_md5)
                    && !answered.contains(&p.chart_md5)
                {
                    fresh.insert(p.chart_md5);
                }
            }
        }
        self.listed.extend(plays.iter().map(|p| p.chart_md5));
        if !fresh.is_empty() && self.lists_one_to_answer(fresh).await? {
            let _ = self.events.send(AppEvent::AttentionRequested);
        }
        Ok(())
    }

    /// Whether any of `charts` shows in a session list as a map to answer: osu!.db lists it, in
    /// a keymode with a taxonomy (ADR 0020).
    async fn lists_one_to_answer(&self, charts: BTreeSet<ChartMd5>) -> Result<bool, AppError> {
        let cache = self.cache.clone();
        let keymodes = tokio::task::spawn_blocking(move || {
            cache.read(|c| library::catalog_keymodes(c, &charts))
        })
        .await
        .map_err(blocking_join_error)??;
        Ok(keymodes.into_values().any(labeling::answerable))
    }
}
