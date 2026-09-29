//! Keymode and column sets (architecture §5.1). Column 0 is the leftmost, matching osu!'s
//! `floor(x * K / 512)` column mapping.

use crate::error::CoreError;

/// `ColMask` is a `u16`, so no keymode can have more columns than it has bits.
pub const MAX_COLUMNS: u8 = u16::BITS as u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Keymode(u8);

impl Keymode {
    pub const K4: Self = Self(4);
    pub const K7: Self = Self(7);

    pub const fn new(columns: u8) -> Result<Self, CoreError> {
        if columns == 0 || columns > MAX_COLUMNS {
            Err(CoreError::InvalidKeymode(columns))
        } else {
            Ok(Self(columns))
        }
    }

    pub const fn columns(self) -> u8 {
        self.0
    }
}

/// Bit `i` is column `i`. Constructors take the keymode so a mask never holds a column the
/// keymode does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ColMask(u16);

impl ColMask {
    pub const EMPTY: Self = Self(0);

    pub const fn full(keymode: Keymode) -> Self {
        Self(u16::MAX >> (MAX_COLUMNS - keymode.0))
    }

    pub const fn single(keymode: Keymode, col: u8) -> Result<Self, CoreError> {
        if col < keymode.0 {
            Ok(Self(1 << col))
        } else {
            Err(CoreError::ColumnOutOfRange {
                col,
                keymode: keymode.0,
            })
        }
    }

    pub fn from_cols(
        keymode: Keymode,
        cols: impl IntoIterator<Item = u8>,
    ) -> Result<Self, CoreError> {
        cols.into_iter().try_fold(Self::EMPTY, |acc, col| {
            Ok(Self(acc.0 | Self::single(keymode, col)?.0))
        })
    }

    pub const fn from_bits(keymode: Keymode, bits: u16) -> Result<Self, CoreError> {
        let outside = bits & !Self::full(keymode).0;
        if outside == 0 {
            Ok(Self(bits))
        } else {
            Err(CoreError::ColumnOutOfRange {
                col: outside.trailing_zeros() as u8,
                keymode: keymode.0,
            })
        }
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, col: u8) -> bool {
        match 1u16.checked_shl(col as u32) {
            Some(bit) => self.0 & bit != 0,
            None => false,
        }
    }

    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn is_subset_of(self, other: Self) -> bool {
        self.0 & !other.0 == 0
    }

    pub fn iter(self) -> ColIter {
        ColIter(self.0)
    }

    /// Mirror mod: column `i` becomes `K - 1 - i` (§5.1).
    pub fn mirror(self, keymode: Keymode) -> Self {
        let last = keymode.0 - 1;
        self.iter()
            .filter(|&col| col <= last)
            .fold(Self::EMPTY, |acc, col| Self(acc.0 | 1 << (last - col)))
    }
}

/// Ascending column indices of a `ColMask`.
#[derive(Debug, Clone)]
pub struct ColIter(u16);

impl Iterator for ColIter {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.0 == 0 {
            return None;
        }
        let col = self.0.trailing_zeros() as u8;
        self.0 &= self.0 - 1;
        Some(col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_0_and_17() {
        assert!(Keymode::new(0).is_err());
        assert!(Keymode::new(17).is_err());
        assert_eq!(Keymode::new(1).unwrap().columns(), 1);
        assert_eq!(Keymode::new(16).unwrap().columns(), 16);
    }

    #[test]
    fn k7_has_7_columns() {
        assert_eq!(Keymode::K7.columns(), 7);
        assert_eq!(Keymode::K4.columns(), 4);
        assert_eq!(ColMask::full(Keymode::K7).len(), 7);
        assert_eq!(ColMask::full(Keymode::K7).bits(), 0b111_1111);
        assert_eq!(ColMask::full(Keymode::new(16).unwrap()).bits(), u16::MAX);
    }

    #[test]
    fn column_ops_reject_out_of_range() {
        let k7 = Keymode::K7;
        assert!(ColMask::single(k7, 7).is_err());
        assert!(ColMask::from_cols(k7, [0, 7]).is_err());
        assert!(ColMask::from_bits(k7, 1 << 7).is_err());
        assert!(!ColMask::full(k7).contains(7));
        assert!(!ColMask::full(Keymode::new(16).unwrap()).contains(16));
    }

    #[test]
    fn mirror_maps_leftmost_to_rightmost() {
        let k7 = Keymode::K7;
        let m = ColMask::from_cols(k7, [0, 1]).unwrap();
        assert_eq!(m.mirror(k7), ColMask::from_cols(k7, [5, 6]).unwrap());
        assert_eq!(
            ColMask::single(k7, 3).unwrap().mirror(k7),
            ColMask::single(k7, 3).unwrap()
        );
        assert!(ColMask::EMPTY.is_empty());
        assert!(ColMask::EMPTY.is_subset_of(m));
        assert!(m.is_subset_of(ColMask::full(k7)));
        assert!(!ColMask::full(k7).is_subset_of(m));
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;

    use super::*;

    fn keymode_and_mask() -> impl Strategy<Value = (Keymode, ColMask)> {
        (1..=MAX_COLUMNS).prop_flat_map(|k| {
            let keymode = Keymode::new(k).unwrap();
            (Just(keymode), any::<u16>()).prop_map(|(keymode, raw)| {
                (
                    keymode,
                    ColMask::from_bits(keymode, raw & ColMask::full(keymode).bits()).unwrap(),
                )
            })
        })
    }

    proptest! {
        #[test]
        fn mirror_is_involution((k, m) in keymode_and_mask()) {
            prop_assert_eq!(m.mirror(k).mirror(k), m);
            prop_assert_eq!(m.mirror(k).len(), m.len());
        }

        #[test]
        fn ops_stay_within_keymode((k, m) in keymode_and_mask(), col in 0u8..=u8::MAX) {
            let full = ColMask::full(k);
            prop_assert!(m.is_subset_of(full));
            prop_assert!(m.mirror(k).is_subset_of(full));
            prop_assert_eq!(ColMask::single(k, col).is_ok(), col < k.columns());
            if let Ok(single) = ColMask::single(k, col) {
                prop_assert!(single.is_subset_of(full));
                prop_assert_eq!(single.len(), 1);
            }
            let rebuilt = ColMask::from_cols(k, m.iter());
            prop_assert_eq!(rebuilt, Ok(m));
        }

        #[test]
        fn iter_is_ascending_and_matches_len((_k, m) in keymode_and_mask()) {
            let cols: Vec<u8> = m.iter().collect();
            prop_assert_eq!(cols.len(), m.len() as usize);
            prop_assert!(cols.windows(2).all(|w| w[0] < w[1]));
            prop_assert!(cols.iter().all(|&c| m.contains(c)));
        }
    }
}
