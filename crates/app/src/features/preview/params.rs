use wolluf_engine::preview::PreviewParams;
use wolluf_engine::preview::recs::RecsParams;

#[derive(Debug, Clone)]
pub struct PreviewServiceParams {
    pub engine: PreviewParams,
    /// The skill page lists the best counted plays, not every one.
    pub top_plays: usize,
    /// Per keymode: the Deficit exclusions differ between 4K and 7K.
    pub recs: fn(u8) -> RecsParams,
}

impl Default for PreviewServiceParams {
    fn default() -> Self {
        Self {
            engine: PreviewParams::default(),
            top_plays: 50,
            recs: RecsParams::for_keymode,
        }
    }
}
