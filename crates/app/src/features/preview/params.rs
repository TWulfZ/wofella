use wolluf_engine::preview::PreviewParams;

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewServiceParams {
    pub engine: PreviewParams,
    /// The skill page lists the best counted plays, not every one.
    pub top_plays: usize,
}

impl Default for PreviewServiceParams {
    fn default() -> Self {
        Self {
            engine: PreviewParams::default(),
            top_plays: 50,
        }
    }
}
