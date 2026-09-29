//! `wolluf setup detect|set|status` over `SetupService` (spec 005).

use std::process::ExitCode;

use wolluf_app::context::AppContext;
use wolluf_app::features::setup::dto::{InstallCandidateDto, InstallDto, SetupStatusDto};

use crate::cli::SetupCmd;
use crate::exit;
use crate::render;

pub(crate) async fn run(ctx: &AppContext, cmd: SetupCmd, json: bool) -> anyhow::Result<ExitCode> {
    match cmd {
        SetupCmd::Detect => {
            let candidates = ctx.setup().detect_installs().await?;
            if json {
                render::json(&candidates)?;
            } else {
                render::text(&candidates_table(&candidates))?;
            }
        }
        SetupCmd::Set { path } => {
            let install = ctx.setup().set_install_path(path).await?;
            if json {
                render::json(&install)?;
            } else {
                render::text(&install_text(&install))?;
            }
        }
        SetupCmd::Status => {
            let status = ctx.setup().status().await?;
            if json {
                render::json(&status)?;
            } else {
                render::text(&status_text(&status))?;
            }
        }
    }
    Ok(exit::exit_code(exit::SUCCESS))
}

fn candidates_table(candidates: &[InstallCandidateDto]) -> String {
    let rows: Vec<Vec<String>> = candidates
        .iter()
        .map(|c| {
            vec![
                render::wire(&c.source),
                if c.valid { "yes" } else { "no" }.to_owned(),
                render::opt(c.osu_db_version),
                if c.missing.is_empty() {
                    "-".to_owned()
                } else {
                    c.missing.join(",")
                },
                c.path.clone(),
            ]
        })
        .collect();
    render::table(&["SOURCE", "VALID", "OSU_DB", "MISSING", "PATH"], &rows)
}

fn install_text(install: &InstallDto) -> String {
    render::key_values(&[
        ("install", install.id.to_string()),
        ("path", install.root_path.clone()),
        ("osu!.db", render::opt(install.osu_db_version)),
        ("detected", install.detected_at.clone()),
    ])
}

fn status_text(status: &SetupStatusDto) -> String {
    let (install, path) = status.install.as_ref().map_or_else(
        || ("-".to_owned(), "-".to_owned()),
        |i| (i.id.to_string(), i.root_path.clone()),
    );
    let last_sync = status.last_sync.as_ref().map_or_else(
        || "-".to_owned(),
        |j| {
            format!(
                "{} {:?} {}",
                j.id,
                j.status,
                render::opt(j.ended.as_deref())
            )
        },
    );
    render::key_values(&[
        ("install", install),
        ("path", path),
        (
            "identity",
            if status.identity_ready {
                "ready"
            } else {
                "needs wizard"
            }
            .to_owned(),
        ),
        ("last sync", last_sync),
        ("data dir", status.data_dir.clone()),
        ("logs dir", status.logs_dir.clone()),
        ("version", status.app_version.clone()),
    ])
}
