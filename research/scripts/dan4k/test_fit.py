"""Self-check of the 4K dan fit on synthetic rows. Run: python3 -I research/scripts/dan4k/test_fit.py"""
import sys

sys.dont_write_bytecode = True

import os
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fit  # noqa: E402


def row(title, version, msd, creator="m", ln=0.0, md5=None):
    return {
        "md5": md5 or f"{title}|{version}|{creator}",
        "title": title,
        "artist": "a",
        "version": version,
        "creator": creator,
        "lnRatio": ln,
        "msdOverallCenti": msd,
    }


class ClassifyTests(unittest.TestCase):
    def dan(self, title, version, **kw):
        verdict = fit.classify(row(title, version, 2000, **kw))
        return verdict.dan if verdict.matched else verdict.reason

    def test_course_token_in_the_version(self):
        self.assertEqual(self.dan("Dan ~ REFORM ~ JackMap Pack", "Flashes ~ 2nd ~ (Marathon)"), 2)
        self.assertEqual(self.dan("Dan ~ REFORM ~ TechMap Pack", "Rave 7.7 ~ 10th ~ (Marathon)"), 10)
        self.assertEqual(self.dan("Dan ~ REFORM ~ JackMap Pack", "Rose Quartz ~ Epsilon ~ (Marathon)"), 15)
        self.assertEqual(self.dan("Dan ~ REFORM ~ 2nd Pack", "~ EXTRA-ALPHA ~ (Marathon)"), 11)

    def test_rate_variants_of_a_course_chart_are_skipped(self):
        self.assertEqual(
            self.dan("Dan ~ REFORM ~ JackMap Pack", "Aquaris ~ Delta ~ (Marathon) 0.95x (173bpm)"),
            "course rate variant",
        )

    def test_unstructured_greek_word_in_a_version_is_not_a_dan(self):
        self.assertEqual(self.dan("Dan ~ REFORM ~ JackMap Pack", "Beta Jack trill"), "no dan token")

    def test_tier_from_the_pack_title(self):
        self.assertEqual(self.dan("10th Dan Practice Pack ~ Jack ~", "Shiva ~ Relect [Mid/High]"), 10)
        self.assertEqual(self.dan("Alpha speedjack pack 2", "[x] y - z 1.1x (Mid)"), 11)

    def test_version_tier_wins_over_the_title_tier(self):
        self.assertEqual(self.dan("Gamma++ Tech Collection", "[Delta Low]  Pluto (Windoze)"), 14)
        self.assertEqual(self.dan("Gamma++ Tech Collection", "[Gamma Mid~High]  NS22 x1.2"), 13)
        self.assertEqual(
            self.dan("The Journey from Beta to Gamma (Tech)", "Teriqma 1.1x ~ Gamma ~ (Mid)"), 13
        )

    def test_exclusions(self):
        self.assertEqual(self.dan("4K Thumb Dan - B Pack", "1st - I (Chroma)"), "thumb scale")
        self.assertEqual(self.dan("4K Pre-Alpha Practice Pack", "Da Xi (JACK)"), "pre-tier")
        self.assertEqual(self.dan("Last Remote - Type gamma", "Another"), "not a pack")
        self.assertEqual(self.dan("Alpha Tech Practice Pack", "Delete After Download"), "placeholder")
        self.assertEqual(self.dan("Alpha Tech Practice Pack", "x (Mid)", ln=0.5), "LN-heavy")
        self.assertEqual(self.dan("7K Road to Gamma Dan Pack", "x"), "other keymode")

    def test_unrated_charts_are_skipped(self):
        verdict = fit.classify(row("Alpha Tech Practice Pack", "x (Mid)", None))
        self.assertEqual(verdict.reason, "no MSD")


class FitTests(unittest.TestCase):
    def test_pool_adjacent_violators(self):
        self.assertEqual(fit.pav([1.0, 3.0, 2.0, 4.0], [1, 1, 1, 1]), [1.0, 2.5, 2.5, 4.0])
        self.assertEqual(fit.pav([5.0, 1.0], [3, 1]), [4.0, 4.0])

    def test_bounds_sit_at_midpoints_and_extrapolate_the_ends(self):
        table = fit.table_from_means({1: 1000.0, 2: 1200.0, 3: 1600.0})
        self.assertEqual(table.bounds, [(1, 900), (2, 1100), (3, 1400)])
        self.assertEqual(table.top_span, 300)

    def test_tied_means_keep_bounds_strictly_increasing(self):
        table = fit.table_from_means({1: 1000.0, 2: 1000.0, 3: 1000.0, 4: 1200.0})
        lowers = [b for _, b in table.bounds]
        self.assertEqual(lowers, sorted(set(lowers)))

    def test_predict(self):
        table = fit.table_from_means({1: 1000.0, 2: 1200.0, 3: 1600.0})
        self.assertEqual(fit.predict(table, 899), 0)
        self.assertEqual(fit.predict(table, 900), 1)
        self.assertEqual(fit.predict(table, 1100), 2)
        self.assertEqual(fit.predict(table, 9999), 3)

    def test_pack_balanced_means(self):
        charts = [
            fit.Chart("a", "P", 1, 1000),
            fit.Chart("b", "P", 1, 1200),
            fit.Chart("c", "Q", 1, 2000),
        ]
        self.assertEqual(fit.dan_means(charts), ({1: 1550.0}, {1: 2}))

    def test_leave_one_pack_out_scores_held_out_charts(self):
        charts = [fit.Chart(str(i), p, d, m) for i, (p, d, m) in enumerate([
            ("P", 1, 1000), ("P", 2, 1200), ("Q", 1, 1010), ("Q", 2, 1190), ("R", 1, 995), ("R", 2, 1205),
        ])]
        result = fit.leave_one_pack_out(charts)
        self.assertEqual(result["charts"], 6)
        self.assertEqual(result["exact_pct"], 100.0)
        self.assertEqual(result["mae"], 0.0)


if __name__ == "__main__":
    unittest.main()
