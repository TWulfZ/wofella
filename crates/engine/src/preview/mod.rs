//! Uncalibrated skill preview (ADR 0024): a play's goal from its judgement counts, Etterna's
//! player rating over per-play SSRs, chart families across rate copies and the 4K dan estimate.
//! Pure: the app feeds counts, OD, mods and centi SSRs from the store.

pub mod dan;
pub mod family;
pub mod goal;
pub mod mods;
pub mod params;
pub mod play;
pub mod rating;
pub mod recs;

pub use dan::{DanEstimate, DanTable4k, DanThird};
pub use family::family_key;
pub use goal::goal_permyriad;
pub use mods::PlayMods;
pub use params::{
    AggregateParams, EvidenceParams, EvidenceTier, ExclusionParams, FamilyParams, GoalParams,
    PreviewParams, RatingParams,
};
pub use play::{Completeness, PlayCounts, ScoreSystem};
pub use rating::{
    PlayerRating, RatedPlay, aggregate_rating, aggregate_with, player_rating, rate_pbs,
    top2_per_family,
};
