# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`pne-profiler` (performance & energy) is a profiler for parallel workloads. It attaches to a running process, and later to an MPI job across nodes, and reports:

- hardware counters (cycles, instructions, cache misses) via `perf_event_open`
- memory bandwidth and roofline position (arithmetic intensity vs. achieved FLOP/s)
- energy consumed, via RAPL (CPU/DRAM) and GPU power APIs (e.g. NVML)

Energy is a first-class metric, not an add-on. The question the product answers is "which job wastes the most energy per result". Existing HPC profilers can do a lot but are clunky, and few of them treat energy as central.

Planned technical areas, roughly in order: Linux `perf_event` and `/proc` → unsafe Rust / FFI → lock-free sample collection → eBPF → distributed aggregation across nodes.

## Workspace layout

A Cargo workspace under `crates/`. The dependency direction is `pne-cli` → {`pne-perf`, `pne-energy`} → `pne-core`.

- `pne-core`: OS-independent shared types (samples, metrics, units, errors). Everything depends on it, and it depends on nothing.
- `pne-perf`: `perf_event_open` FFI. **This is the only crate allowed to use `unsafe`.** The others declare `#![forbid(unsafe_code)]`. Every `unsafe` block needs a `// SAFETY:` comment (enforced by `clippy::undocumented_unsafe_blocks`).
- `pne-energy`: RAPL via powercap sysfs, with GPU power APIs later.
- `pne-cli`: builds the `pne-profiler` binary. It's the only default workspace member, so `cargo run` runs it.

Future subsystems (eBPF, distributed aggregation) should become new crates under `crates/` rather than growing existing ones. Lints and shared package metadata live in the root `Cargo.toml` (`[workspace.lints]`, `[workspace.package]`); member crates opt in with `[lints] workspace = true`. Internal crates are referenced as `pne-x.workspace = true`.

Most crates are still stubs, so check the code before assuming an API exists.

## Platform constraints

- Linux only. Attaching to another process's counters depends on `/proc/sys/kernel/perf_event_paranoid` (≤ 1 for per-process CPU events without `CAP_PERFMON`, ≤ 0 for kernel/system-wide events) and on ptrace permissions.
- RAPL energy counters (`/sys/class/powercap/intel-rapl*/energy_uj`) are root-only on modern kernels, and they wrap around at `max_energy_range_uj`. Code must handle the wraparound and degrade gracefully when access is denied.

## Commands

```bash
cargo build                                     # build
cargo run -- <args>                             # run the pne-profiler binary
cargo test --workspace                          # all tests
cargo test -p pne-perf <name_substring>         # one crate / single test
cargo clippy --workspace --all-targets          # lint (CI uses -D warnings)
cargo fmt --all                                 # format
```

CI (`.github/workflows/ci.yml`) runs fmt check, clippy, tests (with `RUSTFLAGS=-D warnings`), and a `typos` spell check (exceptions go in `_typos.toml`). `.github/workflows/changelog.yml` validates changelog fragments on PRs to `rc/**` and requires each PR to add one unless it's labeled `skip-changelog`.

```bash
Changelogs/fragments.py check                   # validate fragments (same as CI)
```

## Branches and changelog

Work flows ticket branch → `rc/vX.Y.Z` → `master`:

1. Each Jira ticket gets a branch off the current `rc/*` branch, named after the Jira key (`SCRUM-10`, optionally `SCRUM-10-short-desc`). Commits start with the key (`SCRUM-10: ...`) so Jira links them; use the hyphenated form, not `SCRUM10`.
2. The ticket's changes are recorded in `Changelogs/ChangeFragments/<JIRA-KEY>.yml`, copied from `PG-0000-Template.yml`. Drop sections that don't apply and write each entry as:

   ```
   - (component) - (verb past tense) + (change description)
   ```

   Sections: `Trivial_Changes`, `Major_Changes`, `Minor_Changes`, `Bug_Fixes`; pick the one matching the change type. For changes to shipped code, the component must be the crate name (`(pne-cli)`, `(pne-perf)`, ...): `RPM/update-changelog.py` only copies `pne-*` entries into the RPM `%changelog`, and tooling components (`CI`, `RPM`, `Changelog`, `Licensing`, `Tests`, ...) stay in `changelog.yml`. Quote an entry that contains `: `, or YAML parses it as a mapping.
3. The ticket branch is PR'd into the rc branch and reviewed by the maintainer.
4. The maintainer folds approved fragments into `changelog.yml` with `Changelogs/fragments.py fold`, which deletes the folded fragments. `changelog.yml` holds one YAML document per release (`Version`, `Release_Date`, sections), oldest first. Fragments fold into the last document; if it already has a `Release_Date`, the fold starts a new `Release_Date: TBD` document. Folding sets the current release's `Version` (and `Cargo.toml`'s) by bumping the previous release's `Version` by the most significant section present: major, minor, or patch for `Major_Changes`, `Minor_Changes`, `Bug_Fixes`. Don't edit `changelog.yml` from a ticket branch.
5. At release, the last document's `Release_Date` is filled in and `RPM/update-changelog.py` turns that document into the RPM `%changelog` entry (see README "Releasing"), and the rc branch is merged to `master` and tagged.
