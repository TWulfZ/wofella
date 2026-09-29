"""Self-check of the .osg oracle on synthetic bytes. Run: python3 -m unittest research/scripts/osg/test_osg.py"""
import sys

sys.dont_write_bytecode = True

import os
import struct
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import osg  # noqa: E402


def rec(t, counts, score, max_combo, combo, hp=200, v2=None, b4=0, b25=0):
    c300, c100, c50, cmax, c200, cmiss = counts
    b28 = 1 if v2 is not None else 0
    return osg.OsgRecord(t, b4, c300, c100, c50, cmax, c200, cmiss, score, max_combo, combo, b25, hp, b28,
                         *(v2 if v2 is not None else ()))


class DecodeTests(unittest.TestCase):
    def test_decodes_v1_records(self):
        recs = [rec(1000, (0, 0, 0, 1, 0, 0), 320, 1, 1), rec(1200, (1, 0, 0, 1, 0, 0), 620, 2, 2, hp=199)]
        f = osg.decode(osg.encode(20260924, recs))
        self.assertEqual(f.client_version, 20260924)
        self.assertEqual(f.stride, osg.STRIDE_V1)
        self.assertEqual(f.records, recs)
        self.assertEqual(f.records[1].hp_raw, 199)

    def test_decodes_v2_records_with_floats(self):
        recs = [rec(500, (0, 0, 0, 1, 0, 0), 10, 1, 1, v2=(150.0, 0.0))]
        raw = osg.encode(20260711, recs)
        self.assertEqual(len(raw), osg.HEADER.size + osg.STRIDE_V2)
        f = osg.decode(raw)
        self.assertEqual(f.stride, osg.STRIDE_V2)
        self.assertEqual((f.records[0].f0, f.records[0].f1), (150.0, 0.0))

    def test_layout_offsets_match_spec(self):
        raw = osg.encode(1, [rec(0x01020304, (1, 2, 3, 4, 5, 6), 7, 8, 9, hp=10)])
        body = raw[osg.HEADER.size:]
        self.assertEqual(struct.unpack_from('<i', body, 0)[0], 0x01020304)
        self.assertEqual(struct.unpack_from('<6H', body, 5), (1, 2, 3, 4, 5, 6))
        self.assertEqual(struct.unpack_from('<i', body, 17)[0], 7)
        self.assertEqual(struct.unpack_from('<HH', body, 21), (8, 9))
        self.assertEqual(struct.unpack_from('<H', body, 26)[0], 10)

    def test_empty_graph(self):
        f = osg.decode(osg.encode(20260924, []))
        self.assertEqual(f.records, [])
        self.assertIsNone(f.stride)

    def test_rejects_truncated_header(self):
        with self.assertRaises(osg.OsgError):
            osg.decode(b'\x00' * 7)

    def test_rejects_negative_count(self):
        with self.assertRaises(osg.OsgError):
            osg.decode(struct.pack('<ii', 20260924, -1))

    def test_rejects_stride_mismatch(self):
        raw = osg.encode(20260924, [rec(1, (0, 0, 0, 1, 0, 0), 1, 1, 1)])
        with self.assertRaises(osg.OsgError):
            osg.decode(raw + b'\x00')


class EventTests(unittest.TestCase):
    def test_single_merged_and_score_only(self):
        recs = [
            rec(100, (0, 0, 0, 1, 0, 0), 320, 1, 1),
            rec(250, (1, 0, 0, 2, 0, 0), 940, 3, 3),
            rec(900, (1, 0, 0, 2, 0, 1), 940, 3, 0),
            rec(900, (1, 0, 0, 2, 0, 1), 945, 3, 0),
        ]
        ev = osg.events(recs)
        self.assertEqual([e.n for e in ev], [1, 2, 1, 0])
        self.assertEqual(ev[0].kinds, ['MAX'])
        self.assertEqual(ev[1].kinds, ['MAX', '300'])
        self.assertEqual(ev[2].kinds, ['miss'])
        self.assertEqual(ev[3].kinds, [])
        self.assertEqual(ev[1].combo_delta, 2)
        self.assertEqual(ev[2].combo_delta, -3)
        self.assertEqual(ev[3].score_delta, 5)


class NameTests(unittest.TestCase):
    def test_parse_name(self):
        md5 = '0' * 31 + 'a'
        self.assertEqual(osg.parse_name(f'{md5}-134350010443098880.osg'), (md5, 134350010443098880, 'osg'))
        self.assertIsNone(osg.parse_name('foo.osr'))


if __name__ == '__main__':
    unittest.main()
