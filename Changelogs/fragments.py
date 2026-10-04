#!/usr/bin/env python3
"""Validate and fold per-ticket changelog fragments.

Each ticket branch adds one fragment, Changelogs/ChangeFragments/<JIRA-KEY>.yml,
copied from PG-0000-Template.yml. The reviewer folds approved fragments into
changelog.yml:

    Changelogs/fragments.py check            # validate fragments and changelog.yml (CI)
    Changelogs/fragments.py fold --dry-run   # show what would be folded
    Changelogs/fragments.py fold             # append to changelog.yml, delete fragments

changelog.yml holds one YAML document per release, oldest first. Every release
but the last has a Release_Date; the last is the one in progress, with
Release_Date: TBD. Fragments fold into that last release. Once it has a date
(it was released), the next fold starts a new release document after it.

Folding also sets Version in changelog.yml and Cargo.toml: the previous
release's Version bumped by the most significant section the current release
has. Major_Changes bumps X.0.0, Minor_Changes 0.X.0, Bug_Fixes 0.0.X, and
Trivial_Changes alone bumps nothing. For the first release, Version is left
as set.

Requires PyYAML (Fedora: python3-pyyaml).
"""

import argparse
import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
DIR = ROOT / "Changelogs" / "ChangeFragments"
TEMPLATE = DIR / "PG-0000-Template.yml"
CHANGELOG = DIR / "changelog.yml"
CARGO = ROOT / "Cargo.toml"

# Same order as the template. RPM/update-changelog.py keeps its own copy.
SECTIONS = ["Trivial_Changes", "Major_Changes", "Minor_Changes", "Bug_Fixes"]
CHANGELOG_KEYS = ["Version", "Release_Date"]
UNRELEASED = "TBD"

# Jira issue keys look like PROJECT-123.
FRAGMENT_NAME = re.compile(r"^[A-Z][A-Z0-9]+-\d+\.yml$")
# (component) - (verb past tense) + (change description)
ENTRY = re.compile(r"^\([^()]+\) - \S.*$")
# The X.Y.Z at the start of a version; RPM pre-releases add e.g. ~rc1.
SEMVER = re.compile(r"^(\d+)\.(\d+)\.(\d+)")
# A "---" line starts each release document in changelog.yml.
DOC_START = re.compile(r"(?m)^---[ \t]*\n?")


def fragments():
    return sorted(p for p in DIR.glob("*.yml") if p not in (TEMPLATE, CHANGELOG))


def load(path):
    try:
        data = yaml.safe_load(path.read_text())
    except yaml.YAMLError as e:
        return None, [f"{path.name}: invalid YAML: {e}"]
    if not isinstance(data, dict):
        return None, [f"{path.name}: expected a mapping of sections"]
    return data, []


def split_releases(text):
    """The text of each release document in changelog.yml, oldest first."""
    return [doc for doc in DOC_START.split(text) if doc.strip()]


def load_releases():
    """Parse changelog.yml into (texts, releases), or return ([], [], errors)."""
    texts = split_releases(CHANGELOG.read_text())
    if not texts:
        return [], [], [f"{CHANGELOG.name}: has no release documents"]
    releases = []
    for i, text in enumerate(texts, 1):
        try:
            data = yaml.safe_load(text)
        except yaml.YAMLError as e:
            return [], [], [f"{CHANGELOG.name}: release #{i}: invalid YAML: {e}"]
        if not isinstance(data, dict):
            return [], [], [f"{CHANGELOG.name}: release #{i}: expected a mapping"]
        releases.append(data)
    return texts, releases, []


def is_released(release):
    return str(release.get("Release_Date", UNRELEASED)).strip() != UNRELEASED


def check_sections(name, data, extra_keys=()):
    errors = []
    for key, value in data.items():
        if key in extra_keys:
            continue
        if key not in SECTIONS:
            errors.append(
                f"{name}: unknown key {key!r} (expected one of {', '.join(SECTIONS)})"
            )
            continue
        if not isinstance(value, list) or not value:
            errors.append(f"{name}: {key} must be a non-empty list")
            continue
        for entry in value:
            if isinstance(entry, dict):
                errors.append(f"{name}: {key}: an entry containing ': ' must be quoted")
            elif not isinstance(entry, str) or not ENTRY.match(entry.strip()):
                errors.append(
                    f"{name}: {key}: {entry!r} does not match "
                    "'(component) - (verb past tense) (description)'"
                )
            elif entry.strip().startswith("(component)"):
                errors.append(f"{name}: {key}: template placeholder left in")
    return errors


def check_fragment(path):
    errors = []
    if not FRAGMENT_NAME.match(path.name):
        errors.append(f"{path.name}: name it after the Jira ticket, e.g. SCRUM-10.yml")
    data, load_errors = load(path)
    if load_errors:
        return errors + load_errors
    if not any(k in SECTIONS for k in data):
        errors.append(f"{path.name}: has no changelog sections")
    return errors + check_sections(path.name, data)


def check_changelog():
    _, releases, errors = load_releases()
    for i, release in enumerate(releases, 1):
        name = f"{CHANGELOG.name}: release #{i} ({release.get('Version', '?')})"
        for key in CHANGELOG_KEYS:
            if key not in release:
                errors.append(f"{name}: missing {key}")
        if i < len(releases) and not is_released(release):
            errors.append(f"{name}: only the last release may have Release_Date: TBD")
        errors += check_sections(name, release, extra_keys=CHANGELOG_KEYS)
    return errors


def cmd_check(_args):
    errors = check_changelog()
    for path in fragments():
        errors += check_fragment(path)
    for e in errors:
        print(f"error: {e}", file=sys.stderr)
    if errors:
        return 1
    print(f"ok: {CHANGELOG.name} and {len(fragments())} fragment(s)")
    return 0


def next_version(previous, data):
    """Bump `previous` by the most significant section with entries in `data`."""
    m = SEMVER.match(previous)
    if not m:
        raise ValueError(f"{CHANGELOG.name}: previous Version {previous!r} is not X.Y.Z")
    major, minor, patch = map(int, m.groups())
    if data.get("Major_Changes"):
        return f"{major + 1}.0.0"
    if data.get("Minor_Changes"):
        return f"{major}.{minor + 1}.0"
    if data.get("Bug_Fixes"):
        return f"{major}.{minor}.{patch + 1}"
    return previous


def set_cargo_version(version):
    lines = CARGO.read_text().splitlines(keepends=True)
    section = None
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("["):
            section = stripped
        elif section == "[workspace.package]" and re.match(r"version\s*=", stripped):
            lines[i] = f'version = "{version}"\n'
            CARGO.write_text("".join(lines))
            return
    raise ValueError(f"{CARGO.name}: no version in [workspace.package]")


class _IndentedDumper(yaml.SafeDumper):
    # Indent list items under their key, matching the template's style.
    def increase_indent(self, flow=False, indentless=False):
        return super().increase_indent(flow, False)


def dump_release(data):
    # Release_Date is written back unquoted whether it is TBD or a date.
    ordered = {k: data[k] for k in CHANGELOG_KEYS if k in data}
    ordered.update((s, data[s]) for s in SECTIONS if data.get(s))
    return yaml.dump(
        ordered,
        Dumper=_IndentedDumper,
        sort_keys=False,
        width=1000,
        allow_unicode=True,
    )


def join_releases(texts):
    return "".join(f"---\n{text.rstrip()}\n" for text in texts)


def cmd_fold(args):
    paths = fragments()
    if not paths:
        print("nothing to fold")
        return 0

    errors = check_changelog()
    for path in paths:
        errors += check_fragment(path)
    if errors:
        for e in errors:
            print(f"error: {e}", file=sys.stderr)
        print("fix the errors above before folding", file=sys.stderr)
        return 1

    # Released documents are kept as written; only the last one is rewritten.
    texts, releases, _ = load_releases()
    started = is_released(releases[-1])
    if started:
        releases.append({"Version": str(releases[-1]["Version"]), "Release_Date": UNRELEASED})
        texts.append("")
    current = releases[-1]

    for path in paths:
        fragment, _ = load(path)
        for section in SECTIONS:
            current.setdefault(section, []).extend(
                e.strip() for e in fragment.get(section) or []
            )

    old_version = str(current["Version"])
    if len(releases) == 1:
        version = old_version
        version_note = f"Version: {version} (first release, left as set)"
    else:
        previous = str(releases[-2]["Version"])
        try:
            version = next_version(previous, current)
        except ValueError as e:
            print(f"error: {e}", file=sys.stderr)
            return 1
        version_note = f"Version: {old_version} -> {version} (previous release {previous})"
        if version == previous:
            version_note += "\nnote: only Trivial_Changes since the previous release, so no bump"
    current["Version"] = version
    texts[-1] = dump_release(current)

    if not args.dry_run:
        try:
            set_cargo_version(version)
        except ValueError as e:
            print(f"error: {e}", file=sys.stderr)
            return 1
        CHANGELOG.write_text(join_releases(texts))
        for path in paths:
            path.unlink()

    if started:
        print(f"{releases[-2]['Version']} is released; started a new release after it")
    for path in paths:
        print(f"{'would fold' if args.dry_run else 'folded'} {path.name}")
    print(version_note)
    if args.dry_run:
        print(f"\nthe current release would become:\n---\n{texts[-1]}", end="")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("check", help="validate fragments and changelog.yml")
    fold = sub.add_parser("fold", help="fold fragments into changelog.yml")
    fold.add_argument("--dry-run", action="store_true", help="print, change nothing")
    args = parser.parse_args()
    return {"check": cmd_check, "fold": cmd_fold}[args.command](args)


if __name__ == "__main__":
    sys.exit(main())
