# pne-profiler

A performance and energy profiler for parallel workloads on Linux. It reports
hardware counters, cache behaviour, memory bandwidth, roofline position, and
energy consumed (RAPL, GPU power APIs), with energy treated as a first-class
metric.

Status: early development. [![CI](https://github.com/Bsovann/pne-profiler/actions/workflows/ci.yml/badge.svg)](https://github.com/Bsovann/pne-profiler/actions/workflows/ci.yml)

## Building

```bash
cargo build --release
./target/release/pne-profiler
```

## Workspace layout

| Crate        | Purpose                                          |
|--------------|--------------------------------------------------|
| `pne-core`   | Shared types: samples, metrics, units, errors    |
| `pne-perf`   | `perf_event_open` hardware counters (FFI)        |
| `pne-energy` | RAPL / powercap energy readings, GPU power later |
| `pne-cli`    | The `pne-profiler` binary                        |

## Releasing

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
(`rc/vX.Y.Z`) after every ticket PR for the release has been merged into it.
Commit messages still need the `SCRUM-<n>: ` prefix (see Contribution), so use
the ticket that tracks the release.

1. Fold any remaining ticket fragments and commit the result. Folding sets
   `Version` in both `changelog.yml` and `Cargo.toml`, so they already match.

   ```bash
   git switch rc/vX.Y.Z && git pull
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
   git push origin rc/vX.Y.Z
   ```

5. Open a PR from the rc branch into `master` and merge it once CI is green.
   The changelog check only runs on PRs into `rc/**`, so this PR needs no
   fragment.

   ```bash
   gh pr create --base master --head rc/vX.Y.Z --title "Release X.Y.Z"
   ```

6. Tag the merge on `master`:

   ```bash
   git switch master && git pull
   git tag -a vX.Y.Z -m "pne-profiler X.Y.Z"
   git push origin vX.Y.Z
   ```

7. Build the RPM from `RPM/pne-profiler.spec` if you're publishing a package.

`RPM/update-changelog.py` refuses to run when:

- Ticket fragments are still waiting to be folded.
- `Release_Date` is missing or still `TBD`.
- `Version` doesn't match `Cargo.toml`. An RPM pre-release such as
  `0.1.0~rc1` matches Cargo's `0.1.0-rc1`.
- The spec already has an entry for that version-release.

Any `%` in an entry is written as `%%` so RPM doesn't expand it as a macro.
The script needs Python 3.11+ and PyYAML (Fedora: `python3-pyyaml`).

For a packaging-only change (same code, spec fix), bump `Release` in the spec
and add the `%changelog` entry by hand.

### After a release

For the next release, branch a new rc off `master`, named after the version
you expect, e.g. `rc/v0.2.0`. The first fold after a release starts a new
`Release_Date: TBD` document, and the folded tickets decide the actual
version. If they turn out to need a different bump than the branch name
says, the version in `changelog.yml` is the one that ships.

## Contribution

Work is tracked in Jira and flows ticket branch → `rc/vX.Y.Z` → `master`.

1. Branch off the current release candidate, naming the branch after the Jira
   key so Jira links it. Start commit messages with the key too.

   ```bash
   git switch rc/vX.Y.Z && git pull   # the current release candidate
   git switch -c SCRUM-10
   git commit -m "SCRUM-10: ..."
   ```

   A `commit-msg` hook rejects messages that don't start with `SCRUM-<n>: `.
   Enable it once per clone:

   ```bash
   git config core.hooksPath .githooks
   ```

2. Record your changes in `Changelogs/ChangeFragments/SCRUM-10.yml`, copied
   from `PG-0000-Template.yml`. Keep only the sections that apply
   (`Trivial_Changes`, `Major_Changes`, `Minor_Changes`, `Bug_Fixes`) and
   write each entry as `- (component) - (verb past tense) (description)`.
   For a change to the shipped code, use the crate name as the component,
   e.g. `(pne-cli)`; only those entries reach the RPM changelog. Quote an
   entry that contains `: `. Don't edit `changelog.yml` directly.
3. Check the fragment, then open a PR into the rc branch:

   ```bash
   Changelogs/fragments.py check
   ```

   CI fails a PR to `rc/**` that adds no fragment. For a change that needs no
   changelog entry, the maintainer can label the PR `skip-changelog`.
4. After the PR is approved and merged, the maintainer folds fragments into
   `changelog.yml`, which deletes them:

   ```bash
   Changelogs/fragments.py fold --dry-run
   Changelogs/fragments.py fold
   ```

   Fragments go into the last document in `changelog.yml`. If that release
   already has a `Release_Date`, it has shipped, so the fold starts a new
   `Release_Date: TBD` document after it. Earlier releases are never changed.

   Each fold also sets the current release's `Version` and
   `[workspace.package] version` in `Cargo.toml`. It takes the previous
   release's `Version` and bumps it by the most significant section the
   current release holds:

   | Section            | Bump                 | 0.4.2 becomes |
   |--------------------|----------------------|---------------|
   | `Major_Changes`    | major, resets others | 1.0.0         |
   | `Minor_Changes`    | minor, resets patch  | 0.5.0         |
   | `Bug_Fixes`        | patch                | 0.4.3         |
   | `Trivial_Changes`  | none                 | 0.4.2         |

   The bump is computed from the previous release, not from the previous
   fold, so folding three minor tickets into one rc still gives 0.5.0. For
   the first release, `Version` is left as set. Commit `Cargo.lock` too after the
   next build picks up the new version.
