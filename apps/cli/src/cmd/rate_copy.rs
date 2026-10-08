//! `wolluf rate-copy plan|create` over `RateCopiesService` (ADR 0025). `create` is the CLI's
//! confirmation: `--yes` stands for the dialog the desktop shows.

use std::path::Path;
use std::process::ExitCode;

use serde::Serialize;
use wolluf_app::context::AppContext;
use wolluf_app::errors::AppError;
use wolluf_app::features::rate_copies::dto::{REFUSED_KEY, RateCopyPlanDto};
use wolluf_app::jobs::dto::{JobDto, JobSummaryDto};

use crate::cli::{RateCopyArgs, RateCopyCmd};
use crate::exit;
use crate::follow;
use crate::progress::Progress;
use crate::render;

const F5_NOTICE: &str = "press F5 in osu! song select, then run `wolluf sync` to see the copy here";

#[derive(Serialize)]
struct Created<'a> {
    plan: &'a RateCopyPlanDto,
    job: &'a JobDto,
}

pub(crate) async fn run(
    ctx: &AppContext,
    cmd: RateCopyCmd,
    json: bool,
) -> anyhow::Result<ExitCode> {
    match cmd {
        RateCopyCmd::Plan(args) => {
            let plan = plan(ctx, &args).await?;
            if json {
                render::json(&plan)?;
            } else {
                render::text(&plan_text(&plan))?;
            }
            Ok(exit::exit_code(exit::SUCCESS))
        }
        RateCopyCmd::Create(args) => {
            let plan = plan(ctx, &args.copy).await?;
            if let Some(refusal) = &plan.refusal {
                if !json {
                    render::text(&plan_text(&plan))?;
                }
                return Err(AppError::invalid_input()
                    .with_key(REFUSED_KEY)
                    .with_arg("refusal", refusal.as_str())
                    .into());
            }
            // Subscribed before confirming, so the finish event cannot slip past.
            let mut rx = ctx.subscribe();
            let id = ctx.rate_copies().confirm(&plan.preview_id).await?;
            let interrupt = std::pin::pin!(follow::ctrl_c());
            let interrupted =
                follow::until_finished(ctx, &mut rx, &id, &mut Progress::stderr(), interrupt)
                    .await?;
            if !interrupted {
                // The chained index runs against this context; closing it early would cancel it.
                ctx.jobs().wait_idle().await;
            }
            let job = follow::history_entry(ctx, &id).await?;
            if json {
                render::json(&Created {
                    plan: &plan,
                    job: &job,
                })?;
            } else {
                render::text(&created_text(&job))?;
            }
            let code = if interrupted {
                exit::CANCELLED
            } else {
                exit::for_job(job.status)
            };
            Ok(exit::exit_code(code))
        }
    }
}

async fn plan(ctx: &AppContext, args: &RateCopyArgs) -> Result<RateCopyPlanDto, AppError> {
    ctx.rate_copies().plan(&args.md5, args.rate).await
}

fn existing(exists: bool) -> &'static str {
    if exists { "exists" } else { "new" }
}

fn plan_text(p: &RateCopyPlanDto) -> String {
    render::key_values(&[
        ("md5", p.md5.clone()),
        ("rate", format!("{:.3}x", f64::from(p.rate_milli) / 1000.0)),
        ("folder", p.folder.clone()),
        (
            "version",
            render::opt(Some(&p.version).filter(|v| !v.is_empty())),
        ),
        (
            ".osu",
            format!("{} ({})", p.osu_filename, existing(p.osu_exists)),
        ),
        (
            "audio",
            format!("{} ({})", p.audio_filename, existing(p.audio_exists)),
        ),
        ("refusal", render::opt(p.refusal.as_deref())),
    ])
}

fn created_text(job: &JobDto) -> String {
    let mut pairs = vec![
        ("job", job.id.to_string()),
        ("status", render::wire(&job.status)),
    ];
    if let Some(JobSummaryDto::RateCopy(s)) = &job.summary {
        let folder = Path::new(&s.folder);
        let path = |name: &str| folder.join(name).display().to_string();
        let audio = if s.audio_written { "written" } else { "reused" };
        let osu = if s.osu_written {
            "written"
        } else {
            "skipped (exists)"
        };
        pairs.extend([
            (audio, path(&s.audio_filename)),
            (osu, path(&s.osu_filename)),
            ("next", F5_NOTICE.to_owned()),
        ]);
    }
    if let Some(e) = &job.error {
        pairs.push((
            "error",
            format!("{} {}", render::wire(&e.code), e.message_key),
        ));
    }
    render::key_values(&pairs)
}
