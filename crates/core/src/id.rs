//! Stable string ids (architecture §5.1, §11): persisted and never renumbered.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::error::CoreError;

pub const MAX_STABLE_ID_LEN: usize = 64;

/// `[a-z0-9_]+` segments joined by `.`, 1..=64 bytes. `const` so `from_static` can reject bad
/// literals at compile time.
pub const fn is_valid_stable_id(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_STABLE_ID_LEN {
        return false;
    }
    let mut segment_empty = true;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'.' => {
                if segment_empty {
                    return false;
                }
                segment_empty = true;
            }
            b'a'..=b'z' | b'0'..=b'9' | b'_' => segment_empty = false,
            _ => return false,
        }
        i += 1;
    }
    !segment_empty
}

macro_rules! stable_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(Cow<'static, str>);

        impl $name {
            pub fn parse(s: &str) -> Result<Self, CoreError> {
                if is_valid_stable_id(s) {
                    Ok(Self(Cow::Owned(s.to_owned())))
                } else {
                    Err(CoreError::InvalidStableId(s.to_owned()))
                }
            }

            /// Evaluated in a `const`, an invalid literal fails compilation instead of panicking.
            pub const fn from_static(s: &'static str) -> Self {
                assert!(is_valid_stable_id(s), "invalid stable id literal");
                Self(Cow::Borrowed(s))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = CoreError;

            fn try_from(s: String) -> Result<Self, CoreError> {
                if is_valid_stable_id(&s) {
                    Ok(Self(Cow::Owned(s)))
                } else {
                    Err(CoreError::InvalidStableId(s))
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }
    };
}

stable_id!(
    /// Skill axis, e.g. `7k.regular.speed`.
    AxisId
);
stable_id!(
    /// Pattern class, e.g. `regular.jack.minijack`.
    PatternId
);
stable_id!(
    /// Versioned derivation stage, e.g. `players.alias_stats`.
    StageId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dotted_lowercase() {
        for ok in [
            "7k.regular.speed",
            "regular.jack.minijack",
            "a",
            "ln_general",
            "players.alias_stats",
        ] {
            assert_eq!(AxisId::parse(ok).unwrap().as_str(), ok);
            assert_eq!(PatternId::parse(ok).unwrap().as_str(), ok);
            assert_eq!(StageId::parse(ok).unwrap().as_str(), ok);
        }
    }

    #[test]
    fn rejects_uppercase_empty_and_long() {
        let too_long = "a".repeat(MAX_STABLE_ID_LEN + 1);
        for bad in [
            "7K.Speed",
            "",
            ".a",
            "a.",
            "a..b",
            "a-b",
            "a b",
            "é",
            too_long.as_str(),
        ] {
            assert!(AxisId::parse(bad).is_err(), "{bad:?} must be rejected");
        }
        let max = "a".repeat(MAX_STABLE_ID_LEN);
        assert!(AxisId::parse(&max).is_ok());
    }

    #[test]
    fn from_static_in_const() {
        const SPEED: AxisId = AxisId::from_static("7k.regular.speed");
        const MINIJACK: PatternId = PatternId::from_static("regular.jack.minijack");
        const STATS: StageId = StageId::from_static("players.alias_stats");
        assert_eq!(SPEED, AxisId::parse("7k.regular.speed").unwrap());
        assert_eq!(MINIJACK.as_str(), "regular.jack.minijack");
        assert_eq!(STATS.to_string(), "players.alias_stats");
    }

    #[test]
    fn serde_is_plain_string() {
        let id = AxisId::parse("7k.regular.speed").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"7k.regular.speed\"");
        let back: AxisId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
        assert!(serde_json::from_str::<AxisId>("\"7K.Speed\"").is_err());
    }
}
