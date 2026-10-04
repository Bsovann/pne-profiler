//! Shared types for pne-profiler: samples, metrics, units, and errors.
//!
//! This crate has no OS-specific code, so every other crate can depend on it.

#![forbid(unsafe_code)]

use std::time::Duration;

/// A size in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bytes(pub u64);

impl Bytes {
    /// Converts kibibytes (1024 bytes each) to bytes.
    pub const fn from_kib(kib: u64) -> Self {
        Bytes(kib * 1024)
    }
}

/// How the target process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExitOutcome {
    /// Exited normally with this exit code.
    Exited(i32),
    /// Killed by this signal number.
    Signaled(i32),
}

/// CPU time and memory the target used over its whole run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceUsage {
    pub user_time: Duration,
    pub sys_time: Duration,
    /// Peak resident set size.
    pub max_rss: Bytes,
}

impl ResourceUsage {
    /// Total CPU time, user plus system.
    pub fn cpu_time(&self) -> Duration {
        self.user_time + self.sys_time
    }
}

/// Everything measured about one run of the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunStats {
    pub wall_time: Duration,
    pub resource_usage: ResourceUsage,
}

impl RunStats {
    /// CPU time divided by wall time. Above 1.0 means the process used more
    /// than one core on average (e.g. ~4.0 for four busy threads).
    ///
    /// Returns `None` for a zero wall time, which a real run won't produce
    /// but a hand-built value can.
    pub fn cpu_utilization(&self) -> Option<f64> {
        if self.wall_time.is_zero() {
            return None;
        }
        Some(self.resource_usage.cpu_time().as_secs_f64() / self.wall_time.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(user_ms: u64, sys_ms: u64) -> ResourceUsage {
        ResourceUsage {
            user_time: Duration::from_millis(user_ms),
            sys_time: Duration::from_millis(sys_ms),
            max_rss: Bytes(0),
        }
    }

    #[test]
    fn from_kib_converts_to_bytes() {
        assert_eq!(Bytes::from_kib(4), Bytes(4096));
    }

    #[test]
    fn cpu_time_sums_user_and_sys() {
        assert_eq!(usage(300, 200).cpu_time(), Duration::from_millis(500));
    }

    #[test]
    fn cpu_utilization_is_cpu_over_wall() {
        let stats = RunStats {
            wall_time: Duration::from_secs(1),
            resource_usage: usage(1500, 500),
        };
        assert_eq!(stats.cpu_utilization(), Some(2.0));
    }

    #[test]
    fn cpu_utilization_is_none_for_zero_wall_time() {
        let stats = RunStats {
            wall_time: Duration::ZERO,
            resource_usage: usage(100, 0),
        };
        assert_eq!(stats.cpu_utilization(), None);
    }
}
