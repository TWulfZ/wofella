//! stable mod bits (`research/scripts/rejudge/osr_wiki.md:65-101`).

use super::params::ExclusionParams;
use super::play::ScoreSystem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayMods {
    pub ez: bool,
    pub hr: bool,
    pub dt: bool,
    pub ht: bool,
    pub nc: bool,
    pub random: bool,
    pub coop: bool,
    pub score_v2: bool,
    pub mirror: bool,
    pub key_mod: bool,
}

const EZ: i32 = 1 << 1;
const HR: i32 = 1 << 4;
const DT: i32 = 1 << 6;
const HT: i32 = 1 << 8;
const NC: i32 = 1 << 9;
/// Key4..Key8 (bits 15-19), Key9 (24), Key1 (26), Key3 (27), Key2 (28).
const KEY_MOD: i32 = 0b1_1111 << 15 | 1 << 24 | 0b111 << 26;
const RANDOM: i32 = 1 << 21;
const COOP: i32 = 1 << 25;
const SCORE_V2: i32 = 1 << 29;
const MIRROR: i32 = 1 << 30;

const RATE_DT_MILLI: u16 = 1500;
const RATE_HT_MILLI: u16 = 750;
const RATE_NM_MILLI: u16 = 1000;

impl PlayMods {
    pub fn from_bits(bits: i32) -> Self {
        let has = |m: i32| bits & m != 0;
        Self {
            ez: has(EZ),
            hr: has(HR),
            // stable always stores NC with DT; a lone NC bit still plays at 1.5×.
            dt: has(DT) || has(NC),
            ht: has(HT),
            nc: has(NC),
            random: has(RANDOM),
            coop: has(COOP),
            score_v2: has(SCORE_V2),
            mirror: has(MIRROR),
            key_mod: has(KEY_MOD),
        }
    }

    pub fn rate_milli(&self) -> u16 {
        if self.dt || self.nc {
            RATE_DT_MILLI
        } else if self.ht {
            RATE_HT_MILLI
        } else {
            RATE_NM_MILLI
        }
    }

    pub fn score_system(&self) -> ScoreSystem {
        if self.score_v2 {
            ScoreSystem::V2
        } else {
            ScoreSystem::V1
        }
    }

    pub fn unsupported(&self, params: &ExclusionParams) -> bool {
        (params.exclude_random && self.random)
            || (params.exclude_coop && self.coop)
            || (params.exclude_key_mod && self.key_mod)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EZ: i32 = 2;
    const HR: i32 = 16;
    const DT: i32 = 64;
    const HT: i32 = 256;
    const NC: i32 = 512;
    const KEY4: i32 = 32_768;
    const KEY7: i32 = 262_144;
    const KEY9: i32 = 16_777_216;
    const KEY1: i32 = 67_108_864;
    const KEY3: i32 = 134_217_728;
    const KEY2: i32 = 268_435_456;
    const RANDOM: i32 = 2_097_152;
    const COOP: i32 = 33_554_432;
    const SCORE_V2: i32 = 536_870_912;
    const MIRROR: i32 = 1_073_741_824;

    #[test]
    fn no_mod_is_all_false_at_rate_1000() {
        let m = PlayMods::from_bits(0);
        assert_eq!(m, PlayMods::default());
        assert_eq!(m.rate_milli(), 1000);
        assert_eq!(m.score_system(), ScoreSystem::V1);
    }

    #[test]
    fn each_bit_sets_its_flag() {
        type Flag = fn(&PlayMods) -> bool;
        let cases: [(i32, Flag); 9] = [
            (EZ, |m| m.ez),
            (HR, |m| m.hr),
            (DT, |m| m.dt),
            (HT, |m| m.ht),
            (RANDOM, |m| m.random),
            (COOP, |m| m.coop),
            (SCORE_V2, |m| m.score_v2),
            (MIRROR, |m| m.mirror),
            (KEY7, |m| m.key_mod),
        ];
        for (bit, flag) in cases {
            let m = PlayMods::from_bits(bit);
            assert!(flag(&m), "bit {bit}: {m:?}");
            let set = [
                m.ez, m.hr, m.dt, m.ht, m.nc, m.random, m.coop, m.score_v2, m.mirror, m.key_mod,
            ];
            assert_eq!(set.iter().filter(|b| **b).count(), 1, "bit {bit}: {m:?}");
        }
    }

    #[test]
    fn every_key_bit_is_a_key_mod() {
        for bit in [KEY1, KEY2, KEY3, KEY4, 65_536, 131_072, KEY7, 524_288, KEY9] {
            assert!(PlayMods::from_bits(bit).key_mod, "{bit}");
        }
        // NoFail, Hidden, SuddenDeath, FadeIn, Perfect and Flashlight do not touch the preview.
        for bit in [1, 8, 32, 1_048_576, 16_384, 1024] {
            assert_eq!(PlayMods::from_bits(bit), PlayMods::default(), "{bit}");
        }
    }

    #[test]
    fn nightcore_implies_double_time_even_alone() {
        let full = PlayMods::from_bits(NC | DT);
        assert!(full.nc && full.dt);
        let alone = PlayMods::from_bits(NC);
        assert!(alone.nc && alone.dt, "{alone:?}");
        assert_eq!(alone.rate_milli(), 1500);
    }

    #[test]
    fn rate_follows_dt_and_ht() {
        assert_eq!(PlayMods::from_bits(DT).rate_milli(), 1500);
        assert_eq!(PlayMods::from_bits(NC | DT | HR).rate_milli(), 1500);
        assert_eq!(PlayMods::from_bits(HT).rate_milli(), 750);
        assert_eq!(PlayMods::from_bits(HR | MIRROR).rate_milli(), 1000);
    }

    #[test]
    fn score_v2_is_its_own_score_system() {
        let v2 = PlayMods::from_bits(SCORE_V2 | NC | DT);
        assert_eq!(v2.score_system(), ScoreSystem::V2);
        assert_eq!(v2.rate_milli(), 1500);
    }

    #[test]
    fn random_coop_and_key_conversion_are_unsupported() {
        let params = ExclusionParams::default();
        for bit in [RANDOM, COOP, KEY4] {
            assert!(PlayMods::from_bits(bit).unsupported(&params), "{bit}");
        }
        for bit in [0, HR, EZ, DT | NC, HT, MIRROR, SCORE_V2] {
            assert!(!PlayMods::from_bits(bit).unsupported(&params), "{bit}");
        }
        let lenient = ExclusionParams {
            exclude_random: false,
            ..params
        };
        assert!(!PlayMods::from_bits(RANDOM).unsupported(&lenient));
    }
}
