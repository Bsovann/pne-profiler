//! Shared types for pne-profiler: samples, metrics, units, and errors.
//!
//! This crate has no OS-specific code, so every other crate can depend on it.

#![forbid(unsafe_code)]

use std::{fmt::Display, time::Duration};

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

/// The end-of-run report. It starts with a blank line to separate it from
/// the target's own output, and ends with a single newline.
impl Display for RunStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let usage = self.resource_usage;
        let utilization = match self.cpu_utilization() {
            Some(u) => format!("{:.0}%", u * 100.0),
            None => "n/a".to_string(),
        };
        let max_rss_mib = usage.max_rss.0 as f64 / (1024.0 * 1024.0);

        writeln!(f)?;
        writeln!(f, "pne-profiler: resource usage")?;
        writeln!(f, "  wall time   {:.3} s", self.wall_time.as_secs_f64())?;
        writeln!(f, "  user time   {:.3} s", usage.user_time.as_secs_f64())?;
        writeln!(f, "  sys time    {:.3} s", usage.sys_time.as_secs_f64())?;
        writeln!(f, "  CPU usage   {utilization}")?;
        writeln!(f, "  max RSS     {max_rss_mib:.1} MiB")
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

    fn stats(wall_ms: u64, user_ms: u64, sys_ms: u64, max_rss: Bytes) -> RunStats {
        RunStats {
            wall_time: Duration::from_millis(wall_ms),
            resource_usage: ResourceUsage {
                max_rss,
                ..usage(user_ms, sys_ms)
            },
        }
    }

    /// Joins `lines` into one string, each ending in a newline.
    fn lines(lines: &[&str]) -> String {
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    // The report tests compare the whole string, so any change to labels,
    // spacing, precision, or units fails here and has to be made on purpose.

    #[test]
    fn report_format() {
        // 1.5 s of CPU over 2 s of wall time is 75%; 3 MiB + 512 KiB is 3.5 MiB.
        let report = stats(2000, 1250, 250, Bytes::from_kib(3 * 1024 + 512)).to_string();
        assert_eq!(
            report,
            lines(&[
                "",
                "pne-profiler: resource usage",
                "  wall time   2.000 s",
                "  user time   1.250 s",
                "  sys time    0.250 s",
                "  CPU usage   75%",
                "  max RSS     3.5 MiB",
            ])
        );
    }

    #[test]
    fn report_format_rounds_to_display_precision() {
        // Times round to milliseconds, CPU to whole percent, RSS to 0.1 MiB.
        let report = stats(3, 1, 1, Bytes(1_100_000)).to_string();
        assert_eq!(
            report,
            lines(&[
                "",
                "pne-profiler: resource usage",
                "  wall time   0.003 s",
                "  user time   0.001 s",
                "  sys time    0.001 s",
                "  CPU usage   67%",
                "  max RSS     1.0 MiB",
            ])
        );
    }

    #[test]
    fn report_format_parallel_cpu_above_100_percent() {
        let report = stats(1000, 4000, 0, Bytes(0)).to_string();
        assert_eq!(
            report,
            lines(&[
                "",
                "pne-profiler: resource usage",
                "  wall time   1.000 s",
                "  user time   4.000 s",
                "  sys time    0.000 s",
                "  CPU usage   400%",
                "  max RSS     0.0 MiB",
            ])
        );
    }

    #[test]
    fn report_format_zero_wall_time() {
        let report = stats(0, 0, 0, Bytes(0)).to_string();
        assert_eq!(
            report,
            lines(&[
                "",
                "pne-profiler: resource usage",
                "  wall time   0.000 s",
                "  user time   0.000 s",
                "  sys time    0.000 s",
                "  CPU usage   n/a",
                "  max RSS     0.0 MiB",
            ])
        );
    }
}
