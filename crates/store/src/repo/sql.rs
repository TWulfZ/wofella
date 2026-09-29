//! Row decoding shared by the repositories. Stored values that fail to decode are reported as
//! conversion errors instead of being guessed.

use rusqlite::Row;
use rusqlite::types::Type;
use wolluf_core::UnixUs;

use crate::error::StoreError;
use crate::time::parse_rfc3339_ms;

/// Persisted enum with stable string values (architecture §11): never renumbered or renamed.
macro_rules! str_enum {
    ($(#[$meta:meta])* pub enum $name:ident { $($(#[$vmeta:meta])* $variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
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
pub(crate) use str_enum;

fn conversion(idx: usize, ty: Type, msg: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(idx, ty, Box::new(StoreError::InvalidData(msg)))
}

pub(crate) fn fixed<const N: usize>(row: &Row<'_>, idx: usize) -> rusqlite::Result<[u8; N]> {
    let bytes: Vec<u8> = row.get(idx)?;
    <[u8; N]>::try_from(bytes.as_slice()).map_err(|_| {
        conversion(
            idx,
            Type::Blob,
            format!("expected {N} bytes, got {}", bytes.len()),
        )
    })
}

pub(crate) fn opt_fixed<const N: usize>(
    row: &Row<'_>,
    idx: usize,
) -> rusqlite::Result<Option<[u8; N]>> {
    let bytes: Option<Vec<u8>> = row.get(idx)?;
    bytes
        .map(|b| {
            <[u8; N]>::try_from(b.as_slice()).map_err(|_| {
                conversion(
                    idx,
                    Type::Blob,
                    format!("expected {N} bytes, got {}", b.len()),
                )
            })
        })
        .transpose()
}

pub(crate) fn parsed<T, E: std::fmt::Display>(
    row: &Row<'_>,
    idx: usize,
    parse: impl FnOnce(&str) -> Result<T, E>,
) -> rusqlite::Result<T> {
    let text: String = row.get(idx)?;
    parse(&text).map_err(|e| conversion(idx, Type::Text, format!("{text:?}: {e}")))
}

pub(crate) fn opt_parsed<T, E: std::fmt::Display>(
    row: &Row<'_>,
    idx: usize,
    parse: impl FnOnce(&str) -> Result<T, E>,
) -> rusqlite::Result<Option<T>> {
    let text: Option<String> = row.get(idx)?;
    text.map(|t| parse(&t).map_err(|e| conversion(idx, Type::Text, format!("{t:?}: {e}"))))
        .transpose()
}

pub(crate) fn enum_col<T>(
    row: &Row<'_>,
    idx: usize,
    parse: impl FnOnce(&str) -> Option<T>,
) -> rusqlite::Result<T> {
    parsed(row, idx, |s| parse(s).ok_or("unknown value"))
}

pub(crate) fn time(row: &Row<'_>, idx: usize) -> rusqlite::Result<UnixUs> {
    parsed(row, idx, parse_rfc3339_ms)
}

pub(crate) fn opt_time(row: &Row<'_>, idx: usize) -> rusqlite::Result<Option<UnixUs>> {
    opt_parsed(row, idx, parse_rfc3339_ms)
}

pub(crate) fn to_i64(value: u64, what: &str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::InvalidData(format!("{what} {value} exceeds i64")))
}

/// Integer column narrowed to the Rust type the domain uses (`u16` counts, `u64` sizes).
pub(crate) fn int<T: TryFrom<i64>>(row: &Row<'_>, idx: usize) -> rusqlite::Result<T> {
    let value: i64 = row.get(idx)?;
    T::try_from(value).map_err(|_| conversion(idx, Type::Integer, format!("{value} out of range")))
}

pub(crate) fn json_value<T: serde::de::DeserializeOwned>(
    row: &Row<'_>,
    idx: usize,
) -> rusqlite::Result<T> {
    let text: String = row.get(idx)?;
    serde_json::from_str(&text).map_err(|e| conversion(idx, Type::Text, format!("{text:?}: {e}")))
}
