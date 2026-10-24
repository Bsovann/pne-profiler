# pne-profiler

A performance and energy profiler for parallel workloads on Linux. It reports
hardware counters, cache behaviour, memory bandwidth, roofline position, and
energy consumed (RAPL, GPU power APIs), with energy treated as a first-class
metric.

Status: early development. [![CI](https://github.com/Bsovann/pne-profiler/actions/workflows/ci.yml/badge.svg)](https://github.com/Bsovann/pne-profiler/actions/workflows/ci.yml)
Test Coverage: [![Coverage](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FBsovann%2Fpne-profiler%2Fbadges%2Fmaster.json)](https://github.com/Bsovann/pne-profiler/actions/workflows/ci.yml?query=branch%3Amaster)

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

Each rc branch ships as one release: fold the remaining fragments, date the
last `changelog.yml` document, generate the RPM `%changelog` with
`RPM/update-changelog.py`, then merge the rc branch into `master` and tag it.
See [RELEASE.md](RELEASE.md) for the full steps.

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

### Test coverage

CI runs `cargo llvm-cov --workspace` on every PR and on pushes to `master`
and `rc/**`, and prints the per-file summary in the job summary. After a push,
it updates the coverage badge above (`master`) from the `badges` branch, which
holds one shields.io JSON file per branch.

To run it locally:

```bash
rustup component add llvm-tools-preview   # once, for the pinned toolchain
cargo install cargo-llvm-cov --locked     # once
cargo llvm-cov --workspace                # per-file summary
cargo llvm-cov --workspace --open         # HTML report with uncovered lines
```

Coverage is not gated. Use it to find untested branches, such as error paths,
not as a number to push up.
