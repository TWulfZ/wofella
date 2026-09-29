//! Throttled job progress (spec 003: ≤ 10 Hz per job, the last update always delivered).

use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::broadcast;
use wolluf_core::{Clock, UnixUs};

use crate::events::{AppEvent, JobProgressDto};
use crate::jobs::dto::{JobId, JobKindDto, JobStageDto};

/// 10 Hz: faster updates only cost IPC and re-renders without informing anyone (§8).
const MIN_INTERVAL_US: i64 = 100_000;
const US_PER_MS: i64 = 1_000;

struct State {
    stage: Option<JobStageDto>,
    stage_started: UnixUs,
    last_emit: Option<UnixUs>,
    pending: Option<JobProgressDto>,
}

/// Cheap to clone; every clone throttles against the same per-job state, so rayon workers can
/// report concurrently.
#[derive(Clone)]
pub struct ProgressSink {
    job_id: JobId,
    kind: JobKindDto,
    clock: Arc<dyn Clock>,
    events: broadcast::Sender<AppEvent>,
    state: Arc<Mutex<State>>,
}

impl ProgressSink {
    pub fn new(
        job_id: JobId,
        kind: JobKindDto,
        clock: Arc<dyn Clock>,
        events: broadcast::Sender<AppEvent>,
    ) -> Self {
        Self {
            job_id,
            kind,
            clock,
            events,
            state: Arc::new(Mutex::new(State {
                stage: None,
                stage_started: UnixUs(0),
                last_emit: None,
                pending: None,
            })),
        }
    }

    /// A stage change and `done == total` always go out; anything else within 100 ms of the
    /// previous event is held as pending and superseded by the next report.
    pub fn report(&self, stage: JobStageDto, done: u32, total: u32) {
        let now = self.clock.now();
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let stage_changed = state.stage != Some(stage);
        if stage_changed {
            state.stage = Some(stage);
            state.stage_started = now;
        }
        let dto = JobProgressDto {
            job_id: self.job_id.clone(),
            kind: self.kind,
            stage,
            done,
            total,
            eta_ms: eta_ms(now.0 - state.stage_started.0, done, total),
        };
        let due = state
            .last_emit
            .is_none_or(|last| now.0 - last.0 >= MIN_INTERVAL_US);
        if stage_changed || due || done >= total {
            state.last_emit = Some(now);
            state.pending = None;
            drop(state);
            // No subscriber is not an error: the CLI may run without progress output.
            let _ = self.events.send(AppEvent::JobProgress(dto));
        } else {
            state.pending = Some(dto);
        }
    }

    /// Sends a held update, so the last state reported is the last one the UI sees.
    pub fn flush(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(dto) = state.pending.take() {
            state.last_emit = Some(self.clock.now());
            drop(state);
            let _ = self.events.send(AppEvent::JobProgress(dto));
        }
    }
}

/// Linear extrapolation over the current stage; `None` until one item is done.
fn eta_ms(elapsed_us: i64, done: u32, total: u32) -> Option<u32> {
    if done == 0 || done >= total || elapsed_us <= 0 {
        return None;
    }
    let remaining = i64::from(total - done);
    let eta_us = elapsed_us.saturating_mul(remaining) / i64::from(done);
    u32::try_from(eta_us / US_PER_MS).ok()
}

#[cfg(test)]
mod tests {
    use wolluf_core::FixedClock;

    use super::*;

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);

    fn sink(clock: Arc<FixedClock>) -> (ProgressSink, broadcast::Receiver<AppEvent>) {
        let (tx, rx) = broadcast::channel(4_096);
        let sink = ProgressSink::new(JobId("j".into()), JobKindDto::SyncPlays, clock, tx);
        (sink, rx)
    }

    fn drain(rx: &mut broadcast::Receiver<AppEvent>) -> Vec<JobProgressDto> {
        let mut out = Vec::new();
        while let Ok(AppEvent::JobProgress(p)) = rx.try_recv() {
            out.push(p);
        }
        out
    }

    #[test]
    fn progress_at_most_10hz() {
        let clock = Arc::new(FixedClock::new(T0));
        let (sink, mut rx) = sink(clock.clone());
        const ITEMS: u32 = 1_000;
        // 1,000 items in 50 ms: 50 µs per item.
        for i in 1..=ITEMS {
            clock.advance(50);
            sink.report(JobStageDto::Ingest, i, ITEMS);
        }
        sink.flush();
        let events = drain(&mut rx);
        assert!(events.len() <= 2, "{} events", events.len());
        assert_eq!(events.last().map(|e| e.done), Some(ITEMS));
    }

    #[test]
    fn pending_update_flushed_and_stage_change_immediate() {
        let clock = Arc::new(FixedClock::new(T0));
        let (sink, mut rx) = sink(clock.clone());
        sink.report(JobStageDto::Catalog, 1, 10);
        clock.advance(10_000);
        sink.report(JobStageDto::Catalog, 5, 10);
        assert_eq!(drain(&mut rx).len(), 1, "second report is held");
        sink.flush();
        let flushed = drain(&mut rx);
        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].done, 5);
        // 10 ms elapsed for 5 of 10 items → about 10 ms left.
        assert_eq!(flushed[0].eta_ms, Some(10));
        clock.advance(1_000);
        sink.report(JobStageDto::Ingest, 0, 3);
        assert_eq!(drain(&mut rx).len(), 1, "a stage change is never throttled");
        clock.advance(MIN_INTERVAL_US);
        sink.report(JobStageDto::Ingest, 1, 3);
        assert_eq!(
            drain(&mut rx).len(),
            1,
            "100 ms later the next update is due"
        );
    }
}
