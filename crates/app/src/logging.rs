//! Tracing setup shared by the desktop and CLI shells (architecture §7, spec 005).

use std::path::PathBuf;

use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

use crate::errors::AppError;

/// §7: "rolling daily, 14 days kept".
const MAX_LOG_FILES: usize = 14;
const LOG_FILE_PREFIX: &str = "wolluf";
const LOG_FILE_SUFFIX: &str = "jsonl";

#[derive(Debug, Clone)]
pub struct LogOptions {
    /// `EnvFilter` directives, e.g. `warn` or `wolluf_app=debug,info`.
    pub filter: String,
    pub json_dir: Option<PathBuf>,
    pub console: bool,
}

/// Dropping it flushes the non-blocking file writer, so shells hold it until exit.
#[must_use = "dropping the guard stops file logging"]
pub struct LogGuard {
    _file_writer: Option<WorkerGuard>,
}

/// Installs the global subscriber. The `tracing-log` bridge it also installs is how
/// `tauri-plugin-log` (UI) records reach the same JSON file.
pub fn init(options: LogOptions) -> Result<LogGuard, AppError> {
    let (subscriber, guard) = build(options)?;
    subscriber
        .try_init()
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(guard)
}

fn build(
    options: LogOptions,
) -> Result<(impl Subscriber + Send + Sync + 'static, LogGuard), AppError> {
    let filter = EnvFilter::try_new(&options.filter).map_err(|e| {
        AppError::invalid_input()
            .with_arg("filter", options.filter.clone())
            .with_details(e.to_string())
    })?;

    let (json_layer, file_writer) = match &options.json_dir {
        Some(dir) => {
            let appender = Builder::new()
                .rotation(Rotation::DAILY)
                .filename_prefix(LOG_FILE_PREFIX)
                .filename_suffix(LOG_FILE_SUFFIX)
                .max_log_files(MAX_LOG_FILES)
                .build(dir)
                .map_err(|e| {
                    AppError::internal(e.to_string())
                        .with_arg("path", dir.to_string_lossy().into_owned())
                })?;
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let layer = fmt::layer().json().with_ansi(false).with_writer(writer);
            (Some(layer), Some(guard))
        }
        None => (None, None),
    };
    let console_layer = options
        .console
        .then(|| fmt::layer().pretty().with_writer(std::io::stderr));

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .with(console_layer);
    Ok((
        subscriber,
        LogGuard {
            _file_writer: file_writer,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_json_line_to_dir() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        let (subscriber, guard) = build(LogOptions {
            filter: "info".to_owned(),
            json_dir: Some(logs.clone()),
            console: false,
        })
        .unwrap();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(job = "sync_plays", "job finished");
            tracing::debug!("filtered out");
        });
        drop(guard);

        let files: Vec<_> = std::fs::read_dir(&logs)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(files.len(), 1, "{files:?}");
        let name = &files[0];
        assert!(
            name.starts_with("wolluf.") && name.ends_with(".jsonl"),
            "{name}"
        );

        let text = std::fs::read_to_string(logs.join(name)).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 1, "{text}");
        let event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(event["level"], "INFO");
        assert_eq!(event["fields"]["message"], "job finished");
        assert_eq!(event["fields"]["job"], "sync_plays");
    }

    #[test]
    fn bad_filter_is_invalid_input() {
        let err = build(LogOptions {
            filter: "[".to_owned(),
            json_dir: None,
            console: false,
        })
        .err()
        .unwrap();
        assert_eq!(err.code, wolluf_core::ErrorCode::InvalidInput);
        assert_eq!(err.args.get("filter").map(String::as_str), Some("["));
    }
}
