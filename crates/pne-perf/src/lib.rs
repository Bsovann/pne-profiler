//! Hardware performance counters via Linux `perf_event_open(2)`, plus the
//! other raw syscalls the profiler needs, such as `wait4(2)`.
//!
//! This is the only crate allowed to contain `unsafe`. Every `unsafe` block
//! must carry a `// SAFETY:` comment explaining the invariant it relies on.

use pne_core::{Bytes, ExitOutcome, ResourceUsage};
use std::io;
use std::time::Duration;

/// Why [`wait4`] couldn't report on the child.
#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    /// The child's PID doesn't fit in `pid_t`, so it can't be passed to wait4.
    #[error("pid {0} does not fit in pid_t")]
    PidOutOfRange(u32),
    /// The wait4 syscall itself failed, e.g. with `ECHILD`.
    #[error("wait4 failed")]
    Wait(#[source] io::Error),
    /// wait4 returned a status that is neither an exit nor a killing signal.
    #[error("unexpected wait status {0:#x}")]
    UnexpectedStatus(libc::c_int),
}

/// Waits for `child` to end and returns how it ended along with the CPU time
/// and peak memory it used, as reported by `wait4(2)`.
///
/// Taking the `Child` by value means it can't be waited on again afterwards,
/// which would fail since wait4 has already reaped it. The usage covers the
/// child and any descendants it waited on itself.
pub fn wait4(child: std::process::Child) -> Result<(ExitOutcome, ResourceUsage), WaitError> {
    let pid =
        libc::pid_t::try_from(child.id()).map_err(|_| WaitError::PidOutOfRange(child.id()))?;
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
            return Err(WaitError::Wait(err));
        }
    }

    // With no WUNTRACED/WCONTINUED flags, wait4 only returns for a child that
    // exited or was killed.
    let outcome = if libc::WIFEXITED(status) {
        ExitOutcome::Exited(libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        ExitOutcome::Signaled(libc::WTERMSIG(status))
    } else {
        return Err(WaitError::UnexpectedStatus(status));
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

    fn run(script: &str) -> (ExitOutcome, ResourceUsage) {
        let child = Command::new("sh").args(["-c", script]).spawn().unwrap();
        wait4(child).unwrap()
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

    fn timeval(tv_sec: libc::time_t, tv_usec: libc::suseconds_t) -> libc::timeval {
        libc::timeval { tv_sec, tv_usec }
    }

    #[test]
    fn timeval_zero_is_zero_duration() {
        assert_eq!(timeval_to_duration(timeval(0, 0)), Duration::ZERO);
    }

    #[test]
    fn timeval_adds_seconds_and_microseconds() {
        assert_eq!(
            timeval_to_duration(timeval(2, 500_000)),
            Duration::from_millis(2500)
        );
    }

    #[test]
    fn timeval_microseconds_are_not_milliseconds() {
        // 1 µs must stay 1 µs: catches from_millis or from_nanos used by mistake.
        assert_eq!(
            timeval_to_duration(timeval(0, 1)),
            Duration::from_nanos(1000)
        );
    }

    #[test]
    fn timeval_largest_microsecond_field_stays_below_next_second() {
        // tv_usec tops out at 999_999, so it must not carry into a full second.
        assert_eq!(
            timeval_to_duration(timeval(3, 999_999)),
            Duration::new(3, 999_999_000)
        );
    }

    #[test]
    fn timeval_handles_long_runs() {
        // A 30-day job, far past where a 32-bit microsecond total would overflow.
        let secs = 30 * 24 * 60 * 60;
        assert_eq!(
            timeval_to_duration(timeval(secs, 0)),
            Duration::from_secs(secs as u64)
        );
    }

    #[test]
    fn wait_error_messages() {
        assert_eq!(
            WaitError::PidOutOfRange(u32::MAX).to_string(),
            "pid 4294967295 does not fit in pid_t"
        );
        assert_eq!(
            WaitError::UnexpectedStatus(0x137f).to_string(),
            "unexpected wait status 0x137f"
        );
    }

    #[test]
    fn wait_error_keeps_the_os_error_as_its_source() {
        use std::error::Error;

        let err = WaitError::Wait(io::Error::from_raw_os_error(libc::ECHILD));
        assert_eq!(err.to_string(), "wait4 failed");
        let source = err.source().and_then(|s| s.downcast_ref::<io::Error>());
        assert_eq!(source.and_then(io::Error::raw_os_error), Some(libc::ECHILD));
    }
}
