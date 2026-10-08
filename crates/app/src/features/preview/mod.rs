//! Uncalibrated skill preview (ADR 0024): the `ComputePlaySsr` job caches every ledger play's goal
//! and MinaCalc SSRs, and `PreviewService` rates the selected scope from that cache. Deleted, not
//! migrated, once `skill_overview` ships.

pub mod dto;
mod job;
mod params;
mod recs;
mod service;
#[cfg(test)]
mod testkit;

pub use job::ComputePlaySsrJob;
pub use params::PreviewServiceParams;
pub use service::PreviewService;
