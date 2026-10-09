//! Exact decimal time scaling, so `t / r` never picks up binary floating-point error.

/// Caps the mantissa at 10^30 so `mantissa · 2 · 10^6` stays far below `i128::MAX`.
const MAX_DIGITS: usize = 30;

/// `mantissa / 10^scale`, parsed from the file's own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Decimal {
    mantissa: i128,
    scale: u32,
}

impl Decimal {
    /// Plain `[+-]digits[.digits]`; osu! never writes exponents in time fields.
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let (negative, body) = match s.as_bytes().first()? {
            b'-' => (true, &s[1..]),
            b'+' => (false, &s[1..]),
            _ => (false, s),
        };
        let (int, frac) = body.split_once('.').unwrap_or((body, ""));
        let digits = int.len() + frac.len();
        if digits == 0
            || digits > MAX_DIGITS
            || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let mantissa = int
            .bytes()
            .chain(frac.bytes())
            .fold(0i128, |m, b| m * 10 + i128::from(b - b'0'));
        Some(Self {
            mantissa: if negative { -mantissa } else { mantissa },
            scale: u32::try_from(frac.len()).ok()?,
        })
    }

    pub(crate) fn scale(self) -> u32 {
        self.scale
    }

    /// `round_half_up(self / (rate_milli / 1000))` in units of `10^-decimals`; `None` only when
    /// `decimals` exceeds the scale by so much that the exact quotient overflows.
    pub(crate) fn div_rate(self, rate_milli: u16, decimals: u32) -> Option<i128> {
        // Only the scale difference enters the product, so a 30-digit source never overflows.
        let (num, den) = if decimals >= self.scale {
            let up = 10i128.checked_pow(decimals - self.scale)?;
            (
                self.mantissa.checked_mul(1000)?.checked_mul(up)?,
                i128::from(rate_milli),
            )
        } else {
            let down = 10i128.checked_pow(self.scale - decimals)?;
            (
                self.mantissa * 1000,
                i128::from(rate_milli).checked_mul(down)?,
            )
        };
        // floor(x + 1/2) as one exact integer division; div_euclid floors for negatives too.
        Some(
            num.checked_mul(2)?
                .checked_add(den)?
                .div_euclid(den.checked_mul(2)?),
        )
    }
}

/// `value / 10^decimals` with trailing fractional zeros (and a bare point) dropped.
pub(crate) fn format_fixed(value: i128, decimals: u32) -> String {
    let unit = 10i128.pow(decimals);
    let sign = if value < 0 { "-" } else { "" };
    let abs = value.unsigned_abs();
    let unit = unit.unsigned_abs();
    let (int, frac) = (abs / unit, abs % unit);
    if frac == 0 {
        return format!("{sign}{int}");
    }
    let width = usize::try_from(decimals).unwrap_or(0);
    let frac = format!("{frac:0width$}");
    format!("{sign}{int}.{}", frac.trim_end_matches('0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scaled(s: &str, rate: u16, decimals: u32) -> String {
        format_fixed(
            Decimal::parse(s).unwrap().div_rate(rate, decimals).unwrap(),
            decimals,
        )
    }

    #[test]
    fn parses_plain_decimals_only() {
        assert!(Decimal::parse("12").is_some());
        assert!(Decimal::parse(" -12.50 ").is_some());
        assert!(Decimal::parse("+.5").is_some());
        assert!(Decimal::parse("1.").is_some());
        for bad in ["", "-", ".", "1e3", "1.2.3", "abc", "NaN", "1 2"] {
            assert_eq!(Decimal::parse(bad), None, "{bad:?}");
        }
        assert_eq!(Decimal::parse(&"9".repeat(31)), None);
    }

    #[test]
    fn rounds_half_up_toward_positive_infinity() {
        assert_eq!(scaled("2", 800, 0), "3");
        assert_eq!(scaled("-2", 800, 0), "-2");
        assert_eq!(scaled("-3", 800, 0), "-4");
        assert_eq!(scaled("4", 1150, 0), "3");
        assert_eq!(scaled("0", 1150, 0), "0");
    }

    #[test]
    fn keeps_up_to_the_requested_decimals() {
        assert_eq!(scaled("-30", 1150, 3), "-26.087");
        assert_eq!(scaled("1000.5", 1150, 3), "870");
        assert_eq!(scaled("1000.5", 1250, 3), "800.4");
        assert_eq!(scaled("1000.50", 1000, 3), "1000.5");
        assert_eq!(scaled("-2", 800, 3), "-2.5");
        assert_eq!(scaled("0.0004", 1000, 3), "0");
        assert_eq!(scaled("-0.0004", 1000, 3), "0");
    }

    #[test]
    fn rate_1000_at_the_source_scale_is_the_identity() {
        let thirty = format!("0.{}12", "123456789".repeat(3));
        let max_int = "9".repeat(30);
        for s in ["1000.12345", "-0.5", "12", &thirty, &max_int] {
            let d = Decimal::parse(s).unwrap();
            let decimals = d.scale().max(3);
            let got = format_fixed(d.div_rate(1000, decimals).unwrap(), decimals);
            assert_eq!(got, s.trim_start_matches('+'), "{s}");
        }
        assert_eq!(scaled(&max_int, 500, 0), format!("1{}8", "9".repeat(29)));
    }
}
