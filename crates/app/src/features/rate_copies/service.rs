//! `RateCopiesService`: the rate-copy feature's only entry point for shells (D12).

use std::path::PathBuf;

use wolluf_core::{ChartMd5, ErrorCode};
use wolluf_engine::RateCopy;
use wolluf_source_osu::songs::{ChartReadError, read_chart_verified};

use super::assess::{Assessment, assess, has_storyboard_samples};
use super::dto::{RateCopyPlanDto, refusal};
use super::job::RateCopyJob;
use super::params::RateCopiesParams;
use super::targets::{AudioTarget, OsuTarget, audio_target, osu_target};
use crate::context::{AppContext, blocking_join_error, catalog_install, songs_dir};
use crate::errors::AppError;
use crate::export::{self, Preview, SetFolder};
use crate::features::library::dto::MsdStatusDto;
use crate::jobs::dto::JobId;

/// The payload of a recorded preview: exactly what the job writes.
#[derive(Debug, Clone)]
pub(crate) struct RateCopyPreview {
    pub(crate) md5: ChartMd5,
    pub(crate) rate_milli: u16,
    pub(crate) copy: RateCopy,
    pub(crate) songs_dir: PathBuf,
    pub(crate) chart_rel_path: PathBuf,
}

pub struct RateCopiesService<'a> {
    ctx: &'a AppContext,
    params: RateCopiesParams,
}

struct Planned {
    folder: SetFolder,
    assessment: Assessment,
    audio_exists: bool,
    osu_exists: bool,
    songs_dir: PathBuf,
    chart_rel_path: PathBuf,
}

impl<'a> RateCopiesService<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self::with_params(ctx, RateCopiesParams::default())
    }

    pub fn with_params(ctx: &'a AppContext, params: RateCopiesParams) -> Self {
        Self { ctx, params }
    }

    /// Reads the chart and its set folder and records a preview; writes nothing. A refused plan
    /// is not recorded, so its `previewId` is empty.
    pub async fn plan(&self, md5: &str, rate_milli: u16) -> Result<RateCopyPlanDto, AppError> {
        let md5: ChartMd5 = md5
            .parse()
            .map_err(|_| AppError::invalid_input().with_arg("md5", md5))?;
        let library = self.ctx.library();
        let chart = library
            .catalog_charts(vec![md5])
            .await?
            .remove(&md5)
            .ok_or_else(|| not_found(md5))?;
        let ln_heavy = match library.chart_msd(&md5.to_string()).await {
            Ok(msd) => match msd.status {
                MsdStatusDto::LnHeavy => Some(true),
                MsdStatusDto::Rated | MsdStatusDto::CalcRejected => Some(false),
                MsdStatusDto::Pending => None,
            },
            // A keymode without a calculator has no status; the file decides.
            Err(e) if e.code == ErrorCode::NotFound => None,
            Err(e) => return Err(e),
        };
        let (user, cache) = (self.ctx.user_db().clone(), self.ctx.cache_db().clone());
        let params = self.params.clone();
        let planned = tokio::task::spawn_blocking(move || {
            let install = catalog_install(&user, &cache)?.ok_or_else(|| not_found(md5))?;
            let songs_dir = songs_dir(&install.root_path);
            let chart_rel_path = PathBuf::from(&chart.path);
            let folder = SetFolder::resolve(&songs_dir, &chart_rel_path)?;
            let osu = read_chart_verified(&songs_dir, &chart_rel_path, md5)
                .map_err(|e| chart_error(md5, &e))?;
            let mut assessment = assess(&osu, rate_milli, ln_heavy, &params);
            let (mut audio_exists, mut osu_exists) = (false, false);
            if let Some(copy) = &assessment.copy {
                let mut refusal = assessment.refusal;
                let names_ok = folder.entry(&copy.osu_filename).is_ok()
                    && folder.entry(&copy.audio_filename).is_ok();
                if names_ok {
                    // The job decides whether what is there can be reused; the plan only says
                    // the name is taken.
                    audio_exists =
                        audio_target(&folder, &copy.audio_filename)? != AudioTarget::Free;
                    match osu_target(
                        &folder,
                        &copy.osu_filename,
                        &copy.audio_filename,
                        params.max_chart_bytes,
                    )? {
                        OsuTarget::Free => {}
                        OsuTarget::SameCopy => osu_exists = true,
                        // Rendering audio that no `.osu` would play is wasted work and space.
                        OsuTarget::Taken => refusal = refusal.or(Some(refusal::ALREADY_EXISTS)),
                    }
                } else {
                    refusal = refusal.or(Some(refusal::UNSAFE_NAME));
                }
                if refusal.is_none() && storyboard_samples(&folder, params.max_storyboard_bytes)? {
                    refusal = Some(refusal::KEYSOUNDED);
                }
                if refusal.is_none()
                    && !audio_exists
                    && !folder.has_file(&copy.source_audio).unwrap_or(false)
                {
                    refusal = Some(refusal::AUDIO_MISSING);
                }
                assessment.refusal = refusal;
            }
            Ok::<_, AppError>(Planned {
                folder,
                assessment,
                audio_exists,
                osu_exists,
                songs_dir,
                chart_rel_path,
            })
        })
        .await
        .map_err(blocking_join_error)??;
        Ok(self.record(md5, rate_milli, planned))
    }

    fn record(&self, md5: ChartMd5, rate_milli: u16, planned: Planned) -> RateCopyPlanDto {
        let Planned {
            folder,
            assessment,
            audio_exists,
            osu_exists,
            songs_dir,
            chart_rel_path,
        } = planned;
        let mut dto = RateCopyPlanDto {
            preview_id: String::new(),
            md5: md5.to_string(),
            rate_milli,
            folder: folder.shown().to_string_lossy().into_owned(),
            osu_filename: String::new(),
            version: String::new(),
            audio_filename: String::new(),
            audio_exists,
            osu_exists,
            refusal: assessment.refusal.map(str::to_owned),
        };
        let Some(copy) = assessment.copy else {
            return dto;
        };
        dto.osu_filename.clone_from(&copy.osu_filename);
        dto.version.clone_from(&copy.version);
        dto.audio_filename.clone_from(&copy.audio_filename);
        if assessment.refusal.is_none() {
            let files = vec![copy.audio_filename.clone(), copy.osu_filename.clone()];
            dto.preview_id = self.ctx.rate_copy_previews().record(
                self.ctx.clock().now(),
                Preview {
                    folder,
                    files,
                    payload: RateCopyPreview {
                        md5,
                        rate_milli,
                        copy,
                        songs_dir,
                        chart_rel_path,
                    },
                },
            );
        }
        dto
    }

    /// Mints the `ExportPermit` for a recorded preview and starts the `RateCopy` job, which
    /// writes exactly what the preview showed.
    pub async fn confirm(&self, preview_id: &str) -> Result<JobId, AppError> {
        let now = self.ctx.clock().now();
        let (permit, preview) = export::confirm(self.ctx.rate_copy_previews(), preview_id, now)?;
        let job = RateCopyJob::new(permit, preview, self.params.clone());
        Ok(self.ctx.jobs().submit(Box::new(job)))
    }
}

/// Any `.osb` of the set may be the one stable loads. One too large to read is refused as if it
/// had samples: the copy cannot be shown to be safe.
fn storyboard_samples(folder: &SetFolder, max_bytes: u64) -> Result<bool, AppError> {
    let io = |e: std::io::Error| AppError::internal(format!("{}: {e}", folder.path().display()));
    for entry in std::fs::read_dir(folder.path()).map_err(io)? {
        let path = entry.map_err(io)?.path();
        let is_osb = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("osb"));
        if !is_osb || !path.is_file() {
            continue;
        }
        if std::fs::metadata(&path).map_err(io)?.len() > max_bytes
            || has_storyboard_samples(&std::fs::read(&path).map_err(io)?)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn not_found(md5: ChartMd5) -> AppError {
    AppError::not_found().with_arg("md5", md5.to_string())
}

fn chart_error(md5: ChartMd5, e: &ChartReadError) -> AppError {
    match e {
        ChartReadError::Io { .. } => AppError::internal(format!("chart {md5}: {e}")),
        _ => AppError::new(e.code())
            .with_arg("md5", md5.to_string())
            .with_details(e.to_string()),
    }
}
