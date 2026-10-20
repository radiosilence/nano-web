"""Tests for changelog.py, against fixtures in a temporary directory.

    python3 -m unittest scripts/test_changelog.py
"""

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("changelog.py")

RELEASED = """# Changelog

## 0.10.0

The database moves to schema 3.

### Security

- **Tokens are compared in constant time.**

### Added

- **Ten.** A blank line after it, as written.

- **Nested.** Detail.
  - A sub-point.

### Fixed

- **Ten fixed.**

## 0.9.0

### Fixed

- **Nine.**
"""


class Changelog(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.write("0.10.0/_intro.md", "The database moves to schema 3.\n")
        self.write("0.10.0/_sections", "security\nadded\nfixed\n")
        self.write("0.10.0/security/01-tokens.md", "- **Tokens are compared in constant time.**\n")
        self.write("0.10.0/added/01-ten.md", "- **Ten.** A blank line after it, as written.\n\n")
        self.write("0.10.0/added/02-nested.md", "- **Nested.** Detail.\n  - A sub-point.\n")
        self.write("0.10.0/fixed/01-ten-fixed.md", "- **Ten fixed.**\n")
        self.write("0.9.0/fixed/01-nine.md", "- **Nine.**\n")

    def tearDown(self):
        self.dir.cleanup()

    def write(self, path, text):
        path = self.root / "changelog.d" / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def run_script(self, *args):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root), *args],
            capture_output=True,
            text=True,
        )

    @property
    def changelog(self):
        return (self.root / "CHANGELOG.md").read_text()

    def test_released_history_is_written_as_it_is(self):
        self.assertEqual(self.run_script().returncode, 0)
        self.assertEqual(self.changelog, RELEASED)

    def test_unreleased_entries_go_first_by_pr_number_then_name(self):
        self.write("unreleased/fixed/902-later.md", "- **902.**\n")
        self.write("unreleased/fixed/one-without-a-number.md", "- **No number.**\n")
        self.write("unreleased/fixed/89-earlier.md", "- **89.**\n")
        self.write("unreleased/added/950.md", "- **950.**\n")
        self.run_script()
        self.assertEqual(
            self.changelog,
            RELEASED.replace(
                "# Changelog\n",
                "# Changelog\n\n## Unreleased\n\n### Added\n\n- **950.**\n\n"
                "### Fixed\n\n- **89.**\n- **902.**\n- **No number.**\n",
            ),
        )

    def test_stdout_prints_without_writing(self):
        result = self.run_script("--stdout")
        self.assertEqual(result.stdout, RELEASED)
        self.assertFalse((self.root / "CHANGELOG.md").exists())

    def test_writing_twice_changes_nothing(self):
        self.write("unreleased/added/1.md", "- **One.**\n")
        self.run_script()
        first = self.changelog
        self.assertEqual(self.run_script().returncode, 0)
        self.assertEqual(self.changelog, first)
        self.assertEqual(self.run_script("--check").returncode, 0)

    def test_check_fails_on_a_stale_changelog(self):
        self.run_script()
        self.write("unreleased/added/1.md", "- **One.**\n")
        result = self.run_script("--check")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not what changelog.d/ makes", result.stderr)
        (self.root / "CHANGELOG.md").write_text(RELEASED.replace("Nine", "Nein"))
        (self.root / "changelog.d/unreleased/added/1.md").unlink()
        self.assertNotEqual(self.run_script("--check").returncode, 0)

    def test_lint_ignores_unreleased_but_not_history(self):
        self.run_script()
        self.write("unreleased/added/1.md", "- **One.**\n")
        self.assertEqual(self.run_script("--lint").returncode, 0)
        (self.root / "CHANGELOG.md").write_text(self.changelog.replace("Nine", "Nein"))
        self.assertIn("released version", self.run_script("--lint").stderr)
        self.write("unreleased/added/2.md", "- **Two.**\n\n")
        self.assertIn("2.md", self.run_script("--lint").stderr)

    def test_a_release_takes_the_unreleased_fragments_in_order(self):
        self.write("unreleased/fixed/902-later.md", "- **902.**\n")
        self.write("unreleased/fixed/89-earlier.md", "- **89.**\n")
        self.write("unreleased/_intro.md", "Read this first.\n")
        self.run_script()
        unreleased = self.changelog
        self.assertEqual(self.run_script("--release", "0.11.0").returncode, 0)
        release = self.root / "changelog.d/0.11.0"
        self.assertEqual(
            sorted(str(p.relative_to(release)) for p in release.rglob("*.md")),
            ["_intro.md", "fixed/01-89-earlier.md", "fixed/02-902-later.md"],
        )
        self.assertFalse(any((self.root / "changelog.d/unreleased").rglob("*.md")))
        self.assertEqual(self.changelog, unreleased.replace("## Unreleased", "## 0.11.0"))
        self.assertTrue(self.changelog.endswith(RELEASED[len("# Changelog\n") :]))
        self.assertEqual(self.run_script("--check").returncode, 0)

    def test_malformed_fragments_are_refused(self):
        cases = {
            "unreleased/fixes/1.md": "- **Wrong section.**\n",
            "unreleased/fixed/2.md": "\n",
            "unreleased/fixed/3.md": "- **Trailing blank.**\n\n",
            "unreleased/fixed/4.md": "- **No newline.**",
            "unreleased/stray.md": "- **Not in a section.**\n",
            "0.8.0/fixed/unnumbered.md": "- **Released without a place.**\n",
            "unreleased/fixed/5.md": "Not a bullet\nsecond line\n",
            "unreleased/fixed/6.md": "- **Two lines.**\nmore\n",
            "unreleased/fixed/901-no-suffix": "- **Dropped without a suffix.**\n",
            "unreleased/_sections": "fixed\n",
            "0.8.0/_sections": "fixed\nfixed\n",
            "0.x/fixed/01-a.md": "- **Not a version.**\n",
        }
        for path, text in cases.items():
            with self.subTest(path=path):
                self.write(path, text)
                result = self.run_script("--check")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(path.split("/")[0], result.stderr)
                shutil.rmtree((self.root / "changelog.d" / path).parent)

    def test_two_released_fragments_with_one_number_are_refused(self):
        self.write("0.9.0/fixed/01-other.md", "- **Other.**\n")
        result = self.run_script("--check")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("numbered as", result.stderr)

    def test_dotfiles_are_ignored_and_left_where_they_are(self):
        self.write("unreleased/fixed/.DS_Store", "binary")
        self.write("unreleased/fixed/1.md", "- **One.**\n")
        self.write("0.10.0/added/.DS_Store", "binary")
        self.assertEqual(self.run_script("--release", "0.11.0").returncode, 0)
        self.assertTrue((self.root / "changelog.d/0.11.0/fixed/01-1.md").exists())
        self.assertTrue((self.root / "changelog.d/unreleased/fixed/.DS_Store").exists())

    def test_a_release_that_cannot_finish_moves_nothing(self):
        tree = self.root / "changelog.d"
        self.write("unreleased/fixed/1.md", "- **One.**\n")
        self.write("unreleased/added/2.md", "- **Two.**\n")
        for stray, text in [
            ("unreleased/fixed/901-y", "- **No suffix.**\n"),
            ("0.9.0/fixed/02-nine-again", "- **No suffix, released.**\n"),
        ]:
            with self.subTest(stray=stray):
                self.write(stray, text)
                before = sorted(p.relative_to(tree) for p in tree.rglob("*"))
                self.assertNotEqual(self.run_script("--release", "0.11.0").returncode, 0)
                self.assertEqual(sorted(p.relative_to(tree) for p in tree.rglob("*")), before)
                self.assertFalse((self.root / "CHANGELOG.md").exists())
                (tree / stray).unlink()

    def test_a_release_is_refused_twice_or_with_nothing_in_it(self):
        self.assertIn("nothing to release", self.run_script("--release", "0.11.0").stderr)
        self.write("unreleased/fixed/1.md", "- **One.**\n")
        self.assertIn("already released", self.run_script("--release", "0.10.0").stderr)
        self.assertIn("not a version", self.run_script("--release", "v0.11").stderr)
        self.assertTrue((self.root / "changelog.d/unreleased/fixed/1.md").exists())


if __name__ == "__main__":
    unittest.main()
