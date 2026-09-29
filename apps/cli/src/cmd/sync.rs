//! `wolluf sync`: start `SyncPlays` for the registered install, draw progress, cancel on
//! Ctrl-C, and print the finished job (spec 005 Behaviour, "CLI").

use std::future::Future;
use std::process::ExitCode;

use wolluf_app::context::AppContext;
use wolluf_app::errors::AppError;
use wolluf_app::jobs::JobStatusDto;
use wolluf_app::jobs::dto::{JobDto, JobStartDto, JobSummaryDto, SyncPlaysStartDto};
use wolluf_core::ErrorCode;

use crate::exit;
use crate::follow;
use crate::progress::Progress;
use crate::render;

pub(crate) async fn run(ctx: &AppContext, json: bool) -> anyhow::Result<ExitCode> {
    let outcome = sync(ctx, &mut Progress::stderr(), follow::ctrl_c()).await?;
    if json {
        render::json(&outcome.job)?;
    } else {
        render::text(&job_text(&outcome.job))?;
    }
    let code = if outcome.interrupted {
        exit::CANCELLED
    } else {
        exit::for_job(outcome.job.status)
    };
    Ok(exit::exit_code(code))
}

pub(crate) struct Outcome {
    pub(crate) job: JobDto,
    pub(crate) interrupted: bool,
}

pub(crate) async fn sync(
    ctx: &AppContext,
    progress: &mut Progress,
    interrupt: impl Future<Output = ()>,
) -> Result<Outcome, AppError> {
    let install = ctx
        .setup()
        .status()
        .await?
        .install
        .ok_or_else(|| AppError::new(ErrorCode::OsuDirNotFound))?;
    // Subscribed before starting, so the finish event cannot slip past.
    let mut rx = ctx.subscribe();
    let id = ctx
        .job_service()
        .start(JobStartDto::SyncPlays(SyncPlaysStartDto {
            install_id: install.id,
        }))
        .await?;
    tokio::pin!(interrupt);
    let mut interrupted =
        follow::until_finished(ctx, &mut rx, &id, progress, interrupt.as_mut()).await?;
    // The chained identity refresh must finish before exit, or `players list` sees stale
    // rows; a Ctrl-C here stops waiting and dropping the context cancels it.
    if !interrupted {
        tokio::select! {
            biased;
            () = &mut interrupt => interrupted = true,
            () = ctx.jobs().wait_idle() => {}
        }
    }
    let job = follow::history_entry(ctx, &id).await?;
    Ok(Outcome { job, interrupted })
}

fn job_text(job: &JobDto) -> String {
    let mut pairs = vec![
        ("job", job.id.to_string()),
        ("status", render::wire(&job.status)),
    ];
    if let Some(JobSummaryDto::SyncPlays(s)) = &job.summary {
        pairs.extend([
            ("plays new", s.plays_new.to_string()),
            ("plays replay-only", s.plays_replay_only.to_string()),
            ("plays existing", s.plays_existing.to_string()),
            ("conflicts", s.conflicts.to_string()),
            ("skipped non-mania", s.skipped_non_mania.to_string()),
            ("replays linked", s.replays_linked.to_string()),
            (".osg linked", s.osg_linked.to_string()),
            ("charts archived", s.charts_archived.to_string()),
            ("chart unavailable", s.chart_unavailable.to_string()),
            ("chart md5 mismatch", s.chart_md5_mismatch.to_string()),
            ("orphan replays", s.orphan_replays.to_string()),
            ("failed items", s.failed_items.to_string()),
        ]);
    }
    if let Some(e) = &job.error {
        pairs.push((
            "error",
            format!("{} {}", render::wire(&e.code), e.message_key),
        ));
    }
    if job.status == JobStatusDto::Cancelled {
        pairs.push((
            "note",
            "cancelled; rerun `wolluf sync` to resume".to_owned(),
        ));
    }
    render::key_values(&pairs)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use wolluf_app::context::AppPaths;
    use wolluf_core::{FixedClock, UnixUs};

    use super::*;

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/dbs");

    async fn context_with_install(dir: &Path) -> AppContext {
        let root = dir.join("osu!");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("osu!.exe"), b"MZ").unwrap();
        for (from, to) in [
            ("osu_db/osu-20260924.min.db", "osu!.db"),
            ("scores_db/scores-20260924.min.db", "scores.db"),
        ] {
            std::fs::copy(Path::new(FIXTURES).join(from), root.join(to)).unwrap();
        }
        let ctx = AppContext::open(
            AppPaths::from_data_dir(dir.join("data")),
            Arc::new(FixedClock::new(T0)),
        )
        .unwrap();
        ctx.setup()
            .set_install_path(root.to_str().unwrap().to_owned())
            .await
            .unwrap();
        ctx
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn interrupt_cancels_the_job() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context_with_install(dir.path()).await;
        let outcome = sync(&ctx, &mut Progress::stderr(), std::future::ready(()))
            .await
            .unwrap();
        assert!(outcome.interrupted);
        assert_eq!(
            outcome.job.status,
            JobStatusDto::Cancelled,
            "{:?}",
            outcome.job
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn no_interrupt_waits_for_ok() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context_with_install(dir.path()).await;
        let outcome = sync(&ctx, &mut Progress::stderr(), std::future::pending())
            .await
            .unwrap();
        assert!(!outcome.interrupted);
        assert_eq!(outcome.job.status, JobStatusDto::Ok);
        assert!(
            ctx.jobs()
                .list(None)
                .await
                .unwrap()
                .iter()
                .all(|j| j.status != JobStatusDto::Queued)
        );
    }
}
