#!/usr/bin/env python3
"""CHANGELOG.md, generated from the fragments in changelog.d/.

    changelog.d/unreleased/<section>/<pr-or-slug>.md   entries not yet released
    changelog.d/<version>/<section>/<nn>-<slug>.md      a release, in order
    changelog.d/<version>/_intro.md                     prose before its sections
    changelog.d/<version>/_sections                     its section order, where
                                                        it is not SECTIONS' order

A fragment is one entry, or one paragraph or heading between entries, and
the changelog is its fragments written out in order, as they are: a blank
line after an entry is part of that entry's file. An unreleased entry is a
single `- **Lead.** Detail.` line ending in one newline, and they are ordered
by the PR number their name starts with, then by name; a release keeps the
order it was cut in.

    changelog.py              write CHANGELOG.md
    changelog.py --check      fail if CHANGELOG.md is not what the fragments make
    changelog.py --lint       fail on a malformed fragment, or a released version
                              in CHANGELOG.md that the fragments do not make
    changelog.py --release V  move the unreleased fragments to V, then write
    changelog.py --stdout     print it instead of writing it
"""

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Every section a release may have, by directory, in the order they are
# written unless a release's `_sections` says otherwise.
SECTIONS = {
    "breaking": "Breaking",
    "added": "Added",
    "security": "Security",
    "changed": "Changed",
    "removed": "Removed",
    "fixed": "Fixed",
    "tests": "Tests",
    "known-issues": "Known issues",
    "internal": "Internal",
}
HEADER = "# Changelog\n"
VERSION = re.compile(r"^\d+\.\d+\.\d+$")


class Malformed(Exception):
    pass


ENTRY = re.compile(r"- \S.*\n")


def read(path, unreleased):
    text = path.read_text(encoding="utf-8")
    if not text.strip():
        raise Malformed(f"{path}: empty")
    if not text.endswith("\n"):
        raise Malformed(f"{path}: must end with a newline")
    if unreleased and not ENTRY.fullmatch(text):
        raise Malformed(f"{path}: an unreleased fragment is one `- **Lead.** Detail.` line")
    return text


def listed(directory):
    """A directory's entries, less dotfiles such as .DS_Store."""
    return [p for p in directory.iterdir() if not p.name.startswith(".")]


def unreleased_key(path):
    number = re.match(r"\d+", path.name)
    return (int(number.group()) if number else float("inf"), path.name)


def fragments_in(section, unreleased):
    """A section directory's fragments, in order."""
    files = listed(section)
    for path in files:
        if not path.is_file() or path.suffix != ".md":
            raise Malformed(f"{path}: a fragment is a .md file")
    if unreleased:
        return sorted(files, key=unreleased_key)
    numbered = {}
    for path in files:
        number = re.match(r"(\d+)-", path.name)
        if not number:
            raise Malformed(f"{path}: a released fragment is named <nn>-<slug>.md")
        if int(number.group(1)) in numbered:
            raise Malformed(f"{path}: numbered as {numbered[int(number.group(1))].name} is")
        numbered[int(number.group(1))] = path
    return [numbered[n] for n in sorted(numbered)]


def release(directory, unreleased):
    """A release's intro and sections, in order, each a list of its
    fragments' text."""
    order = list(SECTIONS)
    if (directory / "_sections").exists():
        if unreleased:
            raise Malformed(f"{directory}/_sections: unreleased sections go in the usual order")
        order = (directory / "_sections").read_text(encoding="utf-8").split()
        if len(set(order)) != len(order):
            raise Malformed(f"{directory}/_sections: a section is listed twice")
    present = {p.name for p in listed(directory) if p.is_dir()}
    for name in present | set(order):
        if name not in SECTIONS:
            raise Malformed(f"{directory / name}: not a section ({', '.join(SECTIONS)})")
    for name in present - set(order):
        raise Malformed(f"{directory}/_sections: {name} is missing")
    for path in listed(directory):
        if path.is_file() and path.name not in ("_intro.md", "_sections"):
            raise Malformed(f"{path}: fragments go in a section directory")
    sections = []
    for name in order:
        if name in present:
            files = fragments_in(directory / name, unreleased)
            if files:
                sections.append((SECTIONS[name], [read(p, unreleased) for p in files]))
    intro = directory / "_intro.md"
    return (read(intro, False) if intro.exists() else None), sections


def render(fragments):
    versions = sorted(
        (p for p in fragments.iterdir() if p.is_dir() and VERSION.match(p.name)),
        key=lambda p: tuple(int(n) for n in p.name.split(".")),
        reverse=True,
    )
    for path in fragments.iterdir():
        if path.is_dir() and path.name != "unreleased" and not VERSION.match(path.name):
            raise Malformed(f"{path}: neither unreleased nor a version")
    out = HEADER
    blocks = [("Unreleased", fragments / "unreleased", True)] if (fragments / "unreleased").is_dir() else []
    blocks += [(p.name, p, False) for p in versions]
    for title, directory, unreleased in blocks:
        intro, sections = release(directory, unreleased)
        if unreleased and not intro and not sections:
            continue
        out += f"\n## {title}\n"
        if intro:
            out += f"\n{intro}"
        for heading, chunks in sections:
            out += f"\n### {heading}\n\n" + "".join(chunks)
    return out


def released(text):
    """The changelog without its Unreleased block."""
    return re.sub(r"\n## Unreleased\n.*?(?=\n## |\Z)", "", text, flags=re.S)


def cut(fragments, version):
    """Move the unreleased fragments into `version`, numbered in order.
    Everything is checked before anything moves."""
    if not VERSION.match(version):
        raise Malformed(f"{version}: not a version")
    source, target = fragments / "unreleased", fragments / version
    if target.exists():
        raise Malformed(f"{target}: already released")
    if not source.is_dir() or not any(source.rglob("*.md")):
        raise Malformed(f"{source}: nothing to release")
    render(fragments)
    moves = []
    if (source / "_intro.md").exists():
        moves.append((source / "_intro.md", target / "_intro.md"))
    for name in SECTIONS:
        if (source / name).is_dir():
            files = fragments_in(source / name, True)
            width = max(2, len(str(len(files))))
            moves += [(p, target / name / f"{i:0{width}}-{p.name}") for i, p in enumerate(files, 1)]
    for old, new in moves:
        new.parent.mkdir(parents=True, exist_ok=True)
        old.rename(new)
    for name in SECTIONS:
        if (source / name).is_dir() and not any((source / name).iterdir()):
            (source / name).rmdir()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--check", action="store_true")
    action.add_argument("--lint", action="store_true")
    action.add_argument("--stdout", action="store_true")
    action.add_argument("--release", metavar="VERSION")
    parser.add_argument("--root", type=Path, default=ROOT, help=argparse.SUPPRESS)
    args = parser.parse_args()
    fragments, changelog = args.root / "changelog.d", args.root / "CHANGELOG.md"
    try:
        if args.release:
            cut(fragments, args.release)
        text = render(fragments)
    except Malformed as e:
        sys.exit(f"changelog: {e}")
    if args.stdout:
        sys.stdout.write(text)
        return
    if args.lint:
        if not changelog.exists() or released(changelog.read_text(encoding="utf-8")) != released(text):
            sys.exit("changelog: a released version in CHANGELOG.md is not what changelog.d/ makes; edit its fragments instead")
        return
    if args.check:
        if not changelog.exists() or changelog.read_text(encoding="utf-8") != text:
            sys.exit("changelog: CHANGELOG.md is not what changelog.d/ makes; run scripts/changelog.py")
        return
    if not changelog.exists() or changelog.read_text(encoding="utf-8") != text:
        changelog.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
