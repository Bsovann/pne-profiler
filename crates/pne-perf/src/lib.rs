//! Hardware performance counters via Linux `perf_event_open(2)`.
//!
//! This is the only crate allowed to contain `unsafe`. Every `unsafe` block
//! must carry a `// SAFETY:` comment explaining the invariant it relies on.
