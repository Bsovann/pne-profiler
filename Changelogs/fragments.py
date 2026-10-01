#!/usr/bin/env python3
"""Validate and fold per-ticket changelog fragments.

Each ticket branch adds one fragment, Changelogs/ChangeFragments/<JIRA-KEY>.yml,
copied from PG-0000-Template.yml. The reviewer folds approved fragments into
the release candidate's changelog.yml:

    Changelogs/fragments.py check            # validate fragments and changelog.yml (CI)
    Changelogs/fragments.py fold --dry-run   # show what would be folded
    Changelogs/fragments.py fold             # append to changelog.yml, delete fragments

Requires PyYAML (Fedora: python3-pyyaml).
"""

import argparse
import re
import sys
from pathlib import Path

import yaml

DIR = Path(__file__).resolve().parent / "ChangeFragments"
TEMPLATE = DIR / "PG-0000-Template.yml"
CHANGELOG = DIR / "changelog.yml"

# Same order as the template. RPM/update-changelog.py keeps its own copy.
SECTIONS = ["Trivial_Changes", "Major_Changes", "Minor_Changes", "Bug_Fixes"]
CHANGELOG_KEYS = ["Version", "Release_Date"]

# Jira issue keys look like PROJECT-123.
FRAGMENT_NAME = re.compile(r"^[A-Z][A-Z0-9]+-\d+\.yml$")
# (component) - (verb past tense) + (change description)
ENTRY = re.compile(r"^\([^()]+\) - \S.*$")


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


def check_sections(path, data, extra_keys=()):
    errors = []
    for key, value in data.items():
        if key in extra_keys:
            continue
        if key not in SECTIONS:
            errors.append(
                f"{path.name}: unknown key {key!r} (expected one of {', '.join(SECTIONS)})"
            )
            continue
        if not isinstance(value, list) or not value:
            errors.append(f"{path.name}: {key} must be a non-empty list")
            continue
        for entry in value:
            if isinstance(entry, dict):
                errors.append(
                    f"{path.name}: {key}: an entry containing ': ' must be quoted"
                )
            elif not isinstance(entry, str) or not ENTRY.match(entry.strip()):
                errors.append(
                    f"{path.name}: {key}: {entry!r} does not match "
                    "'(component) - (verb past tense) (description)'"
                )
            elif entry.strip().startswith("(component)"):
                errors.append(f"{path.name}: {key}: template placeholder left in")
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
    return errors + check_sections(path, data)


def check_changelog():
    data, errors = load(CHANGELOG)
    if errors:
        return errors
    for key in CHANGELOG_KEYS:
        if key not in data:
            errors.append(f"{CHANGELOG.name}: missing {key}")
    return errors + check_sections(CHANGELOG, data, extra_keys=CHANGELOG_KEYS)


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


class _IndentedDumper(yaml.SafeDumper):
    # Indent list items under their key, matching the template's style.
    def increase_indent(self, flow=False, indentless=False):
        return super().increase_indent(flow, False)


def dump_changelog(data):
    # Release_Date is written back unquoted whether it is TBD or a date.
    ordered = {k: data[k] for k in CHANGELOG_KEYS if k in data}
    ordered.update((s, data[s]) for s in SECTIONS if data.get(s))
    return yaml.dump(
        ordered,
        Dumper=_IndentedDumper,
        explicit_start=True,
        sort_keys=False,
        width=1000,
        allow_unicode=True,
    )


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

    data, _ = load(CHANGELOG)
    for path in paths:
        fragment, _ = load(path)
        for section in SECTIONS:
            data.setdefault(section, []).extend(e.strip() for e in fragment.get(section) or [])
        print(f"{'would fold' if args.dry_run else 'folded'} {path.name}")

    output = dump_changelog(data)
    if args.dry_run:
        print(f"\n{CHANGELOG.name} would become:\n{output}", end="")
        return 0

    CHANGELOG.write_text(output)
    for path in paths:
        path.unlink()
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
