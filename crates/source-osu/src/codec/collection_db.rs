//! collection.db (`research/scripts/rejudge/legacy_db.md`). The encoder is public for F4's
//! export; writing the file is `app::export`'s job (D9).

use crate::codec::version::{admit, finish};
use crate::codec::{FileKind, OsuString, Reader, Writer};
use crate::diag::Diagnostics;
use crate::error::CodecError;

const KIND: FileKind = FileKind::CollectionDb;
/// Smallest collection on disk: an absent-name tag plus the `Int` md5 count.
const MIN_COLLECTION_SIZE: usize = 1 + 4;
/// Smallest md5 entry: an absent-string tag.
const MIN_STRING_SIZE: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionDb {
    pub version: i32,
    pub collections: Vec<Collection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub name: OsuString,
    pub beatmap_md5s: Vec<OsuString>,
}

pub fn decode_collection_db(bytes: &[u8]) -> Result<(CollectionDb, Diagnostics), CodecError> {
    let mut r = Reader::new(bytes, KIND);
    let version = r.i32()?;
    let class = admit(KIND, version)?;
    let body = decode_body(&mut r).map(|collections| {
        (
            CollectionDb {
                version,
                collections,
            },
            Diagnostics::new(),
        )
    });
    finish(KIND, version, class, body)
}

fn decode_body(r: &mut Reader<'_>) -> Result<Vec<Collection>, CodecError> {
    let n = r.count(MIN_COLLECTION_SIZE)?;
    let mut collections = Vec::with_capacity(n);
    for _ in 0..n {
        let name = r.osu_string()?;
        let m = r.count(MIN_STRING_SIZE)?;
        let beatmap_md5s = (0..m).map(|_| r.osu_string()).collect::<Result<_, _>>()?;
        collections.push(Collection { name, beatmap_md5s });
    }
    r.expect_eof()?;
    Ok(collections)
}

/// Exact inverse of [`decode_collection_db`] for any value it produces (minimal ULEB128, as
/// osu! writes it).
pub fn encode_collection_db(db: &CollectionDb) -> Vec<u8> {
    let mut w = Writer::new();
    w.i32(db.version);
    w.count(db.collections.len());
    for c in &db.collections {
        w.osu_string(&c.name);
        w.count(c.beatmap_md5s.len());
        for md5 in &c.beatmap_md5s {
            w.osu_string(md5);
        }
    }
    w.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CodecError;

    fn fixture_bytes() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(20_260_624_i32.to_le_bytes());
        b.extend(2_i32.to_le_bytes());
        b.extend([0x0b, 3]);
        b.extend(b"LN!");
        b.extend(2_i32.to_le_bytes());
        b.extend([0x0b, 32]);
        b.extend([b'a'; 32]);
        b.push(0x00);
        b.push(0x00);
        b.extend(0_i32.to_le_bytes());
        b
    }

    #[test]
    fn decode_fixture() {
        let (db, diags) = decode_collection_db(&fixture_bytes()).unwrap();
        assert!(diags.is_empty());
        assert_eq!(db.version, 20_260_624);
        assert_eq!(db.collections.len(), 2);
        assert_eq!(db.collections[0].name, OsuString::present(*b"LN!"));
        assert_eq!(
            db.collections[0].beatmap_md5s,
            vec![OsuString::present([b'a'; 32]), OsuString::Absent]
        );
        assert_eq!(db.collections[1].name, OsuString::Absent);
        assert!(db.collections[1].beatmap_md5s.is_empty());
    }

    #[test]
    fn encode_is_minimal_uleb() {
        let db = CollectionDb {
            version: 20_260_624,
            collections: vec![Collection {
                name: OsuString::present(vec![b'x'; 200]),
                beatmap_md5s: vec![],
            }],
        };
        let bytes = encode_collection_db(&db);
        // 200 needs two ULEB groups: 0xc8 0x01.
        assert_eq!(&bytes[8..11], &[0x0b, 0xc8, 0x01]);
        assert_eq!(bytes.len(), 8 + 3 + 200 + 4);
        assert_eq!(
            encode_collection_db(&decode_collection_db(&fixture_bytes()).unwrap().0),
            fixture_bytes()
        );
    }

    #[test]
    fn rejects_structural_errors() {
        let mut trailing = fixture_bytes();
        trailing.push(0);
        assert!(matches!(
            decode_collection_db(&trailing),
            Err(CodecError::TrailingBytes {
                kind: FileKind::CollectionDb,
                ..
            })
        ));
        let mut old = fixture_bytes();
        old[..4].copy_from_slice(&20_140_608_i32.to_le_bytes());
        assert!(matches!(
            decode_collection_db(&old),
            Err(CodecError::UnsupportedFormat { .. })
        ));
        let mut newer = fixture_bytes();
        newer[..4].copy_from_slice(&20_270_101_i32.to_le_bytes());
        let (_, diags) = decode_collection_db(&newer).unwrap();
        assert!(diags.contains(crate::diag::DiagCode::FormatUnverifiedVersion));
        newer.push(0);
        assert!(matches!(
            decode_collection_db(&newer),
            Err(CodecError::UnsupportedFormat { .. })
        ));
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;

    use super::*;
    use crate::codec::version::{COLLECTION_DB_MIN, COLLECTION_DB_NEWEST_VERIFIED};

    fn osu_string() -> impl Strategy<Value = OsuString> {
        prop_oneof![
            Just(OsuString::Absent),
            proptest::collection::vec(any::<u8>(), 0..160).prop_map(OsuString::Present),
        ]
    }

    fn collection_db() -> impl Strategy<Value = CollectionDb> {
        let collection = (osu_string(), proptest::collection::vec(osu_string(), 0..6))
            .prop_map(|(name, beatmap_md5s)| Collection { name, beatmap_md5s });
        (
            COLLECTION_DB_MIN..=COLLECTION_DB_NEWEST_VERIFIED,
            proptest::collection::vec(collection, 0..6),
        )
            .prop_map(|(version, collections)| CollectionDb {
                version,
                collections,
            })
    }

    /// Hand-rolled so the byte-level property does not reuse the encoder under test.
    fn raw_bytes(db: &CollectionDb) -> Vec<u8> {
        fn string(out: &mut Vec<u8>, s: &OsuString) {
            match s {
                OsuString::Absent => out.push(0),
                OsuString::Present(b) => {
                    out.push(0x0b);
                    let mut n = b.len();
                    loop {
                        let group = (n & 0x7f) as u8;
                        n >>= 7;
                        if n == 0 {
                            out.push(group);
                            break;
                        }
                        out.push(group | 0x80);
                    }
                    out.extend(b);
                }
            }
        }
        let mut out = db.version.to_le_bytes().to_vec();
        out.extend((db.collections.len() as i32).to_le_bytes());
        for c in &db.collections {
            string(&mut out, &c.name);
            out.extend((c.beatmap_md5s.len() as i32).to_le_bytes());
            for m in &c.beatmap_md5s {
                string(&mut out, m);
            }
        }
        out
    }

    proptest! {
        #[test]
        fn encode_decode_roundtrip(db in collection_db()) {
            let (decoded, diags) = decode_collection_db(&encode_collection_db(&db)).unwrap();
            prop_assert!(diags.is_empty());
            prop_assert_eq!(decoded, db);
        }

        #[test]
        fn decode_encode_bytes_identical(db in collection_db()) {
            let bytes = raw_bytes(&db);
            let (decoded, _) = decode_collection_db(&bytes).unwrap();
            prop_assert_eq!(encode_collection_db(&decoded), bytes);
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..128)) {
            let _ = decode_collection_db(&bytes);
        }
    }
}
