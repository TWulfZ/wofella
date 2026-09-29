//! Hand-written matchers for the regexes of `research/scripts/audit/labels.py`. Each one mirrors a
//! single Python pattern, including its backtracking quirks. `\d` is ASCII here, while Python's
//! `str` regexes also accept other Unicode decimal digits.

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn digits_end(b: &[u8], from: usize) -> usize {
    b.get(from..).map_or(from, |t| {
        from + t.iter().take_while(|c| c.is_ascii_digit()).count()
    })
}

/// `\d(?:\.\d+)?x \(\d+bpm\)|\bOD\d|\+FLN|\[\d\.\d+x\]$`, searched anywhere.
pub(super) fn has_rate_marker(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len()).any(|i| rate_bpm_at(b, i) || od_at(s, i))
        || s.contains("+FLN")
        || ends_with_bracket_rate(b)
}

fn rate_bpm_at(b: &[u8], i: usize) -> bool {
    if !b[i].is_ascii_digit() {
        return false;
    }
    let mut j = i + 1;
    if b.get(j) == Some(&b'.') && b.get(j + 1).is_some_and(u8::is_ascii_digit) {
        j = digits_end(b, j + 1);
    }
    if !b[j..].starts_with(b"x (") {
        return false;
    }
    let bpm = digits_end(b, j + 3);
    bpm > j + 3 && b[bpm..].starts_with(b"bpm)")
}

fn od_at(s: &str, i: usize) -> bool {
    let b = s.as_bytes();
    b[i..].starts_with(b"OD")
        && b.get(i + 2).is_some_and(u8::is_ascii_digit)
        && !s[..i].chars().next_back().is_some_and(is_word_char)
}

fn ends_with_bracket_rate(b: &[u8]) -> bool {
    let Some(body) = b.strip_suffix(b"x]") else {
        return false;
    };
    let frac = body.iter().rev().take_while(|c| c.is_ascii_digit()).count();
    let head = &body[..body.len() - frac];
    frac > 0 && matches!(head, [.., b'[', d, b'.'] if d.is_ascii_digit())
}

/// `^(\d+)(st|nd|rd|th)` on an already trimmed and lowercased string; returns the matched prefix.
pub(super) fn ordinal_prefix(s: &str) -> Option<(&str, &str)> {
    let end = digits_end(s.as_bytes(), 0);
    let suffix = s.get(end..end + 2)?;
    (end > 0 && matches!(suffix, "st" | "nd" | "rd" | "th")).then(|| (&s[..end], &s[..end + 2]))
}

/// Group 1 of `^[~-]\s*([^~]+?)\s*~` (the caller has checked the `[~-]` prefix). Empty when the
/// pattern does not match, like `m.group(1) if m else ''`.
pub(super) fn tilde_level(v: &str) -> &str {
    let mut chars = v.chars();
    chars.next();
    let rest = chars.as_str();
    let Some(close) = rest.find('~') else {
        return "";
    };
    let segment = &rest[..close];
    let inner = segment.trim_start();
    if !inner.is_empty() {
        return inner.trim_end();
    }
    // All blank: the greedy `\s*` hands its last whitespace char back to the lazy `+?` group.
    segment
        .char_indices()
        .next_back()
        .map_or("", |(at, _)| &segment[at..])
}

/// `re.findall(r'\[([a-z]+\d?)_([^\]]+)\]', v)`: (table, level) pairs, left to right, non-overlapping.
pub(super) fn bms_tags(v: &str) -> Vec<(&str, &str)> {
    let b = v.as_bytes();
    let mut tags = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match bms_tag_at(b, i) {
            Some((table_end, level_start, close)) => {
                tags.push((&v[i + 1..table_end], &v[level_start..close]));
                i = close + 1;
            }
            None => i += 1,
        }
    }
    tags
}

fn bms_tag_at(b: &[u8], i: usize) -> Option<(usize, usize, usize)> {
    if b[i] != b'[' {
        return None;
    }
    let letters = b[i + 1..]
        .iter()
        .take_while(|c| c.is_ascii_lowercase())
        .count();
    if letters == 0 {
        return None;
    }
    let mut table_end = i + 1 + letters;
    if b.get(table_end).is_some_and(u8::is_ascii_digit) && b.get(table_end + 1) == Some(&b'_') {
        table_end += 1;
    }
    if b.get(table_end) != Some(&b'_') {
        return None;
    }
    let level_start = table_end + 1;
    let close = level_start + b[level_start..].iter().position(|&c| c == b']')?;
    (close > level_start).then_some((table_end, level_start, close))
}

/// `^(\d+)([+-]?)$` → (number, sign).
pub(super) fn signed_level(lv: &str) -> Option<(&str, Option<char>)> {
    let end = digits_end(lv.as_bytes(), 0);
    if end == 0 {
        return None;
    }
    match &lv[end..] {
        "" => Some((&lv[..end], None)),
        "+" => Some((&lv[..end], Some('+'))),
        "-" => Some((&lv[..end], Some('-'))),
        _ => None,
    }
}

/// `re.match(r'\[O2Jam\] \[(\w)\] \[(\d+)\]', v)` → (difficulty char, level digits).
pub(super) fn o2jam_header(v: &str) -> Option<(char, &str)> {
    let rest = v.strip_prefix("[O2Jam] [")?;
    let mut chars = rest.chars();
    let diff = chars.next().filter(|&c| is_word_char(c))?;
    let rest = chars.as_str().strip_prefix("] [")?;
    let end = digits_end(rest.as_bytes(), 0);
    (end > 0 && rest[end..].starts_with(']')).then(|| (diff, &rest[..end]))
}

/// Digits of the leftmost `{open}(\d+){close}`.
pub(super) fn first_number_between<'a>(v: &'a str, open: &str, close: &str) -> Option<&'a str> {
    v.match_indices(open).find_map(|(at, _)| {
        let from = at + open.len();
        let end = digits_end(v.as_bytes(), from);
        (end > from && v[end..].starts_with(close)).then(|| &v[from..end])
    })
}

/// Group 1 of the leftmost `\((A|B|…)\)` over `words`.
pub(super) fn first_parenthesized<'w>(v: &str, words: &[&'w str]) -> Option<&'w str> {
    v.match_indices('(').find_map(|(at, _)| {
        let rest = &v[at + 1..];
        words
            .iter()
            .find(|w| rest.strip_prefix(**w).is_some_and(|r| r.starts_with(')')))
            .copied()
    })
}
