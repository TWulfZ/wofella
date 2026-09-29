use crate::codec::Writer;
use crate::codec::osg::OsgFile;

/// Test-only inverse of `decode_osg`; wolluf never writes `.osg` files (spec 006 Scope).
pub fn encode_osg(osg: &OsgFile) -> Vec<u8> {
    let mut w = Writer::new();
    w.i32(osg.client_version);
    w.count(osg.records.len());
    for r in &osg.records {
        w.i32(r.t_ms);
        w.u8(r.b4);
        let c = &r.counts;
        for n in [c.n300, c.n100, c.n50, c.geki, c.katu, c.miss] {
            w.u16(n);
        }
        w.i32(r.score);
        w.u16(r.max_combo);
        w.u16(r.combo);
        w.u8(r.b25);
        w.u16(r.hp_raw);
        w.u8(r.b28);
        if let Some([f0, f1]) = r.v2 {
            w.f64(f0);
            w.f64(f1);
        }
    }
    w.into_bytes()
}
