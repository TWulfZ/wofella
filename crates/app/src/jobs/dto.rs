//! Job DTOs (spec 003 IPC). camelCase on the wire, snake_case enum values, no 64-bit integers
//! (the bindings export fails on them, spec 005).

use serde::{Deserialize, Serialize};

use crate::errors::ErrorCodeDto;

/// A ULID: sortable by creation time and above 2^53, so it crosses IPC as a string (§8).
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(transparent)]
pub struct JobId(pub String);

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Persisted in `job_run.kind`: stable strings, never renumbered (§11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum JobKindDto {
    SyncPlays,
    /// Chained after every `SyncPlays` (spec 004); never started from IPC.
    RefreshIdentity,
    /// Chained after every `SyncPlays` too, and startable on its own.
    IndexLibrary,
    /// Chained after every `IndexLibrary` (ADR 0024); never started from IPC.
    ComputePlaySsr,
    /// Started only by `rate_copy_confirm`, which mints its `ExportPermit` (ADR 0025, D9).
    RateCopy,
}

impl JobKindDto {
    pub const ALL: &'static [Self] = &[
        Self::SyncPlays,
        Self::RefreshIdentity,
        Self::IndexLibrary,
        Self::ComputePlaySsr,
        Self::RateCopy,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SyncPlays => "sync_plays",
            Self::RefreshIdentity => "refresh_identity",
            Self::IndexLibrary => "index_library",
            Self::ComputePlaySsr => "compute_play_ssr",
            Self::RateCopy => "rate_copy",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum JobStatusDto {
    Queued,
    Running,
    Ok,
    Failed,
    Cancelled,
}

/// The step a job is in; the UI localises it (`jobs.stage.<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum JobStageDto {
    Catalog,
    Ingest,
    Archive,
    Index,
    /// Per-play goals and SSRs for the skill preview.
    PlaySsr,
    /// Time-stretching the chart audio for a rate copy.
    RenderAudio,
    /// Writing a rate copy's files into the set folder.
    Write,
}

/// `JobService::start` input, tagged on `kind` (spec 003 IPC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobStartDto {
    SyncPlays(SyncPlaysStartDto),
    /// Indexes the charts of the current catalog, whichever install it came from.
    IndexLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlaysStartDto {
    pub install_id: u32,
}

/// SyncPlays step-4 counters (spec 003 Behaviour, "Finish").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummaryDto {
    pub plays_new: u32,
    pub plays_replay_only: u32,
    pub plays_existing: u32,
    pub conflicts: u32,
    pub skipped_non_mania: u32,
    pub replays_linked: u32,
    pub osg_linked: u32,
    pub charts_archived: u32,
    pub chart_unavailable: u32,
    pub chart_md5_mismatch: u32,
    pub orphan_replays: u32,
    pub failed_items: u32,
}

/// IndexLibrary counters. `failedItems` keeps the name every summary shares, which the job
/// tray reads without knowing the kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexLibrarySummaryDto {
    /// Catalog charts of a keymode with an engine profile.
    pub charts_total: u32,
    pub parsed_new: u32,
    /// Already parsed, or already failed, under the current `chart_parse` key.
    pub skipped_memoized: u32,
    /// Missing from `Songs/` or edited since osu!.db recorded its md5; retried next run.
    pub skipped_unavailable: u32,
    pub labels_written: u32,
    /// Pattern segments stored this run (`patterns` stage).
    pub segments_written: u32,
    /// Charts given an MSD status this run (`difficulty` stage), rated or not.
    pub msd_written: u32,
    pub failed_items: u32,
}

/// ComputePlaySsr counters (ADR 0024).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ComputePlaySsrSummaryDto {
    /// Ledger plays, of every alias, on a catalog chart of a keymode with a calculator.
    pub plays_total: u32,
    /// Rows written this run, counted or excluded.
    pub computed: u32,
    pub counted: u32,
    pub excluded: u32,
    /// Already had a row under the current `play_ssr` key.
    pub skipped_memoized: u32,
    pub failed_items: u32,
}

/// What a rate copy wrote into its set folder (ADR 0025).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RateCopySummaryDto {
    pub folder: String,
    pub osu_filename: String,
    pub audio_filename: String,
    /// `false`: a file of that name was already there and was left untouched.
    pub osu_written: bool,
    pub audio_written: bool,
    /// The audio at that rate already existed, so none was rendered.
    pub audio_reused: bool,
    /// [`RATE_COPY_NEXT_STEP`]: the catalog comes from osu!.db, so the copy shows up in wofella
    /// only after stable refreshes (F5 in song select) and a sync reads it back.
    pub next_step: String,
    pub failed_items: u32,
}

/// The UI localises it (`jobs.next_step.<id>`).
pub const RATE_COPY_NEXT_STEP: &str = "refresh_osu_then_sync";

/// Per-kind result, stored as `job_run.summary_json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", content = "counters", rename_all = "snake_case")]
pub enum JobSummaryDto {
    SyncPlays(SyncSummaryDto),
    IndexLibrary(IndexLibrarySummaryDto),
    ComputePlaySsr(ComputePlaySsrSummaryDto),
    RateCopy(RateCopySummaryDto),
}

/// Why a job ended `failed` or `cancelled`; the UI localises `messageKey` (§7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JobErrorDto {
    pub code: ErrorCodeDto,
    pub message_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JobDto {
    pub id: JobId,
    pub kind: JobKindDto,
    pub status: JobStatusDto,
    /// RFC 3339 UTC with milliseconds.
    pub started: Option<String>,
    pub ended: Option<String>,
    pub summary: Option<JobSummaryDto>,
    pub error: Option<JobErrorDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_shapes() {
        let start: JobStartDto =
            serde_json::from_str(r#"{"kind":"sync_plays","installId":3}"#).unwrap();
        assert_eq!(
            start,
            JobStartDto::SyncPlays(SyncPlaysStartDto { install_id: 3 })
        );
        let summary = JobSummaryDto::SyncPlays(SyncSummaryDto {
            plays_new: 6,
            failed_items: 1,
            ..SyncSummaryDto::default()
        });
        let json = serde_json::to_string(&summary).unwrap();
        assert!(
            json.starts_with(r#"{"kind":"sync_plays","counters":{"playsNew":6,"#),
            "{json}"
        );
        assert!(json.contains(r#""failedItems":1"#), "{json}");
        assert_eq!(
            serde_json::to_string(&JobStatusDto::Cancelled).unwrap(),
            r#""cancelled""#
        );
        let start: JobStartDto = serde_json::from_str(r#"{"kind":"index_library"}"#).unwrap();
        assert_eq!(start, JobStartDto::IndexLibrary);
        let summary = JobSummaryDto::IndexLibrary(IndexLibrarySummaryDto {
            charts_total: 3,
            parsed_new: 2,
            skipped_unavailable: 1,
            ..IndexLibrarySummaryDto::default()
        });
        assert_eq!(
            serde_json::to_string(&summary).unwrap(),
            r#"{"kind":"index_library","counters":{"chartsTotal":3,"parsedNew":2,"skippedMemoized":0,"skippedUnavailable":1,"labelsWritten":0,"segmentsWritten":0,"msdWritten":0,"failedItems":0}}"#
        );
        assert_eq!(
            serde_json::to_string(&JobStageDto::Index).unwrap(),
            r#""index""#
        );
        assert_eq!(JobKindDto::IndexLibrary.as_str(), "index_library");
        assert_eq!(JobKindDto::ComputePlaySsr.as_str(), "compute_play_ssr");
        assert_eq!(JobKindDto::RateCopy.as_str(), "rate_copy");
        for (stage, wire) in [
            (JobStageDto::RenderAudio, r#""render_audio""#),
            (JobStageDto::Write, r#""write""#),
        ] {
            assert_eq!(serde_json::to_string(&stage).unwrap(), wire);
        }
        let summary = JobSummaryDto::RateCopy(RateCopySummaryDto {
            folder: "/s/1 a".to_owned(),
            osu_filename: "a [x 1.10x (132bpm)].osu".to_owned(),
            audio_filename: "audio 1.10x.ogg".to_owned(),
            osu_written: true,
            audio_written: false,
            audio_reused: true,
            next_step: RATE_COPY_NEXT_STEP.to_owned(),
            failed_items: 0,
        });
        assert_eq!(
            serde_json::to_string(&summary).unwrap(),
            r#"{"kind":"rate_copy","counters":{"folder":"/s/1 a","osuFilename":"a [x 1.10x (132bpm)].osu","audioFilename":"audio 1.10x.ogg","osuWritten":true,"audioWritten":false,"audioReused":true,"nextStep":"refresh_osu_then_sync","failedItems":0}}"#
        );
        assert_eq!(
            serde_json::to_string(&JobStageDto::PlaySsr).unwrap(),
            r#""play_ssr""#
        );
        let summary = JobSummaryDto::ComputePlaySsr(ComputePlaySsrSummaryDto {
            plays_total: 4,
            computed: 3,
            counted: 2,
            excluded: 1,
            skipped_memoized: 1,
            failed_items: 0,
        });
        assert_eq!(
            serde_json::to_string(&summary).unwrap(),
            r#"{"kind":"compute_play_ssr","counters":{"playsTotal":4,"computed":3,"counted":2,"excluded":1,"skippedMemoized":1,"failedItems":0}}"#
        );
        for kind in JobKindDto::ALL {
            assert_eq!(JobKindDto::parse(kind.as_str()), Some(*kind));
            assert_eq!(
                serde_json::to_string(kind).unwrap(),
                format!("\"{}\"", kind.as_str())
            );
        }
    }
}
