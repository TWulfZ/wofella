//! `wolluf osg dump|survey` (spec 006 Behaviour): thin arms over `plays::osg`, which also owns
//! the rendering (D11).

use std::process::ExitCode;

use wolluf_app::errors::AppError;
use wolluf_app::features::plays::osg::{self, DumpFormat, DumpView, SurveyOptions};
use wolluf_core::ErrorCode;

use crate::cli::{OsgCmd, OsgDumpArgs, OsgFormat, OsgSurveyArgs};
use crate::exit;
use crate::render;

pub(crate) async fn run(cmd: OsgCmd, json: bool) -> anyhow::Result<ExitCode> {
    match cmd {
        OsgCmd::Dump(args) => dump(&args, json),
        OsgCmd::Survey(args) => survey(args, json).await,
    }
}

fn dump(args: &OsgDumpArgs, json: bool) -> anyhow::Result<ExitCode> {
    let dump = match osg::inspect(&args.path) {
        Ok(dump) => dump,
        Err(e) => match dump_error_line(&e) {
            Some(line) => {
                render::stderr_line(&line);
                return Ok(exit::exit_code(exit::for_error(e.code)));
            }
            None => return Err(e.into()),
        },
    };
    for line in osg::render_diagnostics(&dump) {
        render::warning(&line);
    }
    let format = if json {
        DumpFormat::Json
    } else {
        dump_format(args.format)
    };
    let view = if args.events {
        DumpView::Events
    } else {
        DumpView::Records
    };
    render::text(&osg::render_dump(&dump, format, view, args.limit)?)?;
    Ok(exit::exit_code(exit::SUCCESS))
}

async fn survey(args: OsgSurveyArgs, json: bool) -> anyhow::Result<ExitCode> {
    let opts = SurveyOptions {
        max_files: args.max_files,
        ..SurveyOptions::default()
    };
    // Ctrl-C keeps its default action: the survey only reads, so stopping it anywhere loses
    // nothing, and a token nobody cancels needs no signal handler.
    let report =
        tokio::task::spawn_blocking(move || osg::survey(&args.corpus, &opts, &Default::default()))
            .await??;
    if json {
        render::json(&report)?;
    } else {
        render::text(&osg::render_survey(&report))?;
    }
    if args.strict && !report.strict_passes() {
        render::stderr_line(&format!(
            "strict: {} unexplained failures",
            report.unexplained_failures()
        ));
        return Ok(exit::exit_code(exit::FAILURE));
    }
    Ok(exit::exit_code(exit::SUCCESS))
}

const fn dump_format(format: OsgFormat) -> DumpFormat {
    match format {
        OsgFormat::Table => DumpFormat::Table,
        OsgFormat::Json => DumpFormat::Json,
        OsgFormat::Csv => DumpFormat::Csv,
    }
}

/// Spec 006 fixes `PARSE_FAILED: <variant> <details>` (the codec error's Debug, which the app
/// puts in `details`) and `NOT_FOUND` for a missing file. Other errors keep the spec 005 line.
fn dump_error_line(e: &AppError) -> Option<String> {
    let what = match e.code {
        ErrorCode::ParseFailed => e.details.as_deref(),
        ErrorCode::NotFound => e.args.get("path").map(String::as_str),
        _ => None,
    }?;
    Some(format!("{}: {what}", e.code.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osg_dump_error_lines_follow_spec_006() {
        let parse = AppError::new(ErrorCode::ParseFailed)
            .with_arg("path", "/x.osg")
            .with_details("StrideMismatch { len: 14, count: 1 }");
        assert_eq!(
            dump_error_line(&parse).as_deref(),
            Some("PARSE_FAILED: StrideMismatch { len: 14, count: 1 }")
        );
        let missing = AppError::not_found().with_arg("path", "/x.osg");
        assert_eq!(
            dump_error_line(&missing).as_deref(),
            Some("NOT_FOUND: /x.osg")
        );
        assert_eq!(dump_error_line(&AppError::internal("denied")), None);
    }

    #[test]
    fn osg_formats_map_one_to_one() {
        assert_eq!(dump_format(OsgFormat::Table), DumpFormat::Table);
        assert_eq!(dump_format(OsgFormat::Json), DumpFormat::Json);
        assert_eq!(dump_format(OsgFormat::Csv), DumpFormat::Csv);
    }
}
