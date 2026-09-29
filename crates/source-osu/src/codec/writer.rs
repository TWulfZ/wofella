//! Mirror of [`Reader`](super::Reader) for encoders; ULEB128 is always minimal.

use super::reader::{OsuString, STRING_ABSENT, STRING_PRESENT};

const ULEB128_CONTINUE: u8 = 0x80;
const ULEB128_PAYLOAD: u64 = 0x7f;
const ULEB128_GROUP_BITS: u32 = 7;

#[derive(Debug, Clone, Default)]
pub struct Writer {
    buf: Vec<u8>,
}

macro_rules! le_writers {
    ($($name:ident: $ty:ty),+ $(,)?) => {
        $(
            pub fn $name(&mut self, value: $ty) {
                self.buf.extend_from_slice(&value.to_le_bytes());
            }
        )+
    };
}

impl Writer {
    pub const fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    le_writers!(
        u8: u8,
        u16: u16,
        i16: i16,
        u32: u32,
        i32: i32,
        i64: i64,
        f32: f32,
        f64: f64,
    );

    pub fn bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn uleb128(&mut self, mut value: u64) {
        loop {
            let group = (value & ULEB128_PAYLOAD) as u8;
            value >>= ULEB128_GROUP_BITS;
            if value == 0 {
                self.buf.push(group);
                return;
            }
            self.buf.push(group | ULEB128_CONTINUE);
        }
    }

    pub fn osu_string(&mut self, s: &OsuString) {
        match s {
            OsuString::Absent => self.buf.push(STRING_ABSENT),
            OsuString::Present(bytes) => {
                self.buf.push(STRING_PRESENT);
                self.uleb128(bytes.len() as u64);
                self.buf.extend_from_slice(bytes);
            }
        }
    }

    /// Decoders only ever produce counts that fit an `i32`, so saturation is unreachable for
    /// any value that came from a decode; it only keeps the encoder total.
    pub fn count(&mut self, n: usize) {
        self.i32(i32::try_from(n).unwrap_or(i32::MAX));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uleb128_is_minimal() {
        let cases: [(u64, &[u8]); 5] = [
            (0, &[0x00]),
            (127, &[0x7f]),
            (128, &[0x80, 0x01]),
            (624_485, &[0xe5, 0x8e, 0x26]),
            (u64::from(u32::MAX), &[0xff, 0xff, 0xff, 0xff, 0x0f]),
        ];
        for (value, expected) in cases {
            let mut w = Writer::new();
            w.uleb128(value);
            assert_eq!(w.as_bytes(), expected, "{value}");
        }
    }
}
