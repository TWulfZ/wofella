//! Column → (hand, finger) layouts (architecture §3: presets are data with stable string ids).
//! Pattern rules read hands and fingers from here, never from column numbers.

use wolluf_core::Keymode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hand {
    Left,
    Right,
    /// A column either thumb may take, like the 7K middle key in the both-thumbs style; rules
    /// that split by hand must treat it as belonging to neither side.
    Both,
}

impl Hand {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Finger {
    Pinky,
    Ring,
    Middle,
    Index,
    Thumb,
}

impl Finger {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pinky => "pinky",
            Self::Ring => "ring",
            Self::Middle => "middle",
            Self::Index => "index",
            Self::Thumb => "thumb",
        }
    }
}

/// The pilot's layout (`3|1+3`, right thumb on the middle key).
pub const DEFAULT_K7: &str = "k7.313_right_thumb";

const GENERIC_SUFFIX: &str = ".generic";

struct Preset {
    id: &'static str,
    columns: &'static [(Hand, Finger)],
}

use Finger::{Index, Middle, Pinky, Ring, Thumb};
use Hand::{Both, Left, Right};

/// Ids are persisted: append new presets, never rename or reuse one.
const PRESETS: &[Preset] = &[
    Preset {
        id: DEFAULT_K7,
        columns: &[
            (Left, Ring),
            (Left, Middle),
            (Left, Index),
            (Right, Thumb),
            (Right, Index),
            (Right, Middle),
            (Right, Ring),
        ],
    },
    Preset {
        id: "k7.313_left_thumb",
        columns: &[
            (Left, Ring),
            (Left, Middle),
            (Left, Index),
            (Left, Thumb),
            (Right, Index),
            (Right, Middle),
            (Right, Ring),
        ],
    },
    Preset {
        id: "k7.43",
        columns: &[
            (Left, Pinky),
            (Left, Ring),
            (Left, Middle),
            (Left, Index),
            (Right, Index),
            (Right, Middle),
            (Right, Ring),
        ],
    },
    Preset {
        id: "k7.34",
        columns: &[
            (Left, Ring),
            (Left, Middle),
            (Left, Index),
            (Right, Index),
            (Right, Middle),
            (Right, Ring),
            (Right, Pinky),
        ],
    },
    Preset {
        id: "k7.both_thumbs",
        columns: &[
            (Left, Ring),
            (Left, Middle),
            (Left, Index),
            (Both, Thumb),
            (Right, Index),
            (Right, Middle),
            (Right, Ring),
        ],
    },
];

pub fn preset_ids() -> impl Iterator<Item = &'static str> {
    PRESETS.iter().map(|p| p.id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    id: String,
    keymode: Keymode,
    columns: Vec<(Hand, Finger)>,
    mirrored: bool,
}

impl Layout {
    /// A preset id, or `k<N>.generic` for N in 1..=16.
    pub fn by_id(id: &str) -> Option<Self> {
        if let Some(preset) = PRESETS.iter().find(|p| p.id == id) {
            let keys = u8::try_from(preset.columns.len()).ok()?;
            return Some(Self {
                id: preset.id.to_owned(),
                keymode: Keymode::new(keys).ok()?,
                columns: preset.columns.to_vec(),
                mirrored: false,
            });
        }
        let digits = id.strip_prefix('k')?.strip_suffix(GENERIC_SUFFIX)?;
        // Canonical digits only, so every generic layout has exactly one id.
        if digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let keymode = Keymode::new(digits.parse().ok()?).ok()?;
        Some(Self::generic(keymode))
    }

    /// The first preset of the keymode, else the generic split.
    pub fn default_for(keymode: Keymode) -> Self {
        PRESETS
            .iter()
            .filter(|p| p.columns.len() == usize::from(keymode.columns()))
            .find_map(|p| Self::by_id(p.id))
            .unwrap_or_else(|| Self::generic(keymode))
    }

    /// Left hand takes the floor half, right hand the ceil half. Each hand fills from the
    /// centre outwards: index, middle, ring, pinky; from five columns a hand the thumb takes the
    /// innermost column, and any columns beyond five fall to the pinky.
    pub fn generic(keymode: Keymode) -> Self {
        let keys = keymode.columns();
        let left = hand_columns(keys / 2);
        let right = hand_columns(keys - keys / 2);
        let columns = left
            .iter()
            .rev()
            .map(|&f| (Left, f))
            .chain(right.iter().map(|&f| (Right, f)))
            .collect();
        Self {
            id: format!("k{keys}{GENERIC_SUFFIX}"),
            keymode,
            columns,
            mirrored: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn keymode(&self) -> Keymode {
        self.keymode
    }

    pub fn columns(&self) -> &[(Hand, Finger)] {
        &self.columns
    }

    pub fn column(&self, col: u8) -> Option<(Hand, Finger)> {
        self.columns.get(usize::from(col)).copied()
    }

    pub fn is_mirrored(&self) -> bool {
        self.mirrored
    }

    /// Under the Mirror mod chart column `i` is played at physical column `K-1-i`, so it gets
    /// that column's finger; the hands themselves do not move (architecture §5.1).
    pub fn mirror(&self) -> Self {
        Self {
            id: self.id.clone(),
            keymode: self.keymode,
            columns: self.columns.iter().rev().copied().collect(),
            mirrored: !self.mirrored,
        }
    }
}

/// Fingers of one hand, innermost column first.
fn hand_columns(n: u8) -> Vec<Finger> {
    let base: &[Finger] = if n <= 4 {
        &[Index, Middle, Ring, Pinky]
    } else {
        &[Thumb, Index, Middle, Ring, Pinky]
    };
    (0..usize::from(n))
        .map(|i| base.get(i).copied().unwrap_or(Pinky))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use wolluf_core::Keymode;
    use wolluf_core::id::is_valid_stable_id;

    use super::Finger::{Index, Middle, Pinky, Ring, Thumb};
    use super::Hand::{Both, Left, Right};
    use super::*;

    fn columns_of(id: &str) -> Vec<(Hand, Finger)> {
        let layout = Layout::by_id(id).unwrap();
        assert_eq!(layout.id(), id);
        assert_eq!(layout.keymode(), Keymode::K7);
        assert!(!layout.is_mirrored());
        layout.columns().to_vec()
    }

    #[test]
    fn k7_313_right_thumb() {
        assert_eq!(
            columns_of("k7.313_right_thumb"),
            [
                (Left, Ring),
                (Left, Middle),
                (Left, Index),
                (Right, Thumb),
                (Right, Index),
                (Right, Middle),
                (Right, Ring)
            ]
        );
    }

    #[test]
    fn k7_313_left_thumb() {
        assert_eq!(
            columns_of("k7.313_left_thumb"),
            [
                (Left, Ring),
                (Left, Middle),
                (Left, Index),
                (Left, Thumb),
                (Right, Index),
                (Right, Middle),
                (Right, Ring)
            ]
        );
    }

    #[test]
    fn k7_43() {
        assert_eq!(
            columns_of("k7.43"),
            [
                (Left, Pinky),
                (Left, Ring),
                (Left, Middle),
                (Left, Index),
                (Right, Index),
                (Right, Middle),
                (Right, Ring)
            ]
        );
    }

    #[test]
    fn k7_34() {
        assert_eq!(
            columns_of("k7.34"),
            [
                (Left, Ring),
                (Left, Middle),
                (Left, Index),
                (Right, Index),
                (Right, Middle),
                (Right, Ring),
                (Right, Pinky)
            ]
        );
    }

    #[test]
    fn k7_both_thumbs() {
        assert_eq!(
            columns_of("k7.both_thumbs"),
            [
                (Left, Ring),
                (Left, Middle),
                (Left, Index),
                (Both, Thumb),
                (Right, Index),
                (Right, Middle),
                (Right, Ring)
            ]
        );
    }

    #[test]
    fn default_k7_is_the_pilot_right_thumb_layout() {
        assert_eq!(Layout::default_for(Keymode::K7).id(), "k7.313_right_thumb");
        assert_eq!(DEFAULT_K7, "k7.313_right_thumb");
    }

    #[test]
    fn keymodes_without_presets_default_to_generic() {
        assert_eq!(
            Layout::default_for(Keymode::K4),
            Layout::generic(Keymode::K4)
        );
    }

    #[test]
    fn generic_splits_floor_left_ceil_right_from_the_index_outwards() {
        let k = |n| Layout::generic(Keymode::new(n).unwrap());
        assert_eq!(k(1).columns(), [(Right, Index)]);
        assert_eq!(
            k(4).columns(),
            [
                (Left, Middle),
                (Left, Index),
                (Right, Index),
                (Right, Middle)
            ]
        );
        assert_eq!(k(4).id(), "k4.generic");
        assert_eq!(
            k(7).columns(),
            [
                (Left, Ring),
                (Left, Middle),
                (Left, Index),
                (Right, Index),
                (Right, Middle),
                (Right, Ring),
                (Right, Pinky)
            ]
        );
        assert_eq!(
            k(10).columns(),
            [
                (Left, Pinky),
                (Left, Ring),
                (Left, Middle),
                (Left, Index),
                (Left, Thumb),
                (Right, Thumb),
                (Right, Index),
                (Right, Middle),
                (Right, Ring),
                (Right, Pinky),
            ]
        );
        let k16 = k(16);
        assert_eq!(k16.columns().len(), 16);
        assert_eq!(&k16.columns()[..4], [(Left, Pinky); 4]);
        assert_eq!(k16.columns()[7], (Left, Thumb));
        assert_eq!(k16.columns()[8], (Right, Thumb));
        for n in 1..=16u8 {
            let layout = k(n);
            let left = layout.columns().iter().filter(|(h, _)| *h == Left).count();
            assert_eq!(left, usize::from(n / 2), "K={n}");
            assert_eq!(layout.columns().len(), usize::from(n));
            assert_eq!(Layout::by_id(layout.id()), Some(layout));
        }
    }

    #[test]
    fn lookup_rejects_unknown_ids() {
        for id in [
            "",
            "k7",
            "k7.unknown",
            "k0.generic",
            "k17.generic",
            "k07.generic",
            "K4.generic",
        ] {
            assert_eq!(Layout::by_id(id), None, "{id}");
        }
    }

    #[test]
    fn mirror_reverses_columns_and_is_an_involution() {
        let layout = Layout::default_for(Keymode::K7);
        let mirrored = layout.mirror();
        assert!(mirrored.is_mirrored());
        assert_eq!(mirrored.id(), layout.id());
        assert_eq!(mirrored.column(0), Some((Right, Ring)));
        assert_eq!(mirrored.column(2), Some((Right, Index)));
        assert_eq!(mirrored.column(3), Some((Right, Thumb)));
        assert_eq!(mirrored.column(6), Some((Left, Ring)));
        assert_eq!(mirrored.column(7), None);
        assert_eq!(mirrored.mirror(), layout);
    }

    #[test]
    fn preset_ids_are_unique_stable_ids_matching_their_keymode() {
        let ids: Vec<&str> = preset_ids().collect();
        assert_eq!(ids.len(), 5);
        assert_eq!(
            ids.iter().copied().collect::<BTreeSet<_>>().len(),
            ids.len()
        );
        for id in ids {
            assert!(is_valid_stable_id(id), "{id}");
            let layout = Layout::by_id(id).unwrap();
            assert!(
                id.starts_with(&format!("k{}.", layout.keymode().columns())),
                "{id}"
            );
        }
    }

    #[test]
    fn hand_and_finger_stable_strings() {
        let hands: Vec<&str> = [Left, Right, Both].iter().map(|h| h.as_str()).collect();
        assert_eq!(hands, ["left", "right", "both"]);
        let fingers: Vec<&str> = [Pinky, Ring, Middle, Index, Thumb]
            .iter()
            .map(|f| f.as_str())
            .collect();
        assert_eq!(fingers, ["pinky", "ring", "middle", "index", "thumb"]);
    }
}
