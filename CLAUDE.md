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

CI (`.github/workflows/ci.yml`) runs fmt check, clippy, and tests, with `RUSTFLAGS=-D warnings`.

## Changelog fragments

Changes are recorded as YAML fragments in `Changelogs/ChangeFragments/`, one file per change, named after the ticket ID (template: `PG-0000-Template.yml`). Copy the template, drop sections that don't apply, and write each entry as:

```
- (component) - (verb past tense) + (change description)
```

Sections: `Trivial Changes`, `Major_Changes`, `Minor_Changes`, `Bug_Fixes`. Note the template spells the first key with a space while the others use underscores — keep the keys exactly as in the template unless the convention is deliberately changed.
