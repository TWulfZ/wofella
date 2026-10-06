//! Skins limits (D17).

use wolluf_source_osu::skins::SkinsParams;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SkinParams {
    /// Caps on what `Skins/` may cost one `skin_get`, and the `skin.ini` format defaults.
    pub read: SkinsParams,
}
