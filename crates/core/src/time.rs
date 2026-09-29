//! Time and rate types (architecture §5.1, §5.3). Chart time and wall-clock time are distinct
//! types so they cannot be mixed.

use std::fmt;
use std::ops::{Add, Sub};

use crate::error::CoreError;

const US_PER_MS: i64 = 1_000;
const DOTNET_TICKS_PER_US: i64 = 10;

/// .NET ticks (100 ns) from 0001-01-01 to the Unix epoch.
pub const DOTNET_TO_UNIX_TICKS: i64 = 621_355_968_000_000_000;

/// .NET ticks from 0001-01-01 to 1601-01-01, the FILETIME epoch. The `Data/r` suffix equals
/// scores.db ticks minus this offset (research 03 l.168, 4362/4362 verified).
pub const DOTNET_TO_FILETIME_TICKS: i64 = 504_911_232_000_000_000;

/// Chart (map) time in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TimeUs(pub i64);

impl TimeUs {
    pub const ZERO: Self = Self(0);

    /// Takes `i32` because osu! stores integer ms times as 32-bit, which also makes the
    /// conversion lossless and overflow-free.
    pub const fn from_ms(ms: i32) -> Self {
        Self(ms as i64 * US_PER_MS)
    }

    /// Floors (not truncates), so negative lead-in times land in the right millisecond.
    pub const fn as_ms_floor(self) -> i64 {
        self.0.div_euclid(US_PER_MS)
    }

    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }
}

impl Add for TimeUs {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for TimeUs {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

/// Wall-clock instant in microseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixUs(pub i64);

/// .NET `DateTime` ticks as stored by scores.db and the replay header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DotNetTicks(pub i64);

impl DotNetTicks {
    pub const fn to_unix_us(self) -> UnixUs {
        // Dividing before subtracting cannot overflow; exact because the offset is a multiple
        // of the divisor.
        UnixUs(self.0.div_euclid(DOTNET_TICKS_PER_US) - DOTNET_TO_UNIX_TICKS / DOTNET_TICKS_PER_US)
    }

    pub const fn to_filetime(self) -> Option<FileTime> {
        match self.0.checked_sub(DOTNET_TO_FILETIME_TICKS) {
            Some(v) => FileTime::new(v),
            None => None,
        }
    }
}

/// Windows FILETIME (100 ns since 1601-01-01), never negative. Its decimal text is the
/// `Data/r` suffix and the `play.filetime` column, and `PlayId` hashes it (ADR 0006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileTime(i64);

impl FileTime {
    pub const fn new(value: i64) -> Option<Self> {
        if value < 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    pub const fn to_dotnet_ticks(self) -> Option<DotNetTicks> {
        match self.0.checked_add(DOTNET_TO_FILETIME_TICKS) {
            Some(v) => Some(DotNetTicks(v)),
            None => None,
        }
    }

    /// Canonical decimal only (no sign, no leading zeros), so `parse_decimal(s).to_string() == s`
    /// and a file name maps to exactly one `play.filetime` text.
    pub fn parse_decimal(s: &str) -> Result<Self, CoreError> {
        let invalid = || CoreError::InvalidFileTime(s.to_owned());
        let bytes = s.as_bytes();
        let canonical = !bytes.is_empty()
            && bytes.iter().all(u8::is_ascii_digit)
            && (bytes.len() == 1 || bytes[0] != b'0');
        if !canonical {
            return Err(invalid());
        }
        s.parse::<i64>()
            .ok()
            .and_then(Self::new)
            .ok_or_else(invalid)
    }
}

impl fmt::Display for FileTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Playback rate in thousandths (1000 = 1.0x); never zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RateMilli(u32);

impl RateMilli {
    pub const ONE: Self = Self(1_000);

    pub const fn new(milli: u32) -> Result<Self, CoreError> {
        if milli == 0 {
            Err(CoreError::ZeroRate)
        } else {
            Ok(Self(milli))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pilot replay: scores.db ticks and the matching `Data/r` file-name suffix (research 03 l.168).
    const PILOT_TICKS: i64 = 639_190_703_004_225_018;
    const PILOT_FILETIME_TEXT: &str = "134279471004225018";

    #[test]
    fn ms_floor_rounds_down_for_negatives() {
        assert_eq!(TimeUs(-1).as_ms_floor(), -1);
        assert_eq!(TimeUs(-1000).as_ms_floor(), -1);
        assert_eq!(TimeUs(-1001).as_ms_floor(), -2);
        assert_eq!(TimeUs(999).as_ms_floor(), 0);
        assert_eq!(TimeUs::from_ms(-3), TimeUs(-3000));
        assert_eq!(TimeUs::from_ms(-3).as_ms_floor(), -3);
    }

    #[test]
    fn time_arithmetic() {
        assert_eq!(TimeUs(5) + TimeUs(7), TimeUs(12));
        assert_eq!(TimeUs(5) - TimeUs(7), TimeUs(-2));
        assert_eq!(TimeUs(i64::MAX).checked_add(TimeUs(1)), None);
        assert_eq!(TimeUs(i64::MIN).checked_sub(TimeUs(1)), None);
        assert_eq!(TimeUs(1).checked_sub(TimeUs(2)), Some(TimeUs(-1)));
    }

    #[test]
    fn dotnet_ticks_unix_epoch_is_zero() {
        assert_eq!(DotNetTicks(DOTNET_TO_UNIX_TICKS).to_unix_us(), UnixUs(0));
        assert_eq!(
            DotNetTicks(DOTNET_TO_UNIX_TICKS + 10).to_unix_us(),
            UnixUs(1)
        );
        // Floor, not truncation: one tick before the epoch is still inside the previous µs.
        assert_eq!(
            DotNetTicks(DOTNET_TO_UNIX_TICKS - 1).to_unix_us(),
            UnixUs(-1)
        );
        assert!(DotNetTicks(i64::MIN).to_unix_us().0 < 0);
    }

    #[test]
    fn filetime_epoch_is_zero() {
        let ft = DotNetTicks(DOTNET_TO_FILETIME_TICKS).to_filetime().unwrap();
        assert_eq!(ft, FileTime::new(0).unwrap());
        assert_eq!(ft.get(), 0);
    }

    #[test]
    fn filetime_before_1601_is_none() {
        assert_eq!(
            DotNetTicks(DOTNET_TO_FILETIME_TICKS - 1).to_filetime(),
            None
        );
        assert_eq!(DotNetTicks(0).to_filetime(), None);
        assert_eq!(DotNetTicks(i64::MIN).to_filetime(), None);
        assert_eq!(FileTime::new(-1), None);
    }

    #[test]
    fn filetime_roundtrip() {
        let ft = DotNetTicks(PILOT_TICKS).to_filetime().unwrap();
        assert_eq!(ft.to_dotnet_ticks(), Some(DotNetTicks(PILOT_TICKS)));
        let max = FileTime::new(i64::MAX).unwrap();
        assert_eq!(max.to_dotnet_ticks(), None);
    }

    #[test]
    fn filetime_decimal_matches_data_r() {
        let ft = DotNetTicks(PILOT_TICKS).to_filetime().unwrap();
        assert_eq!(ft.to_string(), PILOT_FILETIME_TEXT);
        assert_eq!(FileTime::parse_decimal(PILOT_FILETIME_TEXT).unwrap(), ft);
        assert_eq!(FileTime::parse_decimal("0").unwrap().get(), 0);
        for bad in ["", "-1", "+1", "01", "12a", " 1", "9223372036854775808"] {
            assert!(
                FileTime::parse_decimal(bad).is_err(),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn rate_zero_rejected() {
        assert!(RateMilli::new(0).is_err());
        assert_eq!(RateMilli::new(1500).unwrap().get(), 1500);
        assert_eq!(RateMilli::ONE.get(), 1000);
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn filetime_ticks_bijective(ticks in DOTNET_TO_FILETIME_TICKS..=i64::MAX) {
            let ft = DotNetTicks(ticks).to_filetime();
            prop_assert!(ft.is_some());
            let ft = ft.unwrap();
            prop_assert_eq!(ft.to_dotnet_ticks(), Some(DotNetTicks(ticks)));
            prop_assert_eq!(FileTime::parse_decimal(&ft.to_string()), Ok(ft));
        }

        #[test]
        fn filetime_to_ticks_inverts(raw in 0..=(i64::MAX - DOTNET_TO_FILETIME_TICKS)) {
            let ft = FileTime::new(raw).unwrap();
            let ticks = ft.to_dotnet_ticks();
            prop_assert!(ticks.is_some());
            prop_assert_eq!(ticks.unwrap().to_filetime(), Some(ft));
        }
    }
}
