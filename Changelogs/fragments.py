#!/usr/bin/env python3
"""Validate and fold per-ticket changelog fragments.

Each ticket branch adds one fragment, Changelogs/ChangeFragments/<JIRA-KEY>.yml,
copied from PG-0000-Template.yml. The reviewer folds approved fragments into
changelog.yml:

    Changelogs/fragments.py check            # validate fragments and changelog.yml (CI)
    Changelogs/fragments.py fold --dry-run   # show what would be folded
    Changelogs/fragments.py fold             # append to changelog.yml, delete fragments
    Changelogs/fragments.py fold --stable    # same, and make this release 1.0.0

changelog.yml holds one YAML document per release, oldest first. Every release
but the last has a Release_Date; the last is the one in progress, with
Release_Date: TBD. Fragments fold into that last release. Once it has a date
(it was released), the next fold starts a new release document after it.

Folding also sets Version in changelog.yml and Cargo.toml: the previous
release's Version bumped by the most significant section the current release
has. Major_Changes bumps X.0.0, Minor_Changes 0.X.0, Bug_Fixes 0.0.X, and
Trivial_Changes alone bumps nothing. While the major version is 0,
Major_Changes bumps 0.X.0 like Minor_Changes; 1.0.0 is reached only with
--stable, and later folds keep it. For the first release, Version is left as
set. A Version that reuses an already-released one fails check. The version
rules and the release steps are in RELEASE.md.

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
STABLE = "1.0.0"

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


def parse_version(version):
    m = SEMVER.match(version)
    if not m:
        raise ValueError(f"{CHANGELOG.name}: Version {version!r} is not X.Y.Z")
    return tuple(map(int, m.groups()))


def check_versions(releases):
    """Each released Version must be greater than the release before it."""
    errors = []
    for i in range(1, len(releases)):
        if not is_released(releases[i]):
            continue
        previous = str(releases[i - 1].get("Version", ""))
        version = str(releases[i].get("Version", ""))
        name = f"{CHANGELOG.name}: release #{i + 1} ({version})"
        try:
            older, newer = parse_version(previous), parse_version(version)
        except ValueError as e:
            errors.append(str(e))
            continue
        if newer == older:
            errors.append(
                f"{name}: {version} was already released (release #{i}). A release "
                "with only Trivial_Changes can't ship: set Release_Date back to "
                f"{UNRELEASED} and keep the release open until it has a Bug_Fixes, "
                "Minor_Changes or Major_Changes entry"
            )
        elif newer < older:
            errors.append(f"{name}: lower than the previous release, {previous}")
    return errors


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
    return errors + check_versions(releases)


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


def next_version(previous, current, stable=False):
    """The Version of the release in progress, `current`.

    `previous` is the last released Version, bumped by the most significant
    section with entries in `current`, or None for the first release, which
    keeps the Version it has. While the major version is 0, Major_Changes bumps
    the minor version: 1.0.0 needs `stable`, and a release already at 1.0.0
    keeps it.
    """
    if previous is None:
        if stable:
            raise ValueError("--stable needs a previous release to bump from")
        return str(current["Version"])
    major, minor, patch = parse_version(previous)
    if major == 0:
        if stable or str(current.get("Version")) == STABLE:
            return STABLE
        if current.get("Major_Changes") or current.get("Minor_Changes"):
            return f"0.{minor + 1}.0"
    else:
        if stable:
            raise ValueError(f"--stable only applies before 1.0.0, not after {previous}")
        if current.get("Major_Changes"):
            return f"{major + 1}.0.0"
        if current.get("Minor_Changes"):
            return f"{major}.{minor + 1}.0"
    if current.get("Bug_Fixes"):
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
    if not paths and not args.stable:
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
    previous = str(releases[-2]["Version"]) if len(releases) > 1 else None
    try:
        version = next_version(previous, current, stable=args.stable)
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    if previous is None:
        version_note = f"Version: {version} (first release, left as set)"
    else:
        version_note = f"Version: {old_version} -> {version} (previous release {previous})"
        if version == previous:
            version_note += (
                f"\nwarning: only Trivial_Changes since {previous}, so no bump. This "
                "release can't ship until it has a Bug_Fixes, Minor_Changes or "
                "Major_Changes entry"
            )
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
    fold.add_argument(
        "--stable", action="store_true", help="make this release 1.0.0 (only from 0.x)"
    )
    args = parser.parse_args()
    return {"check": cmd_check, "fold": cmd_fold}[args.command](args)


if __name__ == "__main__":
    sys.exit(main())
