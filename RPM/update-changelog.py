#!/usr/bin/env python3
"""Generate the RPM %changelog entry for a release from changelog.yml.

Run at release time, after filling in Release_Date in changelog.yml:

    RPM/update-changelog.py            # update RPM/pne-profiler.spec
    RPM/update-changelog.py --dry-run  # print the entry only

It sets the spec's Version from the last release in changelog.yml, resets
Release to 1, and prepends one %changelog entry built from its sections. Only
entries for the shipped code, components named after a pne-* crate such as
(pne-cli), go into the RPM; tooling entries like (CI), (RPM), (Changelog) or
(Licensing) stay in changelog.yml only. It refuses to run if ticket fragments haven't been folded in yet
(Changelogs/fragments.py fold), if Release_Date is unset, if Version disagrees
with Cargo.toml, or if the spec already has an entry for that version-release.

Requires PyYAML (Fedora: python3-pyyaml).
"""

import argparse
import datetime
import re
import subprocess
import sys
import tomllib
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
FRAGMENTS = ROOT / "Changelogs" / "ChangeFragments"
CHANGELOG = FRAGMENTS / "changelog.yml"
TEMPLATE = FRAGMENTS / "PG-0000-Template.yml"
SPEC = ROOT / "RPM" / "pne-profiler.spec"
CARGO = ROOT / "Cargo.toml"

# Same keys as Changelogs/fragments.py, most significant first.
SECTIONS = ["Major_Changes", "Minor_Changes", "Bug_Fixes", "Trivial_Changes"]
# Entries whose component is a crate under crates/ change what the RPM ships.
PRODUCT_ENTRY = re.compile(r"^\(pne-[a-z0-9-]+\) - ")
# An RPM %changelog entry needs at least one line.
NO_PRODUCT_CHANGES = "No changes to the packaged software."


def fail(msg):
    sys.exit(f"error: {msg}")


def git_config(key):
    out = subprocess.run(
        ["git", "config", key], cwd=ROOT, capture_output=True, text=True
    )
    return out.stdout.strip()


def load_changelog():
    # changelog.yml holds one document per release, oldest first; release the last.
    try:
        releases = [r for r in yaml.safe_load_all(CHANGELOG.read_text()) if r is not None]
    except yaml.YAMLError as e:
        fail(f"{CHANGELOG}: invalid YAML: {e}")
    if not releases:
        fail(f"{CHANGELOG} has no release documents")
    data = releases[-1]
    if not isinstance(data, dict):
        fail(f"{CHANGELOG}: the last release is not a YAML mapping")

    version = str(data.get("Version", "")).strip()
    if not version:
        fail(f"{CHANGELOG}: Version is missing")
    if "-" in version:
        fail(f"{CHANGELOG}: RPM versions cannot contain '-': {version}")

    raw_date = data.get("Release_Date")
    if isinstance(raw_date, datetime.date):
        date = raw_date
    else:
        try:
            date = datetime.date.fromisoformat(str(raw_date).strip())
        except ValueError:
            fail(f"{CHANGELOG}: set Release_Date to YYYY-MM-DD (got {raw_date!r})")

    entries = []
    for section in SECTIONS:
        entries.extend(str(e).strip() for e in data.get(section) or [])
    if not entries:
        fail(f"{CHANGELOG} has no changelog entries")

    return version, date, entries


def check_cargo_version(version):
    cargo = tomllib.loads(CARGO.read_text())["workspace"]["package"]["version"]
    # An RPM pre-release like 0.1.0~rc1 corresponds to Cargo's 0.1.0-rc1.
    if version.replace("~", "-") != cargo:
        fail(
            f"changelog.yml Version {version} does not match Cargo.toml "
            f"version {cargo}"
        )


def render_entry(version, release, date, entries, packager):
    header = f"* {date.strftime('%a %b %d %Y')} {packager} - {version}-{release}"
    # A bare % in %changelog would be expanded as a macro.
    lines = [f"- {e.replace('%', '%%')}" for e in entries]
    return "\n".join([header, *lines]) + "\n"


def update_spec(spec, version, release, entry):
    spec, n = re.subn(r"(?m)^(Version:\s*).*$", rf"\g<1>{version}", spec)
    if n != 1:
        fail(f"{SPEC}: expected one Version: line")
    spec, n = re.subn(
        r"(?m)^(Release:\s*)\S+?(%\{\?dist\})?$", rf"\g<1>{release}\g<2>", spec
    )
    if n != 1:
        fail(f"{SPEC}: expected one Release: line")
    spec, n = re.subn(r"(?m)^%changelog\n", lambda m: m.group(0) + entry + "\n", spec)
    if n != 1:
        fail(f"{SPEC}: expected one %changelog section")
    return spec


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--dry-run", action="store_true", help="print the entry, change nothing"
    )
    args = parser.parse_args()

    unfolded = sorted(
        p.name for p in FRAGMENTS.glob("*.yml") if p not in (CHANGELOG, TEMPLATE)
    )
    if unfolded:
        fail(
            f"fold these fragments first (Changelogs/fragments.py fold): "
            f"{', '.join(unfolded)}"
        )

    version, date, entries = load_changelog()
    check_cargo_version(version)

    product = [e for e in entries if PRODUCT_ENTRY.match(e)]
    skipped = len(entries) - len(product)
    if skipped:
        print(f"skipping {skipped} tooling entries (not pne-*)", file=sys.stderr)
    if not product:
        print(f"no pne-* entries; using {NO_PRODUCT_CHANGES!r}", file=sys.stderr)
        product = [NO_PRODUCT_CHANGES]

    release = 1
    spec = SPEC.read_text()
    if re.search(rf"(?m)^\*.* - {re.escape(version)}-{release}$", spec):
        fail(f"{SPEC} already has a %changelog entry for {version}-{release}")

    name, email = git_config("user.name"), git_config("user.email")
    if not name or not email:
        fail("set git user.name and user.email; they are used as the packager")

    entry = render_entry(version, release, date, product, f"{name} <{email}>")
    if args.dry_run:
        print(entry, end="")
        return

    SPEC.write_text(update_spec(spec, version, release, entry))
    print(f"updated {SPEC.relative_to(ROOT)} for {version}-{release}")


if __name__ == "__main__":
    main()
