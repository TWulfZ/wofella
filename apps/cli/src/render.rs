//! Output: aligned plain-text tables, or the app's DTOs as JSON (spec 005 Design). The CLI is a
//! dev tool, so nothing here is localized.

use std::io::Write;

use serde::Serialize;
use wolluf_app::errors::AppError;

const COLUMN_GAP: &str = "  ";

pub(crate) fn json<T: Serialize + ?Sized>(value: &T) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    writeln!(out)?;
    Ok(())
}

pub(crate) fn text(body: &str) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(body.as_bytes())?;
    if !body.ends_with('\n') {
        writeln!(out)?;
    }
    Ok(())
}

/// Columns are as wide as their widest cell; the last one is not padded, so long paths never
/// leave trailing spaces.
pub(crate) fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let header: Vec<String> = headers.iter().map(|h| (*h).to_owned()).collect();
    let mut out = String::new();
    for row in std::iter::once(&header).chain(rows) {
        let last = row.len().saturating_sub(1);
        for (i, cell) in row.iter().enumerate() {
            if i == last {
                out.push_str(cell);
            } else {
                let pad = widths[i].saturating_sub(cell.chars().count());
                out.push_str(cell);
                out.push_str(&" ".repeat(pad));
                out.push_str(COLUMN_GAP);
            }
        }
        out.push('\n');
    }
    out
}

pub(crate) fn key_values(pairs: &[(&str, String)]) -> String {
    let width = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    pairs
        .iter()
        .map(|(k, v)| format!("{k:<width$}{COLUMN_GAP}{v}\n"))
        .collect()
}

/// The wire string of a snake_case DTO enum, so text and JSON output never disagree.
pub(crate) fn wire<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(e) => format!("<{e}>"),
    }
}

pub(crate) fn opt<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| v.to_string())
}

/// Spec 005: `error[<CODE>]: <messageKey> {k=v, …}`; `args` is a `BTreeMap`, so the order is
/// stable.
pub(crate) fn error_line(e: &AppError) -> String {
    let mut line = format!("error[{}]: {}", e.code.as_str(), e.message_key);
    if !e.args.is_empty() {
        let args: Vec<String> = e.args.iter().map(|(k, v)| format!("{k}={v}")).collect();
        line.push_str(&format!(" {{{}}}", args.join(", ")));
    }
    line
}

pub(crate) fn error(e: &AppError) {
    stderr_line(&error_line(e));
    if let Some(details) = &e.details {
        stderr_line(&format!("  details: {details}"));
    }
}

/// A non-fatal diagnostic; it never changes the exit code.
pub(crate) fn warning(message: &str) {
    stderr_line(&format!("warning: {message}"));
}

pub(crate) fn stderr_line(line: &str) {
    // Nothing useful is left to do when stderr itself is gone.
    let _ = writeln!(std::io::stderr().lock(), "{line}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_aligns_columns() {
        let got = table(
            &["A", "LONG"],
            &[
                vec!["xyz".to_owned(), "1".to_owned()],
                vec!["q".to_owned(), "22".to_owned()],
            ],
        );
        assert_eq!(got, "A    LONG\nxyz  1\nq    22\n");
    }

    #[test]
    fn error_line_format() {
        let e = AppError::osu_dir_not_found("/x").with_arg("b", "2");
        assert_eq!(
            error_line(&e),
            "error[OSU_DIR_NOT_FOUND]: error.code.OSU_DIR_NOT_FOUND {b=2, path=/x}"
        );
        assert_eq!(
            error_line(&AppError::conflict()),
            "error[CONFLICT]: error.code.CONFLICT"
        );
    }

    #[test]
    fn key_values_align() {
        assert_eq!(
            key_values(&[("a", "1".to_owned()), ("bbb", "2".to_owned())]),
            "a    1\nbbb  2\n"
        );
    }
}
