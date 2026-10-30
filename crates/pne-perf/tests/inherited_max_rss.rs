//! SCRUM-18: a child's `ru_maxrss` includes memory its parent used before the
//! child exec'd. At exec, Linux folds the RSS high-water mark of the address
//! space being replaced into the child's peak (`setmax_mm_hiwater_rss` in
//! `exec_mmap`), so the reported peak is never below that.
//!
//! What that old address space is depends on how the child was created:
//!
//! - `posix_spawn` (what `Command::spawn` uses by default) creates the child
//!   with `CLONE_VM | CLONE_VFORK`, so the child execs from the parent's own
//!   address space. The floor is the parent's peak RSS so far (`VmHWM`), even
//!   for memory it has already freed.
//! - `fork` gives the child a copy of the parent's address space, whose
//!   high-water mark starts at the RSS copied into it. The floor is the
//!   parent's resident anonymous memory at the moment of the fork.
//!
//! This is its own test binary, with a single test, because growing the test
//! process raises the floor for every other test spawned from it.

use pne_core::Bytes;
use std::os::unix::process::CommandExt;
use std::process::Command;

const MIB: u64 = 1024 * 1024;

/// Peak RSS of `true`, which on its own stays well under 8 MiB.
fn max_rss_of_true(mut cmd: Command) -> Bytes {
    let (_, usage) = pne_perf::wait4(cmd.spawn().unwrap()).unwrap();
    usage.max_rss
}

#[test]
fn child_max_rss_inherits_parent_peak_through_posix_spawn_but_not_fork() {
    const GROWTH: u64 = 256 * MIB;

    // Grow the parent, touching every page so it's resident, then free it.
    // An allocation this large is its own mmap, so freeing it returns the
    // memory to the kernel and the parent's current RSS drops back.
    let memory = vec![1u8; GROWTH as usize];
    std::hint::black_box(&memory);
    drop(memory);

    // The posix_spawn path still reports the parent's freed peak.
    let spawned = max_rss_of_true(Command::new("true"));
    assert!(
        spawned >= Bytes(GROWTH),
        "posix_spawn: expected at least the parent's {} MiB peak, got {} MiB",
        GROWTH / MIB,
        spawned.0 / MIB
    );

    // Any pre_exec hook makes std fork instead of using posix_spawn.
    let mut forked = Command::new("true");
    // SAFETY: the hook does nothing, so it can't break the rules for code
    // running in a forked child (async-signal-safe calls only, no allocation).
    unsafe { forked.pre_exec(|| Ok(())) };
    let forked = max_rss_of_true(forked);
    assert!(
        forked < Bytes(8 * MIB),
        "fork: expected only true's own few MiB, got {} MiB",
        forked.0 / MIB
    );
}
