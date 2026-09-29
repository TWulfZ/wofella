"""Self-check of the correlation joins. Run: python3 -m unittest research/scripts/osg/test_correlate.py"""
import sys

sys.dont_write_bytecode = True

import os
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import correlate  # noqa: E402
import osg  # noqa: E402


def ev(t, kinds, idx=0):
    return osg.OsgEvent(idx, t, {}, list(kinds), len(kinds), 0, 0)


def inst(t, kind, cat='note', source='input'):
    return correlate.Instant(t, kind, cat, source)


class InstantTests(unittest.TestCase):
    W = [16, 34, 67, 97, 121, 158]

    def test_note_press_and_deadline_miss(self):
        res = [dict(t=1000, kind='note', j=0, delta=3), dict(t=2000, kind='note', j=5, delta=None)]
        got = correlate.judgement_instants(res, self.W, v2=False)
        self.assertEqual(got, [inst(1003, 'MAX'), inst(2000 + 97, 'miss', source='deadline')])

    def test_v1_ln_is_judged_at_release(self):
        res = [dict(t=1000, end=1500, kind='ln', j=1, head_delta=2, rel_delta=-4)]
        got = correlate.judgement_instants(res, self.W, v2=False)
        self.assertEqual(got, [inst(1496, '300', 'tail')])

    def test_v2_tail_deadline_uses_widened_window(self):
        res = [dict(t=1500, kind='tail', j=5, delta=None)]
        got = correlate.judgement_instants(res, self.W, v2=True)
        # expiry is the first ms strictly after t + (O - 0.5) * 1.5
        self.assertEqual(got, [inst(1500 + 145, 'miss', 'tail', 'deadline')])


class JoinTests(unittest.TestCase):
    def test_unique_single(self):
        r = correlate.join([ev(100, ['MAX'])], [inst(100, 'MAX'), inst(300, 'MAX')], tol=1, by_kind=True)
        self.assertEqual(r['unique'], {'note': 1})
        self.assertEqual(r['units'], 1)

    def test_chord_of_same_kind_is_unique_as_a_set(self):
        r = correlate.join([ev(100, ['MAX', 'MAX'])], [inst(100, 'MAX'), inst(101, 'MAX')], tol=1, by_kind=True)
        self.assertEqual(r['unique'], {'note': 2})

    def test_extra_candidate_is_ambiguous(self):
        cands = [inst(100, 'MAX', 'head'), inst(101, 'MAX', 'head')]
        r = correlate.join([ev(100, ['MAX'])], cands, tol=1, by_kind=True)
        self.assertEqual(r['ambiguous'], {'head': 1})
        self.assertEqual(r['unique'], {})

    def test_kind_mismatch_is_unmatched_but_time_join_finds_it(self):
        e, c = [ev(100, ['300'])], [inst(100, 'MAX')]
        self.assertEqual(correlate.join(e, c, tol=1, by_kind=True)['unmatched'], 1)
        self.assertEqual(correlate.join(e, c, tol=1, by_kind=False)['unique'], {'note': 1})

    def test_carried_unit_resolves_against_previous_record_time(self):
        evs = [ev(5070, ['MAX'], 0), ev(5373, ['MAX', 'MAX'], 1)]
        cands = [inst(5070, 'MAX'), inst(5070, 'MAX'), inst(5373, 'MAX')]
        r = correlate.join(evs, cands, tol=1, by_kind=True)
        self.assertEqual(r['unmatched'], 1)
        self.assertEqual(r['carry_resolved'], 1)

    def test_consumed_candidate_is_not_reused(self):
        r = correlate.join([ev(100, ['MAX'], 0), ev(101, ['MAX'], 1)], [inst(100, 'MAX')], tol=1, by_kind=True)
        self.assertEqual(r['unique'], {'note': 1})
        self.assertEqual(r['unmatched'], 1)

    def test_exact_time_share(self):
        r = correlate.exact_share([ev(100, ['MAX']), ev(200, ['300', 'MAX'])],
                                  [inst(100, 'MAX'), inst(200, 'MAX'), inst(200, '200')], by_kind=True)
        self.assertEqual((r['events_matched'], r['events']), (1, 2))
        self.assertEqual((r['units_matched'], r['units']), (2, 3))
        self.assertEqual(r['by_n'], {'1': dict(events=1, events_matched=1), '2': dict(events=1, events_matched=0)})

    def test_unit_carried_from_previous_record(self):
        # Two presses judged at 5070, but the record at 5070 shows one and the next record two.
        evs = [ev(5070, ['MAX'], 0), ev(5373, ['MAX', 'MAX'], 1)]
        cands = [inst(5070, 'MAX'), inst(5070, 'MAX'), inst(5373, 'MAX')]
        r = correlate.exact_share(evs, cands, by_kind=True)
        self.assertEqual(r['units_carried_from_previous_record'], 1)


class F0Tests(unittest.TestCase):
    def test_f0_increment_model(self):
        self.assertAlmostEqual(correlate.f0_model(1), 150.0)
        self.assertAlmostEqual(correlate.f0_model(3), 237.7443751081735)


if __name__ == '__main__':
    unittest.main()
