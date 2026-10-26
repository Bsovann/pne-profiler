"""Unit tests for fragments.py's version rules.

    python3 -m unittest discover -s Changelogs -v
"""

import unittest

from fragments import check_versions, next_version

MAJOR = {"Major_Changes": ["(pne-cli) - removed a flag."]}
MINOR = {"Minor_Changes": ["(pne-cli) - added a flag."]}
BUGFIX = {"Bug_Fixes": ["(pne-cli) - fixed a flag."]}
TRIVIAL = {"Trivial_Changes": ["(Docs) - reworded a line."]}


def release(version, date="2026-10-03", **sections):
    return {"Version": version, "Release_Date": date, **sections}


class NextVersionPreStable(unittest.TestCase):
    def test_major_bumps_minor(self):
        self.assertEqual(next_version("0.4.2", MAJOR), "0.5.0")

    def test_minor_bumps_minor(self):
        self.assertEqual(next_version("0.4.2", MINOR), "0.5.0")

    def test_major_and_minor_bump_minor_once(self):
        self.assertEqual(next_version("0.4.2", {**MAJOR, **MINOR}), "0.5.0")

    def test_bugfix_bumps_patch(self):
        self.assertEqual(next_version("0.4.2", BUGFIX), "0.4.3")

    def test_stable_opts_in_to_1_0_0(self):
        self.assertEqual(next_version("0.4.2", MAJOR, stable=True), "1.0.0")
        self.assertEqual(next_version("0.4.2", BUGFIX, stable=True), "1.0.0")

    def test_1_0_0_is_kept_by_later_folds(self):
        current = {"Version": "1.0.0", **MINOR}
        self.assertEqual(next_version("0.4.2", current), "1.0.0")


class NextVersionStable(unittest.TestCase):
    def test_major_bumps_major(self):
        self.assertEqual(next_version("1.4.2", MAJOR), "2.0.0")

    def test_minor_bumps_minor(self):
        self.assertEqual(next_version("1.4.2", MINOR), "1.5.0")

    def test_bugfix_bumps_patch(self):
        self.assertEqual(next_version("1.4.2", BUGFIX), "1.4.3")

    def test_stable_is_rejected(self):
        with self.assertRaises(ValueError):
            next_version("1.4.2", MAJOR, stable=True)


class NextVersionEdgeCases(unittest.TestCase):
    def test_trivial_only_keeps_previous(self):
        self.assertEqual(next_version("0.4.2", TRIVIAL), "0.4.2")
        self.assertEqual(next_version("1.4.2", TRIVIAL), "1.4.2")

    def test_first_release_keeps_its_version(self):
        self.assertEqual(next_version(None, {"Version": "0.1.0", **MAJOR}), "0.1.0")

    def test_first_release_rejects_stable(self):
        with self.assertRaises(ValueError):
            next_version(None, {"Version": "0.1.0"}, stable=True)

    def test_bumps_from_previous_not_current(self):
        # Three minor folds into one release still give one bump.
        self.assertEqual(next_version("0.4.2", {"Version": "0.5.0", **MINOR}), "0.5.0")

    def test_invalid_previous_is_rejected(self):
        with self.assertRaises(ValueError):
            next_version("v0.4", MINOR)


class CheckVersions(unittest.TestCase):
    def test_increasing_versions_pass(self):
        releases = [release("0.1.0"), release("0.2.0"), release("0.2.1")]
        self.assertEqual(check_versions(releases), [])

    def test_unreleased_trivial_only_passes(self):
        releases = [release("0.2.0"), release("0.2.0", date="TBD", **TRIVIAL)]
        self.assertEqual(check_versions(releases), [])

    def test_reused_version_fails_with_advice(self):
        releases = [release("0.2.0"), release("0.2.0", **TRIVIAL)]
        [error] = check_versions(releases)
        self.assertIn("0.2.0 was already released", error)
        self.assertIn("Release_Date back to TBD", error)

    def test_lower_version_fails(self):
        releases = [release("0.3.0"), release("0.2.1")]
        [error] = check_versions(releases)
        self.assertIn("lower than the previous release", error)


if __name__ == "__main__":
    unittest.main()
