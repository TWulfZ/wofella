//! Plays: the `SyncPlays` job that fills the ledger from an osu! install (spec 003).

pub mod osg;
mod record;
mod service;
mod sync;
#[cfg(test)]
pub(crate) mod testkit;

pub use service::PlaysService;
pub use sync::{CATALOG_STAGE, CATALOG_VERSION, SyncPlaysJob};
