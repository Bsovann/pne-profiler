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
///
/// This is for people: times and sizes are scaled to a readable unit.
/// Machine-readable output should use the raw `Duration` and `Bytes` instead.
impl Display for RunStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let usage = self.resource_usage;
        let utilization = match self.cpu_utilization() {
            Some(u) => format!("{:.0}%", u * 100.0),
            None => "n/a".to_string(),
        };

        writeln!(f)?;
        writeln!(f, "pne-profiler: resource usage")?;
        writeln!(f, "  wall time   {}", format_duration(self.wall_time))?;
        writeln!(f, "  user time   {}", format_duration(usage.user_time))?;
        writeln!(f, "  sys time    {}", format_duration(usage.sys_time))?;
        writeln!(f, "  CPU usage   {utilization}")?;
        writeln!(f, "  max RSS     {}", format_bytes(usage.max_rss))
    }
}

/// Divides, rounding halves up.
fn round_div(n: u128, d: u128) -> u128 {
    (n + d / 2) / d
}

/// Formats a duration as `N.NNN s` under a minute, `h:mm:ss.s` from a
/// minute, and `Nd h:mm:ss.s` from a day.
///
/// The format is chosen from the rounded value, so 59.9996 s, which rounds
/// to 60.000 s, prints as `0:01:00.0`, and 23:59:59.96 prints as
/// `1d 0:00:00.0`.
fn format_duration(d: Duration) -> String {
    let nanos = d.as_nanos();
    let millis = round_div(nanos, 1_000_000);
    if millis < 60_000 {
        return format!("{}.{:03} s", millis / 1000, millis % 1000);
    }

    let tenths = round_div(nanos, 100_000_000);
    let (secs, tenth) = (tenths / 10, tenths % 10);
    let (mins, sec) = (secs / 60, secs % 60);
    let (hours, min) = (mins / 60, mins % 60);
    let (days, hour) = (hours / 24, hours % 24);
    let clock = format!("{hour}:{min:02}:{sec:02}.{tenth}");
    if days == 0 {
        clock
    } else {
        format!("{days}d {clock}")
    }
}

/// Formats a size in KiB, MiB, GiB, or TiB (1024-based) with one decimal
/// place.
///
/// It moves to the next unit once the rounded value reaches 1024.0, so
/// 1023.95 MiB prints as `1.0 GiB`, not `1024.0 MiB`. Sizes under 1 KiB
/// still print in KiB, and sizes of 1024 TiB or more stay in TiB.
fn format_bytes(bytes: Bytes) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    let tenths_in = |scale| round_div(u128::from(bytes.0) * 10, scale);

    let mut unit = 0;
    let mut scale = 1024;
    let mut tenths = tenths_in(scale);
    while unit + 1 < UNITS.len() && tenths >= 1024 * 10 {
        unit += 1;
        scale *= 1024;
        tenths = tenths_in(scale);
    }
    format!("{}.{} {}", tenths / 10, tenths % 10, UNITS[unit])
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
                "  max RSS     0.0 KiB",
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
                "  max RSS     0.0 KiB",
            ])
        );
    }

    #[test]
    fn report_format_long_run_large_memory() {
        // 30 days of wall time with 200 GiB peak RSS, the case that used to
        // print as 2592000.000 s and 204800.0 MiB.
        let report = stats(
            30 * 24 * 3600 * 1000,
            3_723_400,
            59_999,
            Bytes::from_kib(200 * 1024 * 1024),
        )
        .to_string();
        assert_eq!(
            report,
            lines(&[
                "",
                "pne-profiler: resource usage",
                "  wall time   30d 0:00:00.0",
                "  user time   1:02:03.4",
                "  sys time    59.999 s",
                "  CPU usage   0%",
                "  max RSS     200.0 GiB",
            ])
        );
    }

    fn nanos(n: u64) -> String {
        format_duration(Duration::from_nanos(n))
    }

    fn secs(s: u64) -> String {
        format_duration(Duration::from_secs(s))
    }

    #[test]
    fn duration_under_a_minute_is_seconds_with_millis() {
        assert_eq!(secs(0), "0.000 s");
        assert_eq!(nanos(499_999), "0.000 s");
        assert_eq!(nanos(500_000), "0.001 s");
        assert_eq!(nanos(59_999_000_000), "59.999 s");
        assert_eq!(nanos(59_999_499_999), "59.999 s");
    }

    #[test]
    fn duration_that_rounds_to_a_minute_is_clock_format() {
        // 59.9995 s and up round to 60.000 s, so they must not print as
        // "60.000 s".
        assert_eq!(nanos(59_999_500_000), "0:01:00.0");
        assert_eq!(nanos(59_999_600_000), "0:01:00.0");
        assert_eq!(secs(60), "0:01:00.0");
    }

    #[test]
    fn duration_clock_format_carries_on_rounding() {
        assert_eq!(nanos(3_723_400_000_000), "1:02:03.4");
        assert_eq!(nanos(3_599_949_999_999), "0:59:59.9");
        assert_eq!(nanos(3_599_950_000_000), "1:00:00.0");
        assert_eq!(secs(36_000), "10:00:00.0");
    }

    #[test]
    fn duration_from_a_day_shows_days() {
        assert_eq!(nanos(86_399_949_999_999), "23:59:59.9");
        assert_eq!(nanos(86_399_950_000_000), "1d 0:00:00.0");
        assert_eq!(secs(86_400), "1d 0:00:00.0");
        assert_eq!(secs(30 * 86_400), "30d 0:00:00.0");
        assert_eq!(
            nanos(30 * 86_400_000_000_000 + 3_723_400_000_000),
            "30d 1:02:03.4"
        );
    }

    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    fn bytes(n: u64) -> String {
        format_bytes(Bytes(n))
    }

    // Each unit boundary is tested just below and at the point where the
    // value rounds to 1024.0. Half a tenth of the lower unit is 51.2 bytes
    // for KiB, 52_428.8 for MiB, and 53_687_091.2 for GiB.

    #[test]
    fn bytes_under_a_mib_are_kib() {
        assert_eq!(bytes(0), "0.0 KiB");
        assert_eq!(bytes(512), "0.5 KiB");
        assert_eq!(bytes(KIB), "1.0 KiB");
        assert_eq!(bytes(MIB - 52), "1023.9 KiB");
    }

    #[test]
    fn bytes_that_round_to_1024_kib_are_mib() {
        assert_eq!(bytes(MIB - 51), "1.0 MiB");
        assert_eq!(bytes(MIB), "1.0 MiB");
        assert_eq!(bytes(3 * MIB + 512 * KIB), "3.5 MiB");
        assert_eq!(bytes(GIB - 52_429), "1023.9 MiB");
    }

    #[test]
    fn bytes_that_round_to_1024_mib_are_gib() {
        assert_eq!(bytes(GIB - 52_428), "1.0 GiB");
        assert_eq!(bytes(GIB), "1.0 GiB");
        assert_eq!(bytes(200 * GIB), "200.0 GiB");
        assert_eq!(bytes(TIB - 53_687_092), "1023.9 GiB");
    }

    #[test]
    fn bytes_that_round_to_1024_gib_are_tib() {
        assert_eq!(bytes(TIB - 53_687_091), "1.0 TiB");
        assert_eq!(bytes(TIB), "1.0 TiB");
        assert_eq!(bytes(2048 * TIB), "2048.0 TiB");
        assert_eq!(bytes(u64::MAX), "16777216.0 TiB");
    }
}
