//! `wolluf jobs list [--limit N]` over `JobService::list` (spec 005).

use std::process::ExitCode;

use wolluf_app::context::AppContext;
use wolluf_app::jobs::dto::{JobDto, JobSummaryDto};

use crate::cli::JobsCmd;
use crate::exit;
use crate::render;

pub(crate) async fn run(ctx: &AppContext, cmd: JobsCmd, json: bool) -> anyhow::Result<ExitCode> {
    match cmd {
        JobsCmd::List { limit } => {
            let jobs = ctx.job_service().list(limit).await?;
            if json {
                render::json(&jobs)?;
            } else {
                render::text(&jobs_table(&jobs))?;
            }
        }
    }
    Ok(exit::exit_code(exit::SUCCESS))
}

fn result(job: &JobDto) -> String {
    if let Some(e) = &job.error {
        return format!("{} {}", render::wire(&e.code), e.message_key);
    }
    match &job.summary {
        Some(JobSummaryDto::SyncPlays(s)) => format!(
            "new={} replay_only={} failed={}",
            s.plays_new, s.plays_replay_only, s.failed_items
        ),
        None => "-".to_owned(),
    }
}

fn jobs_table(jobs: &[JobDto]) -> String {
    let rows: Vec<Vec<String>> = jobs
        .iter()
        .map(|j| {
            vec![
                j.id.to_string(),
                j.kind.as_str().to_owned(),
                render::wire(&j.status),
                render::opt(j.started.as_deref()),
                render::opt(j.ended.as_deref()),
                result(j),
            ]
        })
        .collect();
    render::table(
        &["ID", "KIND", "STATUS", "STARTED", "ENDED", "RESULT"],
        &rows,
    )
}
