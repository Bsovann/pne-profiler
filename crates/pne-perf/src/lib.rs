//! Hardware performance counters via Linux `perf_event_open(2)`, plus the
//! other raw syscalls the profiler needs, such as `wait4(2)`.
//!
//! This is the only crate allowed to contain `unsafe`. Every `unsafe` block
//! must carry a `// SAFETY:` comment explaining the invariant it relies on.

use pne_core::{Bytes, ExitOutcome, ResourceUsage};
use std::io;
use std::time::Duration;

/// Waits for the child process `pid` to end and returns how it ended along
/// with the CPU time and peak memory it used, as reported by `wait4(2)`.
///
/// `pid` must be a child of this process that hasn't been waited on yet.
/// The usage covers the child and any descendants it waited on itself.
pub fn wait4(pid: u32) -> io::Result<(ExitOutcome, ResourceUsage)> {
    let pid =
        libc::pid_t::try_from(pid).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let mut status: libc::c_int = 0;
    // SAFETY: rusage is a C struct of integers and timevals, for which all
    // zero bytes is a valid value.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };

    loop {
        // SAFETY: `status` and `ru` are live, writable locals of the types
        // wait4 expects, and the kernel only writes to them during the call.
        let ret = unsafe { libc::wait4(pid, &mut status, 0, &mut ru) };
        if ret == pid {
            break;
        }
        // A signal arrived before the child ended; wait again.
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }

    // With no WUNTRACED/WCONTINUED flags, wait4 only returns for a child that
    // exited or was killed.
    let outcome = if libc::WIFEXITED(status) {
        ExitOutcome::Exited(libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        ExitOutcome::Signaled(libc::WTERMSIG(status))
    } else {
        return Err(io::Error::other(format!(
            "unexpected wait status {status:#x}"
        )));
    };

    let usage = ResourceUsage {
        user_time: timeval_to_duration(ru.ru_utime),
        sys_time: timeval_to_duration(ru.ru_stime),
        // Linux reports ru_maxrss in kibibytes.
        max_rss: Bytes::from_kib(ru.ru_maxrss as u64),
    };
    Ok((outcome, usage))
}

/// The kernel never reports negative times in rusage, so the casts are lossless.
fn timeval_to_duration(tv: libc::timeval) -> Duration {
    Duration::from_secs(tv.tv_sec as u64) + Duration::from_micros(tv.tv_usec as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[expect(clippy::zombie_processes, reason = "wait4 reaps the child")]
    fn run(script: &str) -> (ExitOutcome, ResourceUsage) {
        let child = Command::new("sh").args(["-c", script]).spawn().unwrap();
        wait4(child.id()).unwrap()
    }

    #[test]
    fn reports_exit_code() {
        let (outcome, _) = run("exit 3");
        assert_eq!(outcome, ExitOutcome::Exited(3));
    }

    #[test]
    fn reports_killing_signal() {
        let (outcome, _) = run("kill -9 $$");
        assert_eq!(outcome, ExitOutcome::Signaled(libc::SIGKILL));
    }

    #[test]
    fn reports_nonzero_max_rss() {
        let (_, usage) = run("true");
        assert!(usage.max_rss > Bytes(0));
    }

    #[test]
    fn non_child_pid_is_an_error() {
        // PID 1 (init) is never our child, so wait4 fails with ECHILD.
        let err = wait4(1).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(libc::ECHILD));
    }
}
