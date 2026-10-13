# RPM spec for pne-profiler.
#
# Build from a tagged release (Source0 is the GitHub tag archive):
#   rpmdev-setuptree
#   spectool -g -R RPM/pne-profiler.spec
#   rpmbuild -ba RPM/pne-profiler.spec
#
# The build runs with --offline. That works while the workspace has no
# external crates; once it gains some, ship a `cargo vendor` tarball as
# Source1 and point .cargo/config.toml at it in %prep.

Name:           pne-profiler
# Keep in sync with [workspace.package] version in Cargo.toml;
# RPM/update-changelog.py sets this at release time.
Version:        0.2.0
Release:        1%{?dist}
Summary:        Performance and energy profiler for parallel workloads

License:        Apache-2.0
URL:            https://github.com/Bsovann/pne-profiler
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

# perf_event_open and RAPL powercap are Linux-only.
ExclusiveOS:    Linux

# Matches rust-version in Cargo.toml.
BuildRequires:  rust >= 1.85
BuildRequires:  cargo

%description
pne-profiler attaches to a running process and reports hardware counters
(cycles, instructions, cache misses) via perf_event_open, memory bandwidth
and roofline position, and energy consumed via RAPL and GPU power APIs.
Energy is treated as a first-class metric.

%prep
%autosetup -n %{name}-%{version}

%build
# Emit debug info so rpm can split it into the -debuginfo package.
export CARGO_PROFILE_RELEASE_DEBUG=2
cargo build --release --locked --offline -p pne-cli

%install
install -Dpm 0755 target/release/%{name} %{buildroot}%{_bindir}/%{name}

%check
cargo test --release --locked --offline --workspace

%files
%license LICENSE
%doc README.md
%{_bindir}/%{name}

# Entries are generated from Changelogs/ChangeFragments/changelog.yml at
# release time by RPM/update-changelog.py. Add one by hand only for a
# packaging-only change (bump Release, keep Version).
%changelog
* Mon Oct 12 2026 Bondith Sovann <bsovann04@gmail.com> - 0.2.0-1
- (pne-core) - added the Bytes, ExitOutcome, ResourceUsage, and RunStats types for reporting the target's resource usage.
- (pne-core) - added Bytes::from_kib to convert kibibyte counts such as getrusage's ru_maxrss to bytes.
- (pne-core) - added ResourceUsage::cpu_time and RunStats::cpu_utilization, which returns CPU time over wall time or None for a zero wall time.
- (pne-perf) - added a safe wait4 wrapper over libc that takes ownership of a spawned Child, reaps it, and returns its exit outcome, user and system CPU time, and max RSS.
- (pne-cli) - changed run to reap the target with wait4 and print its wall time, user and system CPU time, CPU usage, and max RSS to stderr after it exits.
- (pne-core) - moved the resource usage report into a Display impl on RunStats.
- (pne-perf) - changed wait4 to return a typed WaitError built with thiserror instead of io::Error.
- (pne-cli) - changed error handling to anyhow, printing each error with its full context chain while keeping exit codes 126 and 127 for targets that cannot be started.

* Sat Oct 03 2026 Bondith Sovann <bsovann04@gmail.com> - 0.1.0-1
- (pne-cli) - added clap argument parsing with a run subcommand that forwards the target command and its arguments unchanged.
- (pne-cli) - added spawning of the target command with inherited stdio.
- (pne-cli) - exited with the target's exit code, 128 + signal when the target is killed, 127 when the command is not found, and 126 when it cannot be executed.

