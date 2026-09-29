//! Persisted timestamps: RFC 3339 UTC with milliseconds (`2026-09-28T23:13:56.636Z`, spec 003
//! Data). Hand-rolled because no date crate is pinned and only this one fixed shape is needed.

use wolluf_core::UnixUs;

use crate::error::StoreError;

const US_PER_MS: i64 = 1_000;
const MS_PER_SEC: i64 = 1_000;
const SECS_PER_MIN: i64 = 60;
const SECS_PER_HOUR: i64 = 3_600;
const SECS_PER_DAY: i64 = 86_400;

// Civil-date conversion from H. Hinnant, "chrono-Compatible Low-Level Date Algorithms"
// (days_from_civil / civil_from_days). Years are counted from March so the leap day is last.
const DAYS_PER_ERA: i64 = 146_097;
const YEARS_PER_ERA: i64 = 400;
/// Days from 0000-03-01 to 1970-01-01.
const UNIX_EPOCH_SHIFT: i64 = 719_468;

const RFC3339_MS_LEN: usize = "2026-09-28T23:13:56.636Z".len();

struct Civil {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    milli: i64,
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(YEARS_PER_ERA);
    let yoe = y - era * YEARS_PER_ERA;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * DAYS_PER_ERA + doe - UNIX_EPOCH_SHIFT
}

fn civil_from_unix_us(t: UnixUs) -> Civil {
    let ms = t.0.div_euclid(US_PER_MS);
    let secs = ms.div_euclid(MS_PER_SEC);
    let days = secs.div_euclid(SECS_PER_DAY);
    let sod = secs.rem_euclid(SECS_PER_DAY);

    let z = days + UNIX_EPOCH_SHIFT;
    let era = z.div_euclid(DAYS_PER_ERA);
    let doe = z - era * DAYS_PER_ERA;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / (DAYS_PER_ERA - 1)) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * YEARS_PER_ERA + i64::from(month <= 2);

    Civil {
        year,
        month,
        day,
        hour: sod / SECS_PER_HOUR,
        minute: sod % SECS_PER_HOUR / SECS_PER_MIN,
        second: sod % SECS_PER_MIN,
        milli: ms.rem_euclid(MS_PER_SEC),
    }
}

/// Floors to the millisecond, which is all a persisted timestamp keeps.
pub fn format_rfc3339_ms(t: UnixUs) -> String {
    let c = civil_from_unix_us(t);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        c.year, c.month, c.day, c.hour, c.minute, c.second, c.milli
    )
}

/// `yyyymmddThhmmssZ`, the backup file name stamp (spec 003).
pub(crate) fn format_compact_utc(t: UnixUs) -> String {
    let c = civil_from_unix_us(t);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// Accepts exactly what [`format_rfc3339_ms`] writes; anything else is corrupt data.
pub fn parse_rfc3339_ms(s: &str) -> Result<UnixUs, StoreError> {
    let invalid = || StoreError::InvalidData(format!("timestamp {s:?}"));
    let b = s.as_bytes();
    let shape_ok = b.len() == RFC3339_MS_LEN
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 => c == b'-',
            10 => c == b'T',
            13 | 16 => c == b':',
            19 => c == b'.',
            23 => c == b'Z',
            _ => c.is_ascii_digit(),
        });
    if !shape_ok {
        return Err(invalid());
    }
    let num = |from: usize, to: usize| {
        b[from..to]
            .iter()
            .fold(0_i64, |acc, &d| acc * 10 + i64::from(d - b'0'))
    };
    let (year, month, day) = (num(0, 4), num(5, 7), num(8, 10));
    let (hour, minute, second, milli) = (num(11, 13), num(14, 16), num(17, 19), num(20, 23));
    let in_range = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < SECS_PER_MIN
        && second < SECS_PER_MIN;
    if !in_range {
        return Err(invalid());
    }
    let days = days_from_civil(year, month, day);
    let secs = days * SECS_PER_DAY + hour * SECS_PER_HOUR + minute * SECS_PER_MIN + second;
    let t = UnixUs((secs * MS_PER_SEC + milli) * US_PER_MS);
    // Rejects impossible dates such as Feb 30, which would otherwise roll into March.
    if format_rfc3339_ms(t) != s {
        return Err(invalid());
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Computed independently with Python's datetime.
    const VECTORS: &[(&str, i64)] = &[
        ("2026-09-28T23:13:56.636Z", 1_790_637_236_636_000),
        ("1970-01-01T00:00:00.000Z", 0),
        ("2000-02-29T12:00:00.001Z", 951_825_600_001_000),
        ("1969-12-31T23:59:59.999Z", -1_000),
        ("1601-01-01T00:00:00.000Z", -11_644_473_600_000_000),
        ("9999-12-31T23:59:59.999Z", 253_402_300_799_999_000),
    ];

    #[test]
    fn rfc3339_vectors_roundtrip() {
        for &(text, us) in VECTORS {
            assert_eq!(format_rfc3339_ms(UnixUs(us)), text);
            assert_eq!(parse_rfc3339_ms(text).unwrap(), UnixUs(us), "{text}");
        }
    }

    #[test]
    fn format_floors_sub_millisecond() {
        assert_eq!(format_rfc3339_ms(UnixUs(999)), "1970-01-01T00:00:00.000Z");
        assert_eq!(format_rfc3339_ms(UnixUs(-1)), "1969-12-31T23:59:59.999Z");
    }

    #[test]
    fn compact_stamp() {
        assert_eq!(
            format_compact_utc(UnixUs(1_790_637_236_636_000)),
            "20260928T231356Z"
        );
    }

    #[test]
    fn parse_rejects_other_shapes() {
        for bad in [
            "2026-09-28T23:13:56Z",
            "2026-09-28T23:13:56.636+00:00",
            "2026-02-30T00:00:00.000Z",
            "2026-13-01T00:00:00.000Z",
            "2026-09-28 23:13:56.636Z",
            "",
        ] {
            assert!(parse_rfc3339_ms(bad).is_err(), "{bad}");
        }
    }
}
