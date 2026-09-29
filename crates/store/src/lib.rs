//! The only crate with SQL (D6): user.db, cache.db, the blob vault and their repositories
//! (specs 003 and 004). Callers get store and core types, never rusqlite rows.

pub mod db;
pub mod error;
pub mod lock;

pub use db::{Conn, DbHandle, DbKind, Tx};
pub use error::StoreError;
pub use lock::InstanceLock;
