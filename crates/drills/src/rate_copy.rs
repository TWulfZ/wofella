//! Byte-level `.osu` rewriter for rate copies (ADR 0025). It never round-trips through the chart
//! model, so every line it does not retime stays byte-identical.

use std::borrow::Cow;

use crate::decimal::{Decimal, format_fixed};
use crate::error::DrillError;

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
/// What stable writes, used only for lines we insert into a file that has no line break at all.
const DEFAULT_EOL: &[u8] = b"\r\n";
const MANIA_MODE: i64 = 3;
const TYPE_SLIDER: i64 = 2;
const TYPE_SPINNER: i64 = 8;
const TYPE_HOLD: i64 = 128;
/// Timing offsets may be fractional in stable; at least three decimals keep sub-ms placement
/// exact enough, and the source's own decimals are never cut.
const MIN_OFFSET_DECIMALS: u32 = 3;
const IDENTITY_RATE_MILLI: u16 = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateCopyParams {
    /// Appended to `Tags`; the library finds generated copies again by it (ADR 0025).
    pub tag: String,
    pub min_rate_milli: u16,
    pub max_rate_milli: u16,
    /// NC copy: its audio follows the rate in pitch, so it gets its own audio and version names
    /// and never shares files with the pitch-kept copy of the same rate (ADR 0025).
    pub nightcore: bool,
}

impl Default for RateCopyParams {
    fn default() -> Self {
        Self {
            tag: "wofella".to_owned(),
            min_rate_milli: 500,
            max_rate_milli: 2000,
            nightcore: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateCopy {
    pub osu: Vec<u8>,
    pub version: String,
    pub audio_filename: String,
    pub source_audio: String,
    pub osu_filename: String,
}

/// Rewrites a mania `.osu` so that every time becomes `round_half_up(t / r)`, `r = rate_milli /
/// 1000`. Video lines are dropped: the copy's audio is stretched but the video would not be.
///
/// Integer rounding can land two objects that were 1-2 ms apart on the same millisecond at high
/// rates; the app checks the copy for same-column collisions, this rewriter does not.
pub fn rate_copy(
    osu: &[u8],
    rate_milli: u16,
    params: &RateCopyParams,
) -> Result<RateCopy, DrillError> {
    if rate_milli == IDENTITY_RATE_MILLI {
        return Err(DrillError::IdentityRate);
    }
    if rate_milli == 0 || rate_milli < params.min_rate_milli || rate_milli > params.max_rate_milli {
        return Err(DrillError::RateOutOfRange { rate_milli });
    }
    let (bom, body) = match osu.strip_prefix(BOM) {
        Some(rest) => (BOM, rest),
        None => (&[][..], osu),
    };
    let lines = split_lines(body);
    let file_eol = lines
        .iter()
        .map(|l| l.eol)
        .find(|e| !e.is_empty())
        .unwrap_or(DEFAULT_EOL);

    let label = rate_label(rate_milli);
    let mut rw = Rewriter {
        rate_milli,
        label: &label,
        nightcore: params.nightcore,
        tag: params.tag.as_bytes(),
        st: Collected::default(),
    };
    let mut out: Vec<OutLine<'_>> = Vec::with_capacity(lines.len() + 2);
    let mut section = Section::Other;
    let mut first_error: Option<DrillError> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.content.trim_ascii();
        if let Some(name) = section_name(trimmed) {
            section = Section::from_name(name);
            if section == Section::Metadata {
                rw.st.metadata_seen = true;
                rw.st.metadata_tail = out.len();
            }
            out.push(OutLine::keep(line));
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with(b"//") {
            out.push(OutLine::keep(line));
            continue;
        }
        match rw.line(section, line.content, i + 1, out.len()) {
            Ok(Some(Cow::Borrowed(_))) => out.push(OutLine::keep(line)),
            Ok(Some(Cow::Owned(content))) => out.push(OutLine {
                content: Cow::Owned(content),
                eol: line.eol,
            }),
            Ok(None) => {}
            Err(e) => {
                first_error.get_or_insert(e);
                out.push(OutLine::keep(line));
            }
        }
        if section == Section::Metadata {
            rw.st.metadata_tail = out.len().saturating_sub(1);
        }
    }

    let st = rw.st;
    if st.mode != Some(MANIA_MODE) {
        return Err(DrillError::UnsupportedMode);
    }
    if st.already_copy {
        return Err(DrillError::AlreadyRateCopy);
    }
    if let Some(e) = first_error {
        return Err(e);
    }
    let Some((source_audio, audio_filename)) = st.audio else {
        return Err(DrillError::NoAudio);
    };
    if !st.metadata_seen {
        return Err(DrillError::Malformed { line: 0 });
    }
    let bpm = dominant_bpm(&st.red_lines, st.last_time, rate_milli)
        .ok_or(DrillError::Malformed { line: 0 })?;

    let orig_version = st.version.as_ref().map_or(&[][..], |v| v.value.as_slice());
    let mut version = orig_version.to_vec();
    if !version.is_empty() {
        version.push(b' ');
    }
    let mod_name = if params.nightcore { " NC" } else { "" };
    version.extend_from_slice(format!("{label}x{mod_name} ({bpm}bpm)").as_bytes());

    let mut inserts: Vec<Vec<u8>> = Vec::new();
    match &st.version {
        Some(v) => {
            let mut content = v.prefix.clone();
            content.extend_from_slice(&version);
            out[v.out_index].content = Cow::Owned(content);
        }
        None => inserts.push([b"Version:".as_slice(), &version].concat()),
    }
    if !st.tags_seen {
        inserts.push([b"Tags:".as_slice(), params.tag.as_bytes()].concat());
    }
    insert_after(&mut out, st.metadata_tail, inserts, file_eol);

    let version = String::from_utf8_lossy(&version).into_owned();
    let osu_filename = sanitize_filename(&format!(
        "{} - {} ({}) [{version}].osu",
        String::from_utf8_lossy(&st.artist),
        String::from_utf8_lossy(&st.title),
        String::from_utf8_lossy(&st.creator),
    ));

    let mut bytes = Vec::with_capacity(osu.len() + 64);
    bytes.extend_from_slice(bom);
    for line in &out {
        bytes.extend_from_slice(&line.content);
        bytes.extend_from_slice(line.eol);
    }
    Ok(RateCopy {
        osu: bytes,
        version,
        audio_filename,
        source_audio,
        osu_filename,
    })
}

struct Line<'a> {
    content: &'a [u8],
    eol: &'a [u8],
}

struct OutLine<'a> {
    content: Cow<'a, [u8]>,
    eol: &'a [u8],
}

impl<'a> OutLine<'a> {
    fn keep(line: &Line<'a>) -> Self {
        Self {
            content: Cow::Borrowed(line.content),
            eol: line.eol,
        }
    }
}

/// Breaks on CRLF, LF and a lone CR, as stable's .NET `ReadLine` does. Each line keeps its own
/// terminator, so mixed files survive unchanged.
fn split_lines(body: &[u8]) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let Some(at) = rest.iter().position(|&b| b == b'\n' || b == b'\r') else {
            lines.push(Line {
                content: rest,
                eol: &[],
            });
            break;
        };
        let eol_len = if rest[at..].starts_with(b"\r\n") {
            2
        } else {
            1
        };
        lines.push(Line {
            content: &rest[..at],
            eol: &rest[at..at + eol_len],
        });
        rest = &rest[at + eol_len..];
    }
    lines
}

fn insert_after<'a>(out: &mut Vec<OutLine<'a>>, at: usize, lines: Vec<Vec<u8>>, eol: &'a [u8]) {
    for (pos, content) in (at + 1..).zip(lines) {
        if out[pos - 1].eol.is_empty() {
            // The anchor was the last line of a file without a final break.
            out[pos - 1].eol = eol;
        }
        let line_eol = if pos == out.len() { &[][..] } else { eol };
        out.insert(
            pos,
            OutLine {
                content: Cow::Owned(content),
                eol: line_eol,
            },
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Editor,
    Metadata,
    Events,
    TimingPoints,
    HitObjects,
    Other,
}

impl Section {
    fn from_name(name: &[u8]) -> Self {
        match name {
            b"General" => Self::General,
            b"Editor" => Self::Editor,
            b"Metadata" => Self::Metadata,
            b"Events" => Self::Events,
            b"TimingPoints" => Self::TimingPoints,
            b"HitObjects" => Self::HitObjects,
            _ => Self::Other,
        }
    }
}

fn section_name(trimmed: &[u8]) -> Option<&[u8]> {
    trimmed.strip_prefix(b"[")?.strip_suffix(b"]")
}

struct VersionLine {
    out_index: usize,
    prefix: Vec<u8>,
    value: Vec<u8>,
}

#[derive(Default)]
struct Collected {
    mode: Option<i64>,
    /// (original name, copy's name)
    audio: Option<(String, String)>,
    title: Vec<u8>,
    artist: Vec<u8>,
    creator: Vec<u8>,
    version: Option<VersionLine>,
    tags_seen: bool,
    already_copy: bool,
    metadata_seen: bool,
    /// Index in the output of the last content line of `[Metadata]`, where missing keys go.
    metadata_tail: usize,
    /// (offset, beat length) in source time, positive beat lengths only.
    red_lines: Vec<(f64, f64)>,
    last_time: Option<f64>,
}

struct Rewriter<'p> {
    rate_milli: u16,
    label: &'p str,
    nightcore: bool,
    tag: &'p [u8],
    st: Collected,
}

/// `Ok(None)` drops the line; `Ok(Some(Borrowed))` keeps it byte-identical.
type LineResult<'a> = Result<Option<Cow<'a, [u8]>>, DrillError>;

impl Rewriter<'_> {
    fn line<'a>(
        &mut self,
        section: Section,
        content: &'a [u8],
        line_no: usize,
        out_index: usize,
    ) -> LineResult<'a> {
        let malformed = DrillError::Malformed { line: line_no };
        match section {
            Section::General => self.general(content).ok_or(malformed),
            Section::Editor => self.editor(content).ok_or(malformed),
            Section::Metadata => Ok(self.metadata(content, out_index)),
            Section::Events => self.event(content, malformed),
            Section::TimingPoints => {
                let text = std::str::from_utf8(content).map_err(|_| malformed.clone())?;
                self.timing_point(text).map(Some).ok_or(malformed)
            }
            Section::HitObjects => {
                let text = std::str::from_utf8(content).map_err(|_| malformed.clone())?;
                self.hit_object(text, malformed)
            }
            Section::Other => Ok(Some(Cow::Borrowed(content))),
        }
    }

    fn scale_int(&self, field: &str) -> Option<String> {
        Some(
            Decimal::parse(field)?
                .div_rate(self.rate_milli, 0)?
                .to_string(),
        )
    }

    fn scale_int_bytes(&self, field: &[u8]) -> Option<Vec<u8>> {
        Some(
            self.scale_int(std::str::from_utf8(field).ok()?)?
                .into_bytes(),
        )
    }

    /// Full `f64` precision: beat lengths and frame delays are durations, not grid times.
    fn scale_f64(&self, field: &str) -> Option<String> {
        let v: f64 = field.trim().parse().ok()?;
        let v = v / (f64::from(self.rate_milli) / 1000.0);
        v.is_finite().then(|| v.to_string())
    }

    fn general<'a>(&mut self, content: &'a [u8]) -> Option<Option<Cow<'a, [u8]>>> {
        let keep = Some(Some(Cow::Borrowed(content)));
        let Some(kv) = KeyValue::split(content) else {
            return keep;
        };
        match kv.key {
            b"Mode" => {
                self.st.mode = Some(std::str::from_utf8(kv.value).ok()?.parse().ok()?);
                keep
            }
            b"AudioFilename" => {
                let name = std::str::from_utf8(kv.value).ok()?;
                // `virtual` is stable's marker for a chart with no music track.
                if name.is_empty() || name.eq_ignore_ascii_case("virtual") {
                    return keep;
                }
                let base = strip_extension(name);
                let nc = if self.nightcore { " nc" } else { "" };
                let copy_name = format!("{base} {}x{nc}.ogg", self.label);
                let line = kv.with_value(copy_name.as_bytes());
                self.st.audio = Some((name.to_owned(), copy_name));
                Some(Some(Cow::Owned(line)))
            }
            b"PreviewTime" => {
                let value = std::str::from_utf8(kv.value).ok()?;
                if value == "-1" {
                    return keep;
                }
                let scaled = self.scale_int(value)?;
                Some(Some(Cow::Owned(kv.with_value(scaled.as_bytes()))))
            }
            _ => keep,
        }
    }

    fn editor<'a>(&self, content: &'a [u8]) -> Option<Option<Cow<'a, [u8]>>> {
        let keep = Some(Some(Cow::Borrowed(content)));
        let Some(kv) = KeyValue::split(content) else {
            return keep;
        };
        if kv.key != b"Bookmarks" || kv.value.is_empty() {
            return keep;
        }
        // Bookmarks are editor-only, so an unreadable list is kept rather than blocking the copy.
        let scaled = std::str::from_utf8(kv.value).ok().and_then(|v| {
            v.split(',')
                .filter(|t| !t.trim().is_empty())
                .map(|t| self.scale_int(t))
                .collect::<Option<Vec<_>>>()
        });
        match scaled {
            Some(scaled) => Some(Some(Cow::Owned(kv.with_value(scaled.join(",").as_bytes())))),
            None => keep,
        }
    }

    fn metadata<'a>(&mut self, content: &'a [u8], out_index: usize) -> Option<Cow<'a, [u8]>> {
        let keep = Some(Cow::Borrowed(content));
        let Some(kv) = KeyValue::split(content) else {
            return keep;
        };
        match kv.key {
            b"Title" => self.st.title = kv.value.to_vec(),
            b"Artist" => self.st.artist = kv.value.to_vec(),
            b"Creator" => self.st.creator = kv.value.to_vec(),
            b"Version" => {
                if self.st.version.is_none() {
                    self.st.version = Some(VersionLine {
                        out_index,
                        prefix: content[..kv.value_start].to_vec(),
                        value: kv.value.to_vec(),
                    });
                }
            }
            b"Tags" => {
                self.st.tags_seen = true;
                if kv
                    .value
                    .split(u8::is_ascii_whitespace)
                    .any(|t| !t.is_empty() && t == self.tag)
                {
                    self.st.already_copy = true;
                    return keep;
                }
                let mut value = kv.value.to_vec();
                if !value.is_empty() {
                    value.push(b' ');
                }
                value.extend_from_slice(self.tag);
                return Some(Cow::Owned(kv.with_value(&value)));
            }
            // Online ids would point stable at the original difficulty's leaderboard and updates.
            b"BeatmapID" => return Some(Cow::Owned(kv.with_value(b"0"))),
            _ => {}
        }
        keep
    }

    /// Works on bytes: kept lines may carry non-UTF-8 file names, and only the numeric fields
    /// being retimed need to decode.
    fn event<'a>(&self, content: &'a [u8], malformed: DrillError) -> LineResult<'a> {
        if content.starts_with(b" ") || content.starts_with(b"_") {
            return self
                .command(content)
                .map(|l| Some(Cow::Owned(l)))
                .ok_or(malformed);
        }
        let mut fields = split_quoted(content);
        let scaled: &[usize] = match fields[0].trim_ascii() {
            b"1" | b"Video" => return Ok(None),
            b"5" | b"Sample" => return Err(DrillError::Keysounded),
            b"2" | b"Break" => &[1, 2],
            b"3" | b"Colour" => &[1],
            b"6" | b"Animation" => {
                let delay = fields
                    .get(7)
                    .and_then(|f| std::str::from_utf8(f).ok())
                    .and_then(|f| self.scale_f64(f))
                    .ok_or(malformed)?;
                fields[7] = Cow::Owned(delay.into_bytes());
                return Ok(Some(Cow::Owned(fields.join(&b','))));
            }
            _ => return Ok(Some(Cow::Borrowed(content))),
        };
        for &i in scaled {
            let t = fields.get(i).and_then(|f| self.scale_int_bytes(f));
            fields[i] = Cow::Owned(t.ok_or_else(|| malformed.clone())?);
        }
        Ok(Some(Cow::Owned(fields.join(&b','))))
    }

    /// Storyboard commands. Loop-relative times are durations, so scaling them too is exact.
    fn command(&self, content: &[u8]) -> Option<Vec<u8>> {
        let depth_len = content
            .iter()
            .position(|&b| b != b' ' && b != b'_')
            .unwrap_or(content.len());
        let (depth, body) = content.split_at(depth_len);
        let mut fields: Vec<Cow<'_, [u8]>> =
            body.split(|&b| b == b',').map(Cow::Borrowed).collect();
        let (required, optional): (&[usize], &[usize]) = match &*fields[0] {
            b"F" | b"M" | b"MX" | b"MY" | b"S" | b"V" | b"R" | b"C" | b"P" => (&[2], &[3]),
            b"L" => (&[1], &[]),
            b"T" => (&[2], &[3]),
            _ => return None,
        };
        for &i in required {
            fields[i] = Cow::Owned(self.scale_int_bytes(fields.get(i)?)?);
        }
        for &i in optional {
            if let Some(f) = fields.get(i).filter(|f| !f.trim_ascii().is_empty()) {
                fields[i] = Cow::Owned(self.scale_int_bytes(f)?);
            }
        }
        Some([depth, &fields.join(&b',')].concat())
    }

    fn timing_point(&mut self, text: &str) -> Option<Cow<'static, [u8]>> {
        let mut fields: Vec<Cow<'_, str>> = text.split(',').map(Cow::Borrowed).collect();
        let offset = Decimal::parse(fields.first()?)?;
        let beat_len: f64 = fields.get(1)?.trim().parse().ok()?;
        // Pre-v6 lines carry no uninherited flag; a negative beat length there is still an SV.
        let red = match fields.get(6) {
            Some(flag) => flag.trim_start().starts_with('1'),
            None => beat_len > 0.0,
        };
        if red {
            if beat_len > 0.0 {
                let source_offset: f64 = fields[0].trim().parse().ok()?;
                self.st.red_lines.push((source_offset, beat_len));
            }
            fields[1] = Cow::Owned(self.scale_f64(&fields[1])?);
        }
        let decimals = offset.scale().max(MIN_OFFSET_DECIMALS);
        fields[0] = Cow::Owned(format_fixed(
            offset.div_rate(self.rate_milli, decimals)?,
            decimals,
        ));
        Some(Cow::Owned(fields.join(",").into_bytes()))
    }

    fn hit_object<'a>(&mut self, text: &'a str, malformed: DrillError) -> LineResult<'a> {
        let raw: Vec<&str> = text.split(',').collect();
        if raw.len() < 4 {
            return Err(malformed);
        }
        let kind: i64 = raw[3].trim().parse().map_err(|_| malformed.clone())?;
        let mut fields: Vec<Cow<'_, str>> = raw.iter().map(|&f| Cow::Borrowed(f)).collect();

        let (end, hit_sample) = if kind & TYPE_HOLD != 0 {
            // Only a hold carries `endTime:` in front of its hit sample; a tap's first hit-sample
            // value is a sample set, which Companella wrongly retimes.
            let field = raw.get(5).ok_or_else(|| malformed.clone())?;
            match field.split_once(':') {
                Some((end, sample)) => {
                    let scaled = self.scale_int(end).ok_or_else(|| malformed.clone())?;
                    fields[5] = Cow::Owned(format!("{scaled}:{sample}"));
                    (Some(end), Some(sample))
                }
                None => {
                    fields[5] = Cow::Owned(self.scale_int(field).ok_or_else(|| malformed.clone())?);
                    (Some(*field), None)
                }
            }
        } else if kind & TYPE_SPINNER != 0 {
            let end = raw.get(5).ok_or_else(|| malformed.clone())?;
            fields[5] = Cow::Owned(self.scale_int(end).ok_or_else(|| malformed.clone())?);
            (Some(*end), raw.get(6).copied())
        } else if kind & TYPE_SLIDER != 0 {
            (None, raw.get(10).copied())
        } else {
            (None, raw.get(5).copied())
        };
        if hit_sample.and_then(sample_filename).is_some() {
            return Err(DrillError::Keysounded);
        }

        for t in std::iter::once(raw[2]).chain(end) {
            let t: f64 = t.trim().parse().map_err(|_| malformed.clone())?;
            self.st.last_time = Some(self.st.last_time.map_or(t, |m| m.max(t)));
        }
        fields[2] = Cow::Owned(self.scale_int(raw[2]).ok_or(malformed)?);
        Ok(Some(Cow::Owned(fields.join(",").into_bytes())))
    }
}

/// The custom sample file of a hit-sample field (`normal:addition:index:volume:filename`), if
/// any: that is what makes a chart keysounded.
fn sample_filename(sample: &str) -> Option<&str> {
    sample
        .split(':')
        .nth(4)
        .map(str::trim)
        .filter(|f| !f.is_empty())
}

/// Event fields, with commas inside quoted file paths kept in their field.
fn split_quoted(content: &[u8]) -> Vec<Cow<'_, [u8]>> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    for (i, &b) in content.iter().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b',' if !quoted => {
                fields.push(Cow::Borrowed(&content[start..i]));
                start = i + 1;
            }
            _ => {}
        }
    }
    fields.push(Cow::Borrowed(&content[start..]));
    fields
}

/// Drops the extension of the file-name component only; a dot in a folder name or a leading
/// dot is not one.
fn strip_extension(path: &str) -> &str {
    let name_start = path.rfind(['/', '\\']).map_or(0, |i| i + 1);
    match path[name_start..].rfind('.') {
        Some(dot) if dot > 0 => &path[..name_start + dot],
        _ => path,
    }
}

struct KeyValue<'a> {
    content: &'a [u8],
    key: &'a [u8],
    value_start: usize,
    value: &'a [u8],
}

impl<'a> KeyValue<'a> {
    fn split(content: &'a [u8]) -> Option<Self> {
        let colon = content.iter().position(|&b| b == b':')?;
        let after = &content[colon + 1..];
        let pad = after.len() - after.trim_ascii_start().len();
        let value_start = colon + 1 + pad;
        Some(Self {
            content,
            key: content[..colon].trim_ascii(),
            value_start,
            value: content[value_start..].trim_ascii_end(),
        })
    }

    /// Keeps the key and its original separator spacing.
    fn with_value(&self, value: &[u8]) -> Vec<u8> {
        [&self.content[..self.value_start], value].concat()
    }
}

/// `1.15`, `1.10`; a third decimal only when the rate has one, so two rates never share a name.
fn rate_label(rate_milli: u16) -> String {
    let (int, frac) = (rate_milli / 1000, rate_milli % 1000);
    if frac % 10 == 0 {
        format!("{int}.{:02}", frac / 10)
    } else {
        format!("{int}.{frac:03}")
    }
}

/// BPM of the red line covering the most play time, in the copy's time base. Mirrors stable's
/// "most common beat length": the first red line counts from 0, the last runs to the last object,
/// and red lines after the last object count for nothing.
fn dominant_bpm(red_lines: &[(f64, f64)], last_time: Option<f64>, rate_milli: u16) -> Option<i64> {
    let mut lines = red_lines.to_vec();
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    let last_time = last_time.unwrap_or(0.0);

    // (beat length rounded to 1e-3 ms, beat length, covered ms), in first-appearance order.
    let mut groups: Vec<(i64, f64, f64)> = Vec::new();
    for (i, &(offset, beat_len)) in lines.iter().enumerate() {
        let covered = if offset > last_time {
            0.0
        } else {
            let start = if i == 0 { 0.0 } else { offset };
            let end = lines.get(i + 1).map_or(last_time, |next| next.0);
            (end - start).max(0.0)
        };
        let key = (beat_len * 1000.0).round() as i64;
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.2 += covered,
            None => groups.push((key, beat_len, covered)),
        }
    }
    let mut best = groups.first()?;
    for g in &groups[1..] {
        if g.2 > best.2 {
            best = g;
        }
    }
    let bpm = 60.0 * f64::from(rate_milli) / best.1;
    Some((bpm + 0.5).floor() as i64)
}

/// Strips what Windows rejects in a file name.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
        .filter(|c| !c.is_control())
        .collect()
}
