//! Bounded little-endian reader over `&[u8]` with the osu! `String` and ULEB128 types
//! (`research/scripts/rejudge/legacy_db.md`, "Data Types").

use std::borrow::Cow;
use std::fmt;

use crate::codec::FileKind;
use crate::error::CodecError;

pub(crate) const STRING_ABSENT: u8 = 0x00;
pub(crate) const STRING_PRESENT: u8 = 0x0b;
/// .NET `Read7BitEncodedInt`: a 32-bit value takes at most 5 groups of 7 bits.
const ULEB128_MAX_BYTES: usize = 5;
/// Only the low 4 bits of the fifth group still fit in 32 bits.
const ULEB128_LAST_GROUP_MAX: u8 = 0x0f;
const ULEB128_CONTINUE: u8 = 0x80;
const ULEB128_PAYLOAD: u8 = 0x7f;
const ULEB128_GROUP_BITS: u32 = 7;

/// osu! `String`: "absent" (tag 0x00) and "present but empty" are different values on disk, and
/// raw bytes are kept because names are keys that must round-trip exactly (§5.3).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum OsuString {
    #[default]
    Absent,
    Present(Vec<u8>),
}

impl OsuString {
    pub fn present(bytes: impl Into<Vec<u8>>) -> Self {
        Self::Present(bytes.into())
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Absent => None,
            Self::Present(b) => Some(b),
        }
    }

    pub fn bytes_or_empty(&self) -> &[u8] {
        self.as_bytes().unwrap_or_default()
    }

    pub fn to_string_lossy(&self) -> Option<Cow<'_, str>> {
        self.as_bytes().map(String::from_utf8_lossy)
    }

    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

impl fmt::Debug for OsuString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => f.write_str("Absent"),
            Self::Present(b) => match std::str::from_utf8(b) {
                Ok(s) => write!(f, "Present({s:?})"),
                Err(_) => write!(f, "Present(b\"{}\")", b.escape_ascii()),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    kind: FileKind,
}

macro_rules! le_readers {
    ($($name:ident: $ty:ty),+ $(,)?) => {
        $(
            pub fn $name(&mut self) -> Result<$ty, CodecError> {
                Ok(<$ty>::from_le_bytes(self.array()?))
            }
        )+
    };
}

impl<'a> Reader<'a> {
    pub const fn new(bytes: &'a [u8], kind: FileKind) -> Self {
        Self {
            bytes,
            pos: 0,
            kind,
        }
    }

    pub const fn kind(&self) -> FileKind {
        self.kind
    }

    pub const fn position(&self) -> usize {
        self.pos
    }

    pub const fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    pub const fn offset(&self) -> u64 {
        self.pos as u64
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        if n > self.remaining() {
            return Err(CodecError::Truncated {
                kind: self.kind,
                offset: self.offset(),
                needed: n as u64,
            });
        }
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.bytes(N)?);
        Ok(out)
    }

    le_readers!(
        u8: u8,
        u16: u16,
        i16: i16,
        u32: u32,
        i32: i32,
        i64: i64,
        f32: f32,
        f64: f64,
    );

    pub fn uleb128(&mut self) -> Result<u32, CodecError> {
        let start = self.offset();
        let overflow = CodecError::Uleb128Overflow {
            kind: self.kind,
            offset: start,
        };
        let mut value = 0u32;
        for group in 0..ULEB128_MAX_BYTES {
            let byte = self.u8()?;
            let payload = byte & ULEB128_PAYLOAD;
            let last = group == ULEB128_MAX_BYTES - 1;
            if last && (byte & ULEB128_CONTINUE != 0 || payload > ULEB128_LAST_GROUP_MAX) {
                return Err(overflow);
            }
            value |= u32::from(payload) << (ULEB128_GROUP_BITS * group as u32);
            if byte & ULEB128_CONTINUE == 0 {
                return Ok(value);
            }
        }
        Err(overflow)
    }

    pub fn osu_string(&mut self) -> Result<OsuString, CodecError> {
        let offset = self.offset();
        match self.u8()? {
            STRING_ABSENT => Ok(OsuString::Absent),
            STRING_PRESENT => {
                let len = self.uleb128()? as usize;
                Ok(OsuString::Present(self.bytes(len)?.to_vec()))
            }
            tag => Err(CodecError::BadStringTag {
                kind: self.kind,
                offset,
                tag,
            }),
        }
    }

    /// Reads an `i32` element count and rejects it before any allocation unless
    /// `count × min_record_size` still fits in the remaining bytes.
    pub fn count(&mut self, min_record_size: usize) -> Result<usize, CodecError> {
        let offset = self.offset();
        let raw = self.i32()?;
        let invalid = CodecError::InvalidCount {
            kind: self.kind,
            offset,
            count: i64::from(raw),
        };
        let count = usize::try_from(raw).map_err(|_| invalid.clone())?;
        match count.checked_mul(min_record_size) {
            Some(need) if need <= self.remaining() => Ok(count),
            _ => Err(invalid),
        }
    }

    pub fn expect_eof(&self) -> Result<(), CodecError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(CodecError::TrailingBytes {
                kind: self.kind,
                offset: self.offset(),
                remaining: self.remaining() as u64,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{FileKind, Writer};
    use crate::error::CodecError;

    const KIND: FileKind = FileKind::OsuDb;

    #[test]
    fn uleb128_max_5_bytes() {
        let mut r = Reader::new(&[0xff, 0xff, 0xff, 0xff, 0x0f], KIND);
        assert_eq!(r.uleb128(), Ok(u32::MAX));
        assert_eq!(r.remaining(), 0);
        let mut r = Reader::new(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x00], KIND);
        assert_eq!(
            r.uleb128(),
            Err(CodecError::Uleb128Overflow {
                kind: KIND,
                offset: 0
            })
        );
        // A fifth byte above 0x0f would need a 33rd bit.
        let mut r = Reader::new(&[0xff, 0xff, 0xff, 0xff, 0x1f], KIND);
        assert!(matches!(
            r.uleb128(),
            Err(CodecError::Uleb128Overflow { .. })
        ));
        let mut r = Reader::new(&[0xe5, 0x8e, 0x26], KIND);
        assert_eq!(r.uleb128(), Ok(624_485));
    }

    #[test]
    fn string_tag_must_be_0_or_0x0b() {
        let mut r = Reader::new(&[0x0a, 0x00], KIND);
        assert_eq!(
            r.osu_string(),
            Err(CodecError::BadStringTag {
                kind: KIND,
                offset: 0,
                tag: 0x0a
            })
        );
        let mut r = Reader::new(&[0x0b, 0x02, b'h', b'i'], KIND);
        assert_eq!(r.osu_string(), Ok(OsuString::Present(b"hi".to_vec())));
    }

    #[test]
    fn absent_and_empty_string_differ() {
        let mut r = Reader::new(&[0x00, 0x0b, 0x00], KIND);
        let absent = r.osu_string().unwrap();
        let empty = r.osu_string().unwrap();
        assert_eq!(absent, OsuString::Absent);
        assert_eq!(empty, OsuString::Present(Vec::new()));
        assert_ne!(absent, empty);
        assert_eq!(absent.as_bytes(), None);
        assert_eq!(empty.as_bytes(), Some(&b""[..]));
        let mut w = Writer::new();
        w.osu_string(&absent);
        w.osu_string(&empty);
        assert_eq!(w.into_bytes(), vec![0x00, 0x0b, 0x00]);
    }

    #[test]
    fn count_larger_than_remaining_rejected() {
        let mut bytes = 1_000_000_i32.to_le_bytes().to_vec();
        bytes.extend([0u8; 16]);
        let mut r = Reader::new(&bytes, KIND);
        assert_eq!(
            r.count(1),
            Err(CodecError::InvalidCount {
                kind: KIND,
                offset: 0,
                count: 1_000_000
            })
        );
        let negative = (-1_i32).to_le_bytes();
        let mut r = Reader::new(&negative, KIND);
        assert!(matches!(
            r.count(1),
            Err(CodecError::InvalidCount { count: -1, .. })
        ));
        let mut bytes = 4_i32.to_le_bytes().to_vec();
        bytes.extend([0u8; 8]);
        let mut r = Reader::new(&bytes, KIND);
        assert!(r.clone().count(3).is_err());
        assert_eq!(r.count(2), Ok(4));
    }

    #[test]
    fn truncated_reports_offset_and_needed() {
        let mut r = Reader::new(&[1, 2, 3, 4, 5, 6], KIND);
        assert_eq!(r.u16(), Ok(0x0201));
        assert_eq!(
            r.i64(),
            Err(CodecError::Truncated {
                kind: KIND,
                offset: 2,
                needed: 8
            })
        );
        // A string length beyond the buffer is a truncation, not an allocation.
        let mut r = Reader::new(&[0x0b, 0xff, 0xff, 0x03, b'x'], KIND);
        assert_eq!(
            r.osu_string(),
            Err(CodecError::Truncated {
                kind: KIND,
                offset: 4,
                needed: 65_535
            })
        );
    }

    #[test]
    fn expect_eof_reports_trailing() {
        let mut r = Reader::new(&[1, 2, 3], KIND);
        assert_eq!(r.u8(), Ok(1));
        assert_eq!(
            r.expect_eof(),
            Err(CodecError::TrailingBytes {
                kind: KIND,
                offset: 1,
                remaining: 2
            })
        );
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;

    use super::*;
    use crate::codec::{FileKind, Writer};

    #[derive(Debug, Clone)]
    enum Value {
        U8(u8),
        U16(u16),
        I16(i16),
        U32(u32),
        I32(i32),
        I64(i64),
        F32(u32),
        F64(u64),
        Uleb(u32),
        Str(OsuString),
    }

    fn osu_string() -> impl Strategy<Value = OsuString> {
        prop_oneof![
            Just(OsuString::Absent),
            proptest::collection::vec(any::<u8>(), 0..300).prop_map(OsuString::Present),
        ]
    }

    fn value() -> impl Strategy<Value = Value> {
        prop_oneof![
            any::<u8>().prop_map(Value::U8),
            any::<u16>().prop_map(Value::U16),
            any::<i16>().prop_map(Value::I16),
            any::<u32>().prop_map(Value::U32),
            any::<i32>().prop_map(Value::I32),
            any::<i64>().prop_map(Value::I64),
            any::<u32>().prop_map(Value::F32),
            any::<u64>().prop_map(Value::F64),
            any::<u32>().prop_map(Value::Uleb),
            osu_string().prop_map(Value::Str),
        ]
    }

    fn write(w: &mut Writer, v: &Value) {
        match v {
            Value::U8(x) => w.u8(*x),
            Value::U16(x) => w.u16(*x),
            Value::I16(x) => w.i16(*x),
            Value::U32(x) => w.u32(*x),
            Value::I32(x) => w.i32(*x),
            Value::I64(x) => w.i64(*x),
            Value::F32(bits) => w.f32(f32::from_bits(*bits)),
            Value::F64(bits) => w.f64(f64::from_bits(*bits)),
            Value::Uleb(x) => w.uleb128(u64::from(*x)),
            Value::Str(s) => w.osu_string(s),
        }
    }

    fn read_back(r: &mut Reader<'_>, v: &Value) -> bool {
        match v {
            Value::U8(x) => r.u8() == Ok(*x),
            Value::U16(x) => r.u16() == Ok(*x),
            Value::I16(x) => r.i16() == Ok(*x),
            Value::U32(x) => r.u32() == Ok(*x),
            Value::I32(x) => r.i32() == Ok(*x),
            Value::I64(x) => r.i64() == Ok(*x),
            Value::F32(bits) => r.f32().map(f32::to_bits) == Ok(*bits),
            Value::F64(bits) => r.f64().map(f64::to_bits) == Ok(*bits),
            Value::Uleb(x) => r.uleb128() == Ok(*x),
            Value::Str(s) => r.osu_string().as_ref() == Ok(s),
        }
    }

    proptest! {
        #[test]
        fn writer_reader_roundtrip(values in proptest::collection::vec(value(), 0..40)) {
            let mut w = Writer::new();
            for v in &values {
                write(&mut w, v);
            }
            let bytes = w.into_bytes();
            let mut r = Reader::new(&bytes, FileKind::OsuDb);
            for v in &values {
                prop_assert!(read_back(&mut r, v), "{v:?}");
            }
            prop_assert_eq!(r.expect_eof(), Ok(()));
        }

        #[test]
        fn arbitrary_bytes_never_panic(
            bytes in proptest::collection::vec(any::<u8>(), 0..256),
            ops in proptest::collection::vec(0u8..12, 0..64),
        ) {
            let mut r = Reader::new(&bytes, FileKind::ScoresDb);
            for op in ops {
                let ok = match op {
                    0 => r.u8().is_ok(),
                    1 => r.u16().is_ok(),
                    2 => r.i16().is_ok(),
                    3 => r.u32().is_ok(),
                    4 => r.i32().is_ok(),
                    5 => r.i64().is_ok(),
                    6 => r.f32().is_ok(),
                    7 => r.f64().is_ok(),
                    8 => r.uleb128().is_ok(),
                    9 => r.osu_string().is_ok(),
                    10 => r.count(1).is_ok(),
                    _ => r.bytes(usize::from(op)).is_ok(),
                };
                prop_assert!(r.position() <= bytes.len());
                if !ok {
                    break;
                }
            }
        }
    }
}
