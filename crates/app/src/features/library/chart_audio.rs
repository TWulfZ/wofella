//! The chart audio payload: a MIME type the webview's decoder accepts and the bytes as base64,
//! because a typed command cannot return raw bytes (ADR 0018).

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// mp3 and ogg cover 99.7% of the pilot's library (research 03 l.222); wav is listed for
/// hand-made sets.
pub(super) fn mime_of(file_name: &str) -> &'static str {
    let ext = file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match ext.as_deref() {
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// RFC 4648 §4 with padding. Hand-rolled because no workspace crate depends on a base64 crate
/// and a new dependency is a root manifest change.
pub(super) fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(BASE64_ALPHABET[(n >> shift & 0x3f) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_audio_base64_matches_rfc_4648_vectors() {
        for (raw, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(raw.as_bytes()), encoded, "{raw:?}");
        }
        assert_eq!(base64(&[0xfb, 0xff, 0xbf]), "+/+/", "the last two symbols");
    }

    #[test]
    fn chart_audio_mime_follows_the_extension() {
        assert_eq!(mime_of("audio.mp3"), "audio/mpeg");
        assert_eq!(mime_of("Song.MP3"), "audio/mpeg");
        assert_eq!(mime_of("a.b.ogg"), "audio/ogg");
        assert_eq!(mime_of("hit.WAV"), "audio/wav");
        assert_eq!(mime_of("track.flac"), "application/octet-stream");
        assert_eq!(mime_of("mp3"), "application/octet-stream");
    }
}
