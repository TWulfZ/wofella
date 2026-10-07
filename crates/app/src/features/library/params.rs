//! Library limits (D17).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryParams {
    /// Above this a chart's audio is refused before it is read: base64 makes the IPC payload a
    /// third larger, and a full-length song is a few MiB.
    pub max_audio_bytes: u64,
    /// Above this a chart's background is left out rather than read; a 1080p JPEG is well
    /// under it.
    pub max_background_bytes: u64,
    /// Entries listed per folder while matching a background name case-insensitively;
    /// hitsound-heavy sets reach a few hundred files, storyboards a few thousand.
    pub max_background_dir_entries: usize,
    /// Path components a background reference may have, file name included.
    pub max_background_depth: usize,
}

impl Default for LibraryParams {
    fn default() -> Self {
        Self {
            max_audio_bytes: 64 * 1024 * 1024,
            max_background_bytes: 12 * 1024 * 1024,
            max_background_dir_entries: 8_192,
            max_background_depth: 4,
        }
    }
}
