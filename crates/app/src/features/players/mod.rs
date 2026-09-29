//! Players and identity: who the user is among the names in scores.db (architecture §5.6,
//! spec 004, ADR 0005).

/// Closed enum persisted and sent as fixed strings; discriminants never reach disk or the wire
/// (docs/conventions.md, stable ids).
macro_rules! stable_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

pub mod identity;
pub mod names;
pub mod params;
pub mod scope;
pub mod selection;
mod service;
pub mod stats;

#[cfg(test)]
pub(crate) mod testkit;

pub use params::IdentityParams;
pub use service::{PlayersService, RefreshIdentityJob};
