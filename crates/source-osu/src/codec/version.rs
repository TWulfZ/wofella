//! Format kinds and the ADR 0015 version policy.

use crate::stable::stable_str_enum;

stable_str_enum! {
    /// Append-only: 006 adds `osg`.
    pub enum FileKind {
        OsuDb => "osu_db",
        ScoresDb => "scores_db",
        CollectionDb => "collection_db",
        Osr => "osr",
        Cfg => "cfg",
    }
}
