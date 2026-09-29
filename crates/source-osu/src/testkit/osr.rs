use wolluf_core::{ChartMd5, DotNetTicks};

use crate::codec::Writer;
use crate::codec::replay_name::{ReplayFileKind, ReplayFileName};
use crate::codec::score_header::{OnlineId, ScoreHeader, write_online_id, write_score_header};
use crate::testkit::ScoreBuilder;

/// LZMA_ALONE properties byte 0x5d, as every stable replay starts; the rest is opaque filler.
const DEFAULT_PAYLOAD: [u8; 8] = [0x5d, 0, 0, 0x20, 0, 1, 2, 3];

/// Builds `.osr` bytes around a score header; the payload is embedded verbatim, never compressed.
#[derive(Debug, Clone)]
pub struct OsrBuilder {
    score: ScoreBuilder,
    payload: Option<Vec<u8>>,
    trailing: Vec<u8>,
}

impl OsrBuilder {
    pub fn new(score: ScoreBuilder) -> Self {
        Self {
            score,
            payload: Some(DEFAULT_PAYLOAD.to_vec()),
            trailing: Vec::new(),
        }
    }

    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Declared length 0, like the 3 pilot files of 126 bytes.
    pub fn empty_payload(mut self) -> Self {
        self.payload = None;
        self
    }

    pub fn trailing(mut self, bytes: Vec<u8>) -> Self {
        self.trailing = bytes;
        self
    }

    /// The `Data/r` name osu! would give this replay; `None` if the header md5 is not valid hex
    /// or the ticks predate 1601.
    pub fn file_name(&self) -> Option<ReplayFileName> {
        let header = self.score.header();
        let md5: ChartMd5 = std::str::from_utf8(header.beatmap_md5.as_bytes()?)
            .ok()?
            .parse()
            .ok()?;
        Some(ReplayFileName {
            md5,
            filetime: DotNetTicks(header.timestamp_ticks).to_filetime()?,
            kind: ReplayFileKind::Osr,
        })
    }

    pub fn build(self) -> Vec<u8> {
        let record = self.score.build();
        let mut bytes = encode_osr(
            &record.header,
            self.payload.as_deref(),
            record.online_id,
            record.target_practice,
        );
        bytes.extend_from_slice(&self.trailing);
        bytes
    }
}

/// `None` writes a declared length of 0.
pub fn encode_osr(
    header: &ScoreHeader,
    payload: Option<&[u8]>,
    online_id: OnlineId,
    target_practice: Option<f64>,
) -> Vec<u8> {
    let mut w = Writer::new();
    write_score_header(&mut w, header);
    match payload {
        Some(p) => {
            w.count(p.len());
            w.bytes(p);
        }
        None => w.i32(0),
    }
    write_online_id(&mut w, online_id);
    if let Some(tp) = target_practice {
        w.f64(tp);
    }
    w.into_bytes()
}
