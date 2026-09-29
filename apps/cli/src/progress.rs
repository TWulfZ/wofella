//! One rewritten stderr line while a job runs (spec 005). Only on a terminal: pipes and CI logs
//! get nothing. The runner already throttles progress events, so every event is drawn.

use std::io::{IsTerminal, Write};

use wolluf_app::events::JobProgressDto;

use crate::render;

const PERCENT: u64 = 100;
const MS_PER_SECOND: u32 = 1000;

pub(crate) struct Progress {
    enabled: bool,
    drawn_len: usize,
}

impl Progress {
    pub(crate) fn stderr() -> Self {
        Self {
            enabled: std::io::stderr().is_terminal(),
            drawn_len: 0,
        }
    }

    pub(crate) fn update(&mut self, p: &JobProgressDto) {
        if !self.enabled {
            return;
        }
        let text = line(p);
        let pad = self.drawn_len.saturating_sub(text.chars().count());
        let mut err = std::io::stderr().lock();
        // Progress is cosmetic; a broken stderr must not fail the job.
        let _ = write!(err, "\r{text}{}", " ".repeat(pad));
        let _ = err.flush();
        self.drawn_len = text.chars().count();
    }

    /// Clears the line so the summary starts on a clean row.
    pub(crate) fn finish(&mut self) {
        if !self.enabled || self.drawn_len == 0 {
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "\r{}\r", " ".repeat(self.drawn_len));
        let _ = err.flush();
        self.drawn_len = 0;
    }
}

pub(crate) fn line(p: &JobProgressDto) -> String {
    let mut text = format!(
        "{} {} {}/{}",
        p.kind.as_str(),
        render::wire(&p.stage),
        p.done,
        p.total
    );
    if p.total > 0 {
        let pct = u64::from(p.done) * PERCENT / u64::from(p.total);
        text.push_str(&format!(" ({pct}%)"));
    }
    if let Some(eta) = p.eta_ms {
        text.push_str(&format!(" eta {}s", eta.div_ceil(MS_PER_SECOND)));
    }
    text
}

#[cfg(test)]
mod tests {
    use wolluf_app::jobs::{JobId, JobKindDto, JobStageDto};

    use super::*;

    fn progress(done: u32, total: u32, eta_ms: Option<u32>) -> JobProgressDto {
        JobProgressDto {
            job_id: JobId("01J".to_owned()),
            kind: JobKindDto::SyncPlays,
            stage: JobStageDto::Ingest,
            done,
            total,
            eta_ms,
        }
    }

    #[test]
    fn line_shows_stage_count_percent_and_eta() {
        assert_eq!(
            line(&progress(120, 4338, Some(12_001))),
            "sync_plays ingest 120/4338 (2%) eta 13s"
        );
        assert_eq!(line(&progress(0, 0, None)), "sync_plays ingest 0/0");
    }
}
