//! `wolluf osg dump` (spec 006 Behaviour): one `.osg` decoded into rows, rendered as a table,
//! CSV or JSON. Rendering lives here rather than in the CLI so the shell arm stays a thin call
//! (D11) and the goldens pin the exact output (AC5).

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use serde::Serialize;
use wolluf_source_osu::codec::osg::events::{OsgEvent, events};
use wolluf_source_osu::codec::osg::{
    OsgFile, OsgRecord, OsgScoreSystem, STRIDE_V1, STRIDE_V2, decode_osg,
};
use wolluf_source_osu::codec::score_header::JudgementCounts;
use wolluf_source_osu::{CodecError, Diagnostic, Diagnostics};

use crate::errors::AppError;

const SCORE_ONLY: &str = "score_only";
/// Kinds inside one CSV cell; a comma would split the cell.
const CSV_KIND_SEPARATOR: &str = ";";
const TABLE_KIND_SEPARATOR: &str = ",";
const MISSING: &str = "-";
const V1_COLUMNS: [&str; 21] = [
    "idx",
    "t_ms",
    "d300",
    "d100",
    "d50",
    "dmax",
    "d200",
    "dmiss",
    "c300",
    "c100",
    "c50",
    "cmax",
    "c200",
    "cmiss",
    "score",
    "max_combo",
    "combo",
    "hp_raw",
    "b4",
    "b25",
    "b28",
];
const V2_COLUMNS: [&str; 2] = ["f0", "f1"];
const EVENT_COLUMNS: [&str; 6] = ["idx", "t_ms", "kinds", "n", "combo_delta", "score_delta"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OsgDump {
    pub header: OsgDumpHeader,
    pub diagnostics: Vec<DiagnosticRow>,
    pub records: Vec<OsgRecordRow>,
    pub events: Vec<OsgEventRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OsgDumpHeader {
    pub client_version: i32,
    pub record_count: usize,
    /// `None` for a file without records: the stride is only known from the body.
    pub stride: Option<usize>,
    pub score_system: Option<&'static str>,
    pub file_size: u64,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticRow {
    pub code: &'static str,
    pub offset: Option<u64>,
    pub detail: String,
}

/// `d*` are signed changes from the previous record, so a count that goes down shows as
/// negative instead of being hidden (I4).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OsgRecordRow {
    pub idx: usize,
    pub t_ms: i32,
    pub d300: i32,
    pub d100: i32,
    pub d50: i32,
    pub dmax: i32,
    pub d200: i32,
    pub dmiss: i32,
    pub c300: u16,
    pub c100: u16,
    pub c50: u16,
    pub cmax: u16,
    pub c200: u16,
    pub cmiss: u16,
    pub score: i32,
    pub max_combo: u16,
    pub combo: u16,
    pub hp_raw: u16,
    pub b4: u8,
    pub b25: u8,
    pub b28: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f0: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f1: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OsgEventRow {
    pub idx: usize,
    pub t_ms: i32,
    /// Fixed order MAX, 300, 200, 100, 50, miss.
    pub kinds: Vec<&'static str>,
    pub n: u16,
    pub combo_delta: i32,
    pub score_delta: i32,
    pub score_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpFormat {
    Table,
    Json,
    Csv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpView {
    Records,
    Events,
}

/// Reads the file once and never opens it for writing (R8).
pub fn inspect(path: &Path) -> Result<OsgDump, AppError> {
    let bytes = std::fs::read(path).map_err(|e| read_error(path, &e))?;
    let (osg, diags) = decode_osg(&bytes).map_err(|e| codec_error(path, &e))?;
    Ok(build(&osg, &diags, bytes.len() as u64))
}

fn read_error(path: &Path, e: &io::Error) -> AppError {
    let base = if e.kind() == io::ErrorKind::NotFound {
        AppError::not_found()
    } else {
        AppError::internal(e.to_string())
    };
    base.with_arg("path", path.to_string_lossy())
}

/// `details` is the variant with its fields (`StrideMismatch { len: .., count: .. }`), which is
/// what the CLI prints after the code.
fn codec_error(path: &Path, e: &CodecError) -> AppError {
    AppError::new(e.code())
        .with_arg("path", path.to_string_lossy())
        .with_details(format!("{e:?}"))
}

fn build(osg: &OsgFile, diags: &Diagnostics, file_size: u64) -> OsgDump {
    let stride = osg.score_system.map(|s| match s {
        OsgScoreSystem::V1 => STRIDE_V1,
        OsgScoreSystem::V2 => STRIDE_V2,
    });
    let score_system = osg.score_system.map(|s| match s {
        OsgScoreSystem::V1 => "v1",
        OsgScoreSystem::V2 => "v2",
    });
    let zero = JudgementCounts::default();
    let records = osg
        .records
        .iter()
        .enumerate()
        .map(|(idx, r)| {
            let prev = idx
                .checked_sub(1)
                .and_then(|p| osg.records.get(p))
                .map_or(&zero, |p| &p.counts);
            record_row(idx, r, prev)
        })
        .collect();
    OsgDump {
        header: OsgDumpHeader {
            client_version: osg.client_version,
            record_count: osg.records.len(),
            stride,
            score_system,
            file_size,
            diagnostics: diags.len(),
        },
        diagnostics: diags.iter().map(diagnostic_row).collect(),
        records,
        events: events(osg).iter().map(event_row).collect(),
    }
}

fn diagnostic_row(d: &Diagnostic) -> DiagnosticRow {
    DiagnosticRow {
        code: d.code.as_str(),
        offset: d.offset,
        detail: d.detail.clone(),
    }
}

fn record_row(idx: usize, r: &OsgRecord, prev: &JudgementCounts) -> OsgRecordRow {
    let c = &r.counts;
    let d = |now: u16, before: u16| i32::from(now) - i32::from(before);
    OsgRecordRow {
        idx,
        t_ms: r.t_ms,
        d300: d(c.n300, prev.n300),
        d100: d(c.n100, prev.n100),
        d50: d(c.n50, prev.n50),
        dmax: d(c.geki, prev.geki),
        d200: d(c.katu, prev.katu),
        dmiss: d(c.miss, prev.miss),
        c300: c.n300,
        c100: c.n100,
        c50: c.n50,
        cmax: c.geki,
        c200: c.katu,
        cmiss: c.miss,
        score: r.score,
        max_combo: r.max_combo,
        combo: r.combo,
        hp_raw: r.hp_raw,
        b4: r.b4,
        b25: r.b25,
        b28: r.b28,
        f0: r.v2.map(|[f0, _]| f0),
        f1: r.v2.map(|[_, f1]| f1),
    }
}

fn event_row(e: &OsgEvent) -> OsgEventRow {
    OsgEventRow {
        idx: e.idx,
        t_ms: e.t_ms,
        kinds: e.kinds().iter().map(|k| k.as_str()).collect(),
        n: e.n,
        combo_delta: e.combo_delta,
        score_delta: e.score_delta,
        score_only: e.is_score_only(),
    }
}

/// One line per diagnostic, for stderr; warnings never change the exit code.
pub fn render_diagnostics(dump: &OsgDump) -> Vec<String> {
    dump.diagnostics
        .iter()
        .map(|d| match d.offset {
            Some(offset) => format!("{} @{offset}: {}", d.code, d.detail),
            None => format!("{}: {}", d.code, d.detail),
        })
        .collect()
}

/// `limit` caps the rows shown, never the header or the diagnostics.
pub fn render_dump(
    dump: &OsgDump,
    format: DumpFormat,
    view: DumpView,
    limit: Option<usize>,
) -> Result<String, AppError> {
    let take = |len: usize| limit.map_or(len, |l| l.min(len));
    let records = &dump.records[..take(dump.records.len())];
    let events = &dump.events[..take(dump.events.len())];
    if format == DumpFormat::Json {
        return render_json(dump, view, records, events);
    }
    let kind_sep = match format {
        DumpFormat::Csv => CSV_KIND_SEPARATOR,
        _ => TABLE_KIND_SEPARATOR,
    };
    let (columns, rows): (Vec<&str>, Vec<Vec<String>>) = match view {
        DumpView::Records => (
            record_columns(dump),
            records.iter().map(record_cells).collect(),
        ),
        DumpView::Events => (
            EVENT_COLUMNS.to_vec(),
            events.iter().map(|e| event_cells(e, kind_sep)).collect(),
        ),
    };
    let header = header_line(&dump.header);
    Ok(match format {
        DumpFormat::Csv => csv(&header, &columns, &rows),
        _ => table(&header, &columns, &rows),
    })
}

#[derive(Serialize)]
struct JsonDump<'a> {
    header: &'a OsgDumpHeader,
    diagnostics: &'a [DiagnosticRow],
    #[serde(skip_serializing_if = "Option::is_none")]
    records: Option<&'a [OsgRecordRow]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    events: Option<&'a [OsgEventRow]>,
}

fn render_json(
    dump: &OsgDump,
    view: DumpView,
    records: &[OsgRecordRow],
    events: &[OsgEventRow],
) -> Result<String, AppError> {
    let out = JsonDump {
        header: &dump.header,
        diagnostics: &dump.diagnostics,
        records: (view == DumpView::Records).then_some(records),
        events: (view == DumpView::Events).then_some(events),
    };
    serde_json::to_string_pretty(&out).map_err(|e| AppError::internal(e.to_string()))
}

fn header_line(h: &OsgDumpHeader) -> String {
    format!(
        "client_version={} record_count={} stride={} score_system={} file_size={} diagnostics={}",
        h.client_version,
        h.record_count,
        h.stride
            .map_or_else(|| MISSING.to_owned(), |s| s.to_string()),
        h.score_system.unwrap_or(MISSING),
        h.file_size,
        h.diagnostics,
    )
}

fn record_columns(dump: &OsgDump) -> Vec<&'static str> {
    let mut columns = V1_COLUMNS.to_vec();
    if dump.header.score_system == Some("v2") {
        columns.extend(V2_COLUMNS);
    }
    columns
}

fn record_cells(r: &OsgRecordRow) -> Vec<String> {
    let mut cells: Vec<String> = [
        i64::try_from(r.idx).unwrap_or(i64::MAX),
        i64::from(r.t_ms),
        i64::from(r.d300),
        i64::from(r.d100),
        i64::from(r.d50),
        i64::from(r.dmax),
        i64::from(r.d200),
        i64::from(r.dmiss),
        i64::from(r.c300),
        i64::from(r.c100),
        i64::from(r.c50),
        i64::from(r.cmax),
        i64::from(r.c200),
        i64::from(r.cmiss),
        i64::from(r.score),
        i64::from(r.max_combo),
        i64::from(r.combo),
        i64::from(r.hp_raw),
        i64::from(r.b4),
        i64::from(r.b25),
        i64::from(r.b28),
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    cells.extend(r.f0.iter().chain(&r.f1).map(ToString::to_string));
    cells
}

fn event_cells(e: &OsgEventRow, kind_sep: &str) -> Vec<String> {
    let kinds = if e.score_only {
        SCORE_ONLY.to_owned()
    } else {
        e.kinds.join(kind_sep)
    };
    vec![
        e.idx.to_string(),
        e.t_ms.to_string(),
        kinds,
        e.n.to_string(),
        e.combo_delta.to_string(),
        e.score_delta.to_string(),
    ]
}

/// The header goes in a `#` comment so the rest parses as plain CSV.
fn csv(header: &str, columns: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = format!("# {header}\n{}\n", columns.join(","));
    for row in rows {
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

fn table(header: &str, columns: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.len());
        }
    }
    let mut out = format!("{header}\n");
    let mut line = |cells: &mut dyn Iterator<Item = &str>| {
        let padded: Vec<String> = cells
            .zip(&widths)
            .map(|(cell, w)| format!("{cell:>w$}"))
            .collect();
        let _ = writeln!(out, "{}", padded.join(" "));
    };
    line(&mut columns.iter().copied());
    for row in rows {
        line(&mut row.iter().map(String::as_str));
    }
    out
}
