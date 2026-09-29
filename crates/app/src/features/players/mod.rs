//! Players and identity: who the user is among the names in scores.db (architecture §5.6,
//! spec 004, ADR 0005).

pub mod names;
pub mod params;
pub mod selection;
pub mod stats;

#[cfg(test)]
pub(crate) mod testkit;

pub use params::IdentityParams;
