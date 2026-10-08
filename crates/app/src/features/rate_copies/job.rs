//! `RateCopy`: renders the audio at the rate unless a playable file already holds that name,
//! then writes the audio and the `.osu` through `app::export`. Audio first, so stable never lists
//! a copy whose audio is missing; all or nothing per copy, so audio this run wrote is removed
//! again when the `.osu` cannot be written.

use std::ffi::OsStr;
use std::path::Path;

use wolluf_audio::{AudioParams, render_rate};
use wolluf_engine::RateCopy;
use wolluf_source_osu::songs::{SongFileError, read_song_file};

use super::dto::{AUDIO_TARGET_UNUSABLE_KEY, REFUSED_KEY, refusal};
use super::params::RateCopiesParams;
use super::service::RateCopyPreview;
use super::targets::{AudioTarget, OsuTarget, audio_target, osu_target};
use crate::context::blocking_join_error;
use crate::errors::{AppError, keys};
use crate::export::{ExportPermit, WriteOutcome, retract, write_new};
use crate::features::library::IndexLibraryJob;
use crate::jobs::dto::{
    JobKindDto, JobStageDto, JobSummaryDto, RATE_COPY_NEXT_STEP, RateCopySummaryDto,
};
use crate::jobs::{Job, JobCtx, JobFuture, JobSummary};

const DOMAIN_LIBRARY: &str = "library";

pub struct RateCopyJob {
    permit: ExportPermit,
    preview: RateCopyPreview,
    params: RateCopiesParams,
}

impl RateCopyJob {
    pub(super) fn new(
        permit: ExportPermit,
        preview: RateCopyPreview,
        params: RateCopiesParams,
    ) -> Self {
        Self {
            permit,
            preview,
            params,
        }
    }
}

impl Job for RateCopyJob {
    fn kind(&self) -> JobKindDto {
        JobKindDto::RateCopy
    }

    /// One permit, one job: two confirmations never coalesce.
    fn dedupe_key(&self) -> String {
        format!("rate_copy.{}", self.permit.preview_id())
    }

    fn params(&self) -> serde_json::Value {
        serde_json::json!({
            "md5": self.preview.md5.to_string(),
            "rateMilli": self.preview.rate_milli,
            "nightcore": self.preview.nightcore,
        })
    }

    fn run(self: Box<Self>, ctx: JobCtx) -> JobFuture {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || run(&ctx, &self))
                .await
                .map_err(blocking_join_error)?
        })
    }
}

fn run(ctx: &JobCtx, job: &RateCopyJob) -> Result<JobSummary, AppError> {
    let RateCopyJob {
        permit,
        preview,
        params,
    } = job;
    let copy = &preview.copy;
    let folder = permit.folder();
    ctx.check_cancelled()?;
    // The folder may have changed since the plan was shown.
    let osu_now = osu_target(
        folder,
        &copy.osu_filename,
        &copy.audio_filename,
        params.max_chart_bytes,
    )?;
    if osu_now == OsuTarget::Taken {
        return Err(already_exists(&copy.osu_filename));
    }
    ctx.progress.report(JobStageDto::RenderAudio, 0, 1);
    let rendered = match audio_target(folder, &copy.audio_filename)? {
        AudioTarget::Reusable => None,
        AudioTarget::Unusable => return Err(audio_unusable(&copy.audio_filename)),
        AudioTarget::Free => {
            let source = read_song_file(
                &preview.songs_dir,
                &preview.chart_rel_path,
                &copy.source_audio,
                params.max_audio_bytes,
            )
            .map_err(|e| source_audio_error(&e))?;
            let ext = Path::new(&copy.source_audio)
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or_default();
            let audio = AudioParams {
                pitch_follows_rate: preview.nightcore,
                ..params.audio
            };
            let ogg = ctx
                .cpu
                .install(|| render_rate(&source, ext, preview.rate_milli, &audio))
                .map_err(|e| AppError::unsupported_format().with_details(e.to_string()))?;
            Some(ogg)
        }
    };
    ctx.progress.report(JobStageDto::RenderAudio, 1, 1);
    ctx.check_cancelled()?;

    ctx.progress.report(JobStageDto::Write, 0, 2);
    let (audio_written, osu_written) =
        publish_copy(permit, copy, rendered.as_deref(), params.max_chart_bytes)?;
    ctx.progress.report(JobStageDto::Write, 2, 2);
    ctx.progress.flush();

    Ok(JobSummary {
        summary: Some(JobSummaryDto::RateCopy(RateCopySummaryDto {
            folder: folder.shown().to_string_lossy().into_owned(),
            osu_filename: copy.osu_filename.clone(),
            audio_filename: copy.audio_filename.clone(),
            osu_written,
            audio_written,
            audio_reused: !audio_written,
            next_step: RATE_COPY_NEXT_STEP.to_owned(),
            failed_items: ctx.failed_items(),
        })),
        changed: vec![DOMAIN_LIBRARY],
        // The copy is not in the catalog until stable lists it in osu!.db; the index then
        // parses and rates whatever the catalog holds by now.
        follow_ups: vec![Box::new(IndexLibraryJob)],
    })
}

/// Writes the rendered audio (`None`: a playable file is already there), then the `.osu`.
/// Returns `(audio_written, osu_written)`.
fn publish_copy(
    permit: &ExportPermit,
    copy: &RateCopy,
    ogg: Option<&[u8]>,
    max_chart_bytes: u64,
) -> Result<(bool, bool), AppError> {
    let audio = match ogg {
        Some(ogg) => match write_new(permit, &copy.audio_filename, ogg)? {
            WriteOutcome::Written(published) => Some(published),
            // Created while this job rendered; reused on the same terms as before rendering.
            WriteOutcome::Exists(_) => {
                if audio_target(permit.folder(), &copy.audio_filename)? != AudioTarget::Reusable {
                    return Err(audio_unusable(&copy.audio_filename));
                }
                None
            }
        },
        None => None,
    };
    let audio_written = audio.is_some();
    match write_osu(permit, copy, max_chart_bytes) {
        Ok(osu_written) => Ok((audio_written, osu_written)),
        Err(e) => {
            if let Some(published) = audio
                && let Err(undo) = retract(permit, published)
            {
                tracing::warn!(error = %undo.message_key, details = ?undo.details, "rate copy audio left behind");
            }
            Err(e)
        }
    }
}

/// `false` when an earlier copy that plays the same audio already holds the name.
fn write_osu(
    permit: &ExportPermit,
    copy: &RateCopy,
    max_chart_bytes: u64,
) -> Result<bool, AppError> {
    match write_new(permit, &copy.osu_filename, &copy.osu)? {
        WriteOutcome::Written(_) => Ok(true),
        WriteOutcome::Exists(_) => {
            let now = osu_target(
                permit.folder(),
                &copy.osu_filename,
                &copy.audio_filename,
                max_chart_bytes,
            )?;
            if now == OsuTarget::SameCopy {
                Ok(false)
            } else {
                Err(already_exists(&copy.osu_filename))
            }
        }
    }
}

fn already_exists(osu_filename: &str) -> AppError {
    AppError::conflict()
        .with_key(REFUSED_KEY)
        .with_arg("refusal", refusal::ALREADY_EXISTS)
        .with_arg("fileName", osu_filename)
}

fn audio_unusable(audio_filename: &str) -> AppError {
    AppError::conflict()
        .with_key(AUDIO_TARGET_UNUSABLE_KEY)
        .with_arg("fileName", audio_filename)
}

fn source_audio_error(e: &SongFileError) -> AppError {
    match e {
        SongFileError::Missing => AppError::new(e.code()).with_key(keys::CHART_AUDIO_UNAVAILABLE),
        SongFileError::TooLarge { size, max } => AppError::new(e.code())
            .with_key(keys::CHART_AUDIO_TOO_LARGE)
            .with_arg("bytes", size.to_string())
            .with_arg("maxBytes", max.to_string()),
        SongFileError::Io { .. } => AppError::internal(format!("source audio: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use wolluf_core::UnixUs;

    use super::*;
    use crate::export::{
        self, ExportParams, Preview, PreviewRegistry, SetFolder, keys as export_keys,
    };

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);

    fn permit(songs: &Path, files: &[&str]) -> ExportPermit {
        let folder = SetFolder::resolve(songs, Path::new("100 set/map.osu")).unwrap();
        let registry = PreviewRegistry::new(ExportParams::default());
        let id = registry.record(
            T0,
            Preview {
                folder,
                files: files.iter().map(|f| (*f).to_owned()).collect(),
                payload: (),
            },
        );
        export::confirm(&registry, &id, T0).unwrap().0
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// All or nothing per copy: audio this run published is removed when the `.osu` fails.
    #[test]
    fn a_failed_osu_write_removes_the_audio_this_run_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let songs = dir.path().join("Songs");
        let set: PathBuf = songs.join("100 set");
        std::fs::create_dir_all(set.join("copy.osu")).unwrap();
        std::fs::write(set.join("map.osu"), b"osu").unwrap();
        let copy = RateCopy {
            osu: b"osu".to_vec(),
            version: "Normal 1.25x (150bpm)".to_owned(),
            audio_filename: "a 1.25x.ogg".to_owned(),
            source_audio: "a.wav".to_owned(),
            osu_filename: "copy.osu".to_owned(),
        };
        let permit = permit(&songs, &["a 1.25x.ogg", "copy.osu"]);
        let err = publish_copy(&permit, &copy, Some(b"OggS\x00\x02"), u64::MAX).unwrap_err();
        assert_eq!(err.message_key, export_keys::TARGET_NOT_A_FILE);
        assert_eq!(names(&set), ["copy.osu", "map.osu"]);
    }
}
