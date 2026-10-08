use wolluf_audio::AudioParams;
use wolluf_engine::RateCopyParams;
use wolluf_engine::stage::difficulty::MinaCalcParams;

#[derive(Debug, Clone, PartialEq)]
pub struct RateCopiesParams {
    pub rewrite: RateCopyParams,
    pub audio: AudioParams,
    /// Above any osu! song; a larger file is refused rather than decoded into memory.
    pub max_audio_bytes: u64,
    /// A storyboard larger than this is not read, and the set is refused as keysounded.
    pub max_storyboard_bytes: u64,
    /// A file at the copy's `.osu` name larger than this is not read and counts as taken.
    pub max_chart_bytes: u64,
    /// Two objects of one column this close in the copy are a collision (the drills rewriter
    /// rounds to whole ms, ADR 0025).
    pub collision_window_us: i64,
    /// The difficulty stage's own LN cut-off, used while a chart has no MSD status yet.
    pub ln_heavy_hold_share_permille: u16,
}

const MIB: u64 = 1024 * 1024;

impl Default for RateCopiesParams {
    fn default() -> Self {
        Self {
            rewrite: RateCopyParams::default(),
            audio: AudioParams::default(),
            max_audio_bytes: 64 * MIB,
            max_storyboard_bytes: 32 * MIB,
            max_chart_bytes: 16 * MIB,
            collision_window_us: 1_000,
            ln_heavy_hold_share_permille: MinaCalcParams::default().ln_unrated_hold_share_permille,
        }
    }
}
