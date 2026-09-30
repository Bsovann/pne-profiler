# pne-profiler

A performance and energy profiler for parallel workloads on Linux. It reports
hardware counters, cache behaviour, memory bandwidth, roofline position, and
energy consumed (RAPL, GPU power APIs), with energy treated as a first-class
metric.

Status: early development.

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
