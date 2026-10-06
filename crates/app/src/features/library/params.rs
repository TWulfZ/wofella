//! Library limits (D17).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryParams {
    /// Above this a chart's audio is refused before it is read: base64 makes the IPC payload a
    /// third larger, and a full-length song is a few MiB.
    pub max_audio_bytes: u64,
}

impl Default for LibraryParams {
    fn default() -> Self {
        Self {
            max_audio_bytes: 64 * 1024 * 1024,
        }
    }
}
