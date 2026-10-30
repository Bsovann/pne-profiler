# Releasing

## Versions

The next version is the `Version` of the last document in `changelog.yml`,
and only `Changelogs/fragments.py fold` sets it. Everything else is a copy
written by a tool, so don't edit any of them by hand:

- `[workspace.package] version` in `Cargo.toml`, which `--version` reports,
  is written by `fragments.py fold` along with `changelog.yml`.
- `Version:` in `RPM/pne-profiler.spec` is written by
  `RPM/update-changelog.py` at release time.
- The `vX.Y.Z` git tag is derived from `Cargo.toml` in step 6 of the release
  steps.

The rc branch is named `rc/next` and doesn't carry a version, so it can't
disagree with the one that ships.

Each fold takes the previous release's `Version` and bumps it by the most
significant section the release in progress holds:

| Section           | Before 1.0 (0.4.2 becomes) | From 1.0 on (1.4.2 becomes) |
|-------------------|----------------------------|-----------------------------|
| `Major_Changes`   | minor: 0.5.0               | major: 2.0.0                |
| `Minor_Changes`   | minor: 0.5.0               | minor: 1.5.0                |
| `Bug_Fixes`       | patch: 0.4.3               | patch: 1.4.3                |
| `Trivial_Changes` | none: 0.4.2                | none: 1.4.2                 |

The bump is computed from the previous release, not from the previous fold,
so folding three minor tickets into one release still gives 0.5.0. The first
release keeps the `Version` it was given.

- **Before 1.0**, a breaking change bumps the minor version, as Cargo's SemVer
  rules do for 0.x. Reaching 1.0.0 is a decision, not a side effect of a
  fragment: run `Changelogs/fragments.py fold --stable` on the rc branch.
  It sets the release in progress to 1.0.0, and later folds into that
  release keep it.
- **A release with only `Trivial_Changes`** would reuse the previous version.
  Fold warns, and `fragments.py check` and `RPM/update-changelog.py` refuse a
  dated release whose `Version` was already released. Keep the release open
  (`Release_Date: TBD`) until it has a `Bug_Fixes`, `Minor_Changes` or
  `Major_Changes` entry.
- **rc builds** report the version of the release in progress, the same as
  the final release. Include the commit (`git rev-parse --short HEAD`) in bug
  reports from rc builds. We don't use pre-release versions such as
  `0.3.0-rc.1`: `rc.N` would be another number to bump by hand and keep in
  sync with the RPM's `0.3.0~rc.1`, and it still wouldn't identify the commit.

## Release steps

`Changelogs/ChangeFragments/changelog.yml` keeps every release's changes,
one YAML document per release, oldest first. The last document is the
release in progress:

```yaml
---
Version: 0.1.0
Release_Date: 2026-10-30
Minor_Changes:
  - (RPM) - added an RPM spec for packaging the pne-profiler binary.
---
Version: 0.2.0
Release_Date: TBD
Minor_Changes:
  - (pne-perf) - added hardware counter sampling.
```

A release ships the last document. Run these steps on the rc branch
(`rc/next`) after every ticket PR for the release has been merged into it.
Commit messages still need the `SCRUM-<n>: ` prefix (see
[Contribution](README.md#contribution)), so use the ticket that tracks the
release.

1. Fold any remaining ticket fragments and commit the result. Folding sets
   `Version` in both `changelog.yml` and `Cargo.toml`, so they already match.

   ```bash
   git switch rc/next && git pull
   Changelogs/fragments.py fold --dry-run
   Changelogs/fragments.py fold
   cargo build                         # updates Cargo.lock to the new version
   git add Changelogs/ChangeFragments/ Cargo.toml Cargo.lock
   git commit -m "SCRUM-<n>: folded changelog fragments for X.Y.Z"
   ```

2. In `changelog.yml`, replace the last document's `Release_Date: TBD` with
   today's date as `YYYY-MM-DD`, e.g. `Release_Date: 2026-10-03`.

3. Generate the RPM `%changelog` entry and check the spec:

   ```bash
   RPM/update-changelog.py --dry-run   # preview the entry
   RPM/update-changelog.py             # write RPM/pne-profiler.spec
   git diff RPM/pne-profiler.spec
   ```

   The script sets `Version` in `RPM/pne-profiler.spec`, resets `Release` to
   `1`, and prepends a `%changelog` entry built from the last release's
   sections. It uses your git `user.name` and `user.email` as the packager,
   so set those to the identity the RPM should carry.

   Only entries for the shipped code go into the RPM: those whose component
   is a crate, such as `(pne-cli)` or `(pne-perf)`. Tooling entries like
   `(CI)`, `(RPM)`, `(Changelog)`, `(Licensing)` or `(Tests)` stay in
   `changelog.yml` only. If a release has no crate entries, the RPM entry
   reads "No changes to the packaged software."

4. Commit and push the release, then wait for CI to pass on the rc branch:

   ```bash
   git add Changelogs/ChangeFragments/changelog.yml RPM/pne-profiler.spec
   git commit -m "SCRUM-<n>: release X.Y.Z"
   git push origin rc/next
   ```

5. Open a PR from the rc branch into `master` and merge it once CI is green.
   The changelog check only runs on PRs into `rc/**`, so this PR needs no
   fragment.

   ```bash
   gh pr create --base master --head rc/next --title "Release X.Y.Z"
   ```

6. Tag the merge on `master`. The tag name comes from `Cargo.toml`, so it
   can't differ from the version the binary reports. If `master` isn't up to
   date, the version is the previous release's, and `git tag` fails because
   that tag already exists.

   ```bash
   git switch master && git pull
   version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["workspace"]["package"]["version"])')
   git tag -a "v$version" -m "pne-profiler $version"
   git push origin "v$version"
   ```

7. Build the RPM from `RPM/pne-profiler.spec` if you're publishing a package.

`RPM/update-changelog.py` refuses to run when:

- Ticket fragments are still waiting to be folded.
- `Release_Date` is missing or still `TBD`.
- `Version` was already released, i.e. the release has only `Trivial_Changes`.
- `Version` doesn't match `Cargo.toml`. An RPM pre-release such as
  `0.1.0~rc1` matches Cargo's `0.1.0-rc1`.
- The spec already has an entry for that version-release.

Any `%` in an entry is written as `%%` so RPM doesn't expand it as a macro.
The script needs Python 3.11+ and PyYAML (Fedora: `python3-pyyaml`).

For a packaging-only change (same code, spec fix), bump `Release` in the spec
and add the `%changelog` entry by hand.

## After a release

`rc/next` is long-lived. After the release PR is merged, bring it up to
`master` so the next release starts from the release commit; create it from
`master` the first time:

```bash
git switch rc/next && git pull
git merge --ff-only origin/master
git push origin rc/next
```

Don't delete and recreate it, because GitHub closes open PRs whose base
branch is deleted. Leave `Cargo.toml` alone too: the first fold after a
release starts a new `Release_Date: TBD` document and sets the version from
the tickets folded into it.
