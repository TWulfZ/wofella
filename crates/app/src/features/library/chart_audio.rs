//! The chart audio MIME type, one the webview's decoder accepts (ADR 0018).

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

#[cfg(test)]
mod tests {
    use super::*;

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
