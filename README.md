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

Changes for the current release candidate are tracked in
`Changelogs/ChangeFragments/changelog.yml`. When releasing, the RPM spec's
`%changelog` entry is generated from that file:

1. Fold any remaining ticket fragments into `changelog.yml` (see
   Contribution), then fill in `Release_Date` (`YYYY-MM-DD`) and make sure
   `Version` matches `[workspace.package] version` in `Cargo.toml`.
2. Preview the entry, then write it:

   ```bash
   RPM/update-changelog.py --dry-run
   RPM/update-changelog.py
   ```

3. The script sets `Version` in `RPM/pne-profiler.spec`, resets `Release` to
   `1`, and prepends a `%changelog` entry built from the changelog sections,
   using your git `user.name` and `user.email` as the packager.

The script refuses to run when:

- Ticket fragments are still waiting to be folded.
- `Release_Date` is missing or still `TBD`.
- `Version` doesn't match `Cargo.toml`. An RPM pre-release such as
  `0.1.0~rc1` matches Cargo's `0.1.0-rc1`.
- The spec already has an entry for that version-release.

Any `%` in an entry is written as `%%` so RPM doesn't expand it as a macro.
The script needs Python 3.11+ and PyYAML (Fedora: `python3-pyyaml`).

For a packaging-only change (same code, spec fix), bump `Release` in the spec
and add the `%changelog` entry by hand.

## Contribution

Work is tracked in Jira and flows ticket branch → `rc/vX.Y.Z` → `master`.

1. Branch off the current release candidate, naming the branch after the Jira
   key so Jira links it. Start commit messages with the key too.

   ```bash
   git switch rc/v0.0.0 && git pull
   git switch -c SCRUM-10
   git commit -m "SCRUM-10: ..."
   ```

2. Record your changes in `Changelogs/ChangeFragments/SCRUM-10.yml`, copied
   from `PG-0000-Template.yml`. Keep only the sections that apply
   (`Trivial_Changes`, `Major_Changes`, `Minor_Changes`, `Bug_Fixes`) and
   write each entry as `- (component) - (verb past tense) (description)`.
   Quote an entry that contains `: `. Don't edit `changelog.yml` directly.
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
