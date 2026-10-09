//! Rate copies (ADR 0025): `plan` previews a rate-edited copy of a chart, `confirm` mints the
//! `ExportPermit` for that preview and starts the `RateCopy` job, which writes the `.osu` and the
//! time-stretched Ogg audio into the chart's set folder through `app::export` (D9).

mod assess;
pub mod dto;
mod job;
mod params;
mod service;
mod targets;
#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;

pub use job::RateCopyJob;
pub use params::RateCopiesParams;
pub use service::RateCopiesService;
pub(crate) use service::RateCopyPreview;
