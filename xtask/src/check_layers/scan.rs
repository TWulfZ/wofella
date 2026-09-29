//! Source preparation for the L6 banned-API scan: only code tokens can be violations.
//! Comments and string-literal contents are data (docs, messages, xtask's own test fixtures),
//! so they are blanked; char literals are lexed so a `'"'` does not open a string.

/// Replaces comment characters and string-literal contents with spaces, keeping newlines and byte
/// length so offsets map to the original line numbers.
pub(super) fn code_only(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match (c, next) {
            ('/', Some('/')) => {
                while i < chars.len() && chars[i] != '\n' {
                    blank(&mut out, chars[i]);
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                // Rust block comments nest.
                let mut depth = 0usize;
                while i < chars.len() {
                    if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                        depth += 1;
                        blank(&mut out, chars[i]);
                        blank(&mut out, '*');
                        i += 2;
                    } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        blank(&mut out, '*');
                        blank(&mut out, '/');
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        blank(&mut out, chars[i]);
                        i += 1;
                    }
                }
            }
            ('"', _) => i = copy_string(&chars, i, 0, &mut out),
            ('r', Some('"' | '#')) if starts_literal_prefix(&chars, i) => {
                let mut hashes = 0;
                let mut j = i + 1;
                while chars.get(j) == Some(&'#') {
                    hashes += 1;
                    j += 1;
                }
                if chars.get(j) == Some(&'"') {
                    out.extend(&chars[i..j]);
                    i = copy_string(&chars, j, hashes, &mut out);
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            ('\'', _) => i = copy_char_or_lifetime(&chars, i, &mut out),
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

pub(super) fn line_of(text: &str, byte_offset: usize) -> usize {
    text.as_bytes()[..byte_offset]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

fn blank(out: &mut String, c: char) {
    if c == '\n' {
        out.push('\n');
    } else {
        // One space per byte keeps regex byte offsets aligned with the original text.
        out.extend(std::iter::repeat_n(' ', c.len_utf8()));
    }
}

fn prev(chars: &[char], i: usize) -> Option<char> {
    i.checked_sub(1).map(|p| chars[p])
}

/// `r` opens a raw string only as a token start, or right after a `b`/`c` prefix (`br"…"`, `cr"…"`).
fn starts_literal_prefix(chars: &[char], i: usize) -> bool {
    match prev(chars, i) {
        Some('b' | 'c') => !is_ident_char(i.checked_sub(2).map(|p| chars[p])),
        other => !is_ident_char(other),
    }
}

fn is_ident_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Emits a string literal starting at the opening quote with its contents blanked; `hashes > 0`
/// means a raw string (no escapes).
fn copy_string(chars: &[char], start: usize, hashes: usize, out: &mut String) -> usize {
    out.push('"');
    let mut i = start + 1;
    while i < chars.len() {
        let c = chars[i];
        if hashes == 0 && c == '\\' {
            blank(out, c);
            if let Some(&escaped) = chars.get(i + 1) {
                blank(out, escaped);
            }
            i += 2;
            continue;
        }
        i += 1;
        if c != '"' {
            blank(out, c);
            continue;
        }
        let closes = chars[i..]
            .iter()
            .take(hashes)
            .filter(|h| **h == '#')
            .count()
            == hashes;
        if !closes {
            // A quote inside a raw string that lacks the closing hashes is content.
            blank(out, c);
            continue;
        }
        out.push(c);
        out.extend(std::iter::repeat_n('#', hashes));
        return i + hashes;
    }
    i
}

fn copy_char_or_lifetime(chars: &[char], start: usize, out: &mut String) -> usize {
    out.push('\'');
    let is_char_literal = match chars.get(start + 1) {
        Some('\\') => true,
        Some(_) => chars.get(start + 2) == Some(&'\''),
        None => false,
    };
    if !is_char_literal {
        return start + 1;
    }
    let mut i = start + 1;
    while i < chars.len() {
        let c = chars[i];
        out.push(c);
        i += 1;
        if c == '\\' {
            if let Some(&escaped) = chars.get(i) {
                out.push(escaped);
                i += 1;
            }
        } else if c == '\'' {
            break;
        }
    }
    i
}
