//! User preferences kept in user.db `settings`, read by the features they shape.

pub mod dto;
mod service;

pub use service::SettingsService;
pub(crate) use service::session_notify;
