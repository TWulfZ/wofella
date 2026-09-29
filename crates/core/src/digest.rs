//! Content digests and ledger ids (architecture §5.3). Hash encodings are frozen by ADR 0006.

use crate::error::CoreError;
use crate::id::stable_str_enum;
use crate::time::FileTime;

pub(crate) const PLAY_ID_TAG: &[u8] = b"wolluf.play.v1";

/// Lowercase only: osu! writes md5s in lowercase, and one spelling per value keeps text
/// comparisons in SQL and file names exact.
pub(crate) fn parse_hex<const N: usize>(
    s: &str,
    type_name: &'static str,
) -> Result<[u8; N], CoreError> {
    let invalid = || CoreError::InvalidHex {
        type_name,
        input: s.to_owned(),
    };
    let digits = s.as_bytes();
    if digits.len() != N * 2 {
        return Err(invalid());
    }
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; N];
    let (pairs, _) = digits.as_chunks::<2>();
    for (byte, &[hi, lo]) in out.iter_mut().zip(pairs) {
        let (hi, lo) = nibble(hi).zip(nibble(lo)).ok_or_else(invalid)?;
        *byte = hi << 4 | lo;
    }
    Ok(out)
}

pub(crate) fn write_hex(bytes: &[u8], f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))
}

/// Fixed-size byte id printed, parsed and serialized as lowercase hex.
macro_rules! hex_bytes {
    ($(#[$meta:meta])* $name:ident, $len:expr) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(pub [u8; $len]);

        impl std::str::FromStr for $name {
            type Err = $crate::error::CoreError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                $crate::digest::parse_hex(s, stringify!($name)).map(Self)
            }
        }

        impl TryFrom<String> for $name {
            type Error = $crate::error::CoreError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                s.parse()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                $crate::digest::write_hex(&self.0, f)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({self})", stringify!($name))
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }
    };
}

/// ADR 0006 encoding: a raw domain tag, then every byte string as `u32` LE length + bytes and
/// every integer as fixed-width LE. The length prefix makes the encoding injective.
pub(crate) struct FieldHasher(blake3::Hasher);

impl FieldHasher {
    pub(crate) fn new(domain_tag: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain_tag);
        Self(hasher)
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) -> &mut Self {
        // Only inputs above 4 GiB could saturate, far beyond any osu! string or pack section,
        // and saturating keeps derivation total instead of fallible.
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        self.0.update(&len.to_le_bytes());
        self.0.update(bytes);
        self
    }

    pub(crate) fn i64(&mut self, value: i64) -> &mut Self {
        self.0.update(&value.to_le_bytes());
        self
    }

    pub(crate) fn finish(&self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

stable_str_enum! {
    /// Source game of a play or chart.
    pub enum Game, unknown = CoreError::UnknownGame {
        OsuStable => "osu_stable",
    }
}

hex_bytes!(
    /// Beatmap md5 as osu! stores it.
    ChartMd5,
    16
);
hex_bytes!(
    /// Content address of a vault blob.
    BlobSha256,
    32
);
hex_bytes!(
    /// Stable ledger id of a play, see [`PlayId::derive`].
    PlayId,
    32
);

impl PlayId {
    /// `blake3(game, chart_md5, raw_name, filetime)` (§5.3). Raw name bytes, not a normalized
    /// form, because the alias key is the bytes osu! stored and `""` is a valid alias.
    pub fn derive(game: Game, chart_md5: ChartMd5, raw_name: &[u8], filetime: FileTime) -> Self {
        let mut hasher = FieldHasher::new(PLAY_ID_TAG);
        hasher
            .bytes(game.as_str().as_bytes())
            .bytes(&chart_md5.0)
            .bytes(raw_name)
            .i64(filetime.get());
        Self(hasher.finish())
    }
}

/// user.db `alias` row id; local, so it never enters a hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AliasId(pub i64);

/// user.db `profile` row id; local, so it never enters a hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProfileId(pub i64);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::FileTime;

    const PILOT_MD5: &str = "e956977ccc1d74a50ae48b43a868cc20";
    // Computed once from the ADR 0006 encoding with an independent Python blake3 script, then
    // frozen: a change here means every stored play id changes.
    const PILOT_PLAY_ID: &str = "f3ad5bfc0d2ce9ddbf14a94b24f56ee30c7e9057e46d6019e6cfed0b3110c6b7";
    const PILOT_EMPTY_ALIAS_PLAY_ID: &str =
        "f356a414bd95101cd9af72395d5a4faeddcb33cbad426ab79ea47b3734522a63";

    fn pilot_filetime() -> FileTime {
        FileTime::new(134_350_010_443_098_880).unwrap()
    }

    fn pilot_md5() -> ChartMd5 {
        PILOT_MD5.parse().unwrap()
    }

    #[test]
    fn md5_hex_roundtrip() {
        let md5 = pilot_md5();
        assert_eq!(md5.0[0], 0xe9);
        assert_eq!(md5.to_string(), PILOT_MD5);
        let sha_hex = "00ff".repeat(16);
        let sha: BlobSha256 = sha_hex.parse().unwrap();
        assert_eq!(sha.to_string(), sha_hex);
        let json = serde_json::to_string(&md5).unwrap();
        assert_eq!(json, format!("\"{PILOT_MD5}\""));
        assert_eq!(serde_json::from_str::<ChartMd5>(&json).unwrap(), md5);
    }

    #[test]
    fn rejects_bad_hex() {
        let upper = PILOT_MD5.to_uppercase();
        let short = &PILOT_MD5[..31];
        let long = format!("{PILOT_MD5}0");
        let non_hex = format!("{}g", &PILOT_MD5[..31]);
        let multibyte = format!("{}é", &PILOT_MD5[..30]);
        for bad in [
            "",
            short,
            long.as_str(),
            non_hex.as_str(),
            upper.as_str(),
            multibyte.as_str(),
        ] {
            assert!(bad.parse::<ChartMd5>().is_err(), "{bad:?} must be rejected");
        }
        assert!(PILOT_MD5.parse::<BlobSha256>().is_err());
    }

    #[test]
    fn game_stable_strings() {
        assert_eq!(Game::OsuStable.as_str(), "osu_stable");
        for game in Game::ALL {
            assert_eq!(game.as_str().parse::<Game>().unwrap(), *game);
        }
        assert!("osu".parse::<Game>().is_err());
        assert_eq!(
            serde_json::to_string(&Game::OsuStable).unwrap(),
            "\"osu_stable\""
        );
    }

    #[test]
    fn play_id_golden_vector() {
        let id = PlayId::derive(Game::OsuStable, pilot_md5(), b"TWulfZ", pilot_filetime());
        assert_eq!(id.to_string(), PILOT_PLAY_ID);
        assert_eq!(id.to_string().parse::<PlayId>().unwrap(), id);
    }

    #[test]
    fn play_id_distinguishes_empty_alias() {
        let empty = PlayId::derive(Game::OsuStable, pilot_md5(), b"", pilot_filetime());
        let w = PlayId::derive(Game::OsuStable, pilot_md5(), b"W", pilot_filetime());
        assert_eq!(empty.to_string(), PILOT_EMPTY_ALIAS_PLAY_ID);
        assert_ne!(empty, w);
    }

    #[test]
    fn play_id_length_prefix_prevents_concat_collision() {
        let hash = |a: &[u8], b: &[u8]| {
            let mut h = FieldHasher::new(PLAY_ID_TAG);
            h.bytes(a).bytes(b);
            h.finish()
        };
        assert_ne!(hash(b"ab", b"c"), hash(b"a", b"bc"));
        assert_ne!(hash(b"", b"W"), hash(b"W", b""));
        let later = FileTime::new(pilot_filetime().get() + 1).unwrap();
        assert_ne!(
            PlayId::derive(Game::OsuStable, pilot_md5(), b"TWulfZ", pilot_filetime()),
            PlayId::derive(Game::OsuStable, pilot_md5(), b"TWulfZ", later)
        );
    }
}
