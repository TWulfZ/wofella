//! Process exit codes (spec 005 Behaviour, "Exit codes"); 006's `osg` reuses them.

use std::process::ExitCode;

use wolluf_app::jobs::JobStatusDto;
use wolluf_core::ErrorCode;

pub(crate) const SUCCESS: u8 = 0;
/// Environment or runtime failure, or a job that ended `failed`.
pub(crate) const FAILURE: u8 = 1;
/// Fix-your-input. Equal to clap's own usage-error code, so both paths agree.
pub(crate) const USAGE: u8 = 2;
/// 128 + SIGINT, the shell convention for Ctrl-C.
pub(crate) const CANCELLED: u8 = 130;

pub(crate) const fn for_error(code: ErrorCode) -> u8 {
    match code {
        ErrorCode::OsuRunning
        | ErrorCode::Conflict
        | ErrorCode::Internal
        | ErrorCode::SignatureInvalid
        | ErrorCode::ConsentRequired => FAILURE,
        ErrorCode::InvalidInput
        | ErrorCode::NotFound
        | ErrorCode::OsuDirNotFound
        | ErrorCode::ParseFailed
        | ErrorCode::UnsupportedFormat => USAGE,
        ErrorCode::Cancelled => CANCELLED,
    }
}

/// A sync with failed items still ends `ok` and exits 0 (spec 005). A job that is not
/// finished when the CLI stops waiting is a runtime failure.
pub(crate) const fn for_job(status: JobStatusDto) -> u8 {
    match status {
        JobStatusDto::Ok => SUCCESS,
        JobStatusDto::Cancelled => CANCELLED,
        JobStatusDto::Failed | JobStatusDto::Queued | JobStatusDto::Running => FAILURE,
    }
}

pub(crate) fn exit_code(code: u8) -> ExitCode {
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_table_matches_spec() {
        let table: Vec<(&str, u8)> = ErrorCode::ALL
            .iter()
            .map(|c| (c.as_str(), for_error(*c)))
            .collect();
        assert_eq!(
            table,
            vec![
                ("OSU_DIR_NOT_FOUND", 2),
                ("UNSUPPORTED_FORMAT", 2),
                ("PARSE_FAILED", 2),
                ("OSU_RUNNING", 1),
                ("CONSENT_REQUIRED", 1),
                ("SIGNATURE_INVALID", 1),
                ("NOT_FOUND", 2),
                ("INVALID_INPUT", 2),
                ("CONFLICT", 1),
                ("CANCELLED", 130),
                ("INTERNAL", 1),
            ]
        );
        assert_eq!(for_job(JobStatusDto::Ok), 0);
        assert_eq!(for_job(JobStatusDto::Failed), 1);
        assert_eq!(for_job(JobStatusDto::Cancelled), 130);
        assert_eq!(for_job(JobStatusDto::Running), 1);
    }
}
