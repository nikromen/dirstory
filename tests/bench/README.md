# Shell latency benchmark

Run `just container-bench` from the repository root after building the usual test
image (`just container-build-image`). The recipe builds the release binary first,
then runs the benchmark alone. `just bench` is the equivalent recipe inside the
container. No dependencies beyond the existing test image are needed.

The default matrix covers Bash, Zsh, and Fish: tty histories with 100, 10,000, and
100,000 visits, plus pid and tmux histories with 10,000 visits. Every case measures
`:`, an ordinary `cd` appending at the end, one successful `b`, one successful `f`,
and both ten-item directional lists from the middle. List output goes to
`/dev/null`. The same existing directories and equal-length paths are reused for
all sizes in a shell. No TUI, cold-cache, or concurrent-load scenarios are included.

The script reuses `tests/e2e/hooks.py`'s `Console` and `runtest.py`'s `ShellCase`,
including initialization, real interactive PTYs, and prompt readiness. Each mode/size case starts a fresh shell, which
stays alive for all warmups and measured samples; this prevents growing interactive
shell state from being mistaken for directory-history scaling. Fish loads the same generated integration from a
temporary file: piping into `source` gives its initialization backend a pipe on
stdin, which cannot identify a tty session. This startup difference is outside
timing and does not change production integration. Python generates the current v1 binary format once
per fixture and validates every record with `internal snapshot`, outside timing.
Before each sample it sets the starting PWD with tracking suppressed, reconciles
the integration's last PWD, and restores the history header under the existing
lock. A preceding append is truncated. Setup and validation are never timed.

Each scenario has 10 warmups and 100 measured repetitions. `perf_counter_ns()`
measures from sending the shell command through receipt of its next prompt,
including process launches, configuration, locks, I/O, PTY transport, and prompt
handling. Commands are encoded before starting the timer; the timed interval
begins immediately before the PTY write and uses the existing prompt reader.
After timing, the script verifies command success, exact PWD,
cursor, revision, and file size. Failures abort rather than becoming samples.

The table reports median and nearest-rank p95 in milliseconds. The same values,
all individual nanosecond samples, shell versions, Rust version, binary hash, and
actual runtime file paths are saved to `target/bench/baseline-*.json`. The container
recipe binds this output directory to the host; temporary runtime directories are
removed afterward. Reports are also updated after each scenario; only a report
with `complete: true` represents a completed run.

Compare `b` and `f` across tty sizes, then compare their 10,000-visit measurements
across modes. Fixed ten-item lists should likewise remain independent of the
older history prefix. The separate `:` result estimates transport and prompt
cost; it is **not subtracted**. Differences of a few tenths of a millisecond can
reflect scheduling or cache noise rather than algorithmic growth. Repeat the
same scenarios on the same machine before attributing a regression to processes,
configuration, locking, or storage. This benchmark does not justify a history
limit because it does not measure loading or rendering the complete TUI history.

For focused reruns, use e.g.
`BENCH_SHELL=bash BENCH_MODE=tty BENCH_SIZE=10000 just container-bench`.
The script also accepts `--shell`, `--mode`, and `--size` directly inside the
container. Filters select from the documented matrix; they do not add scenarios.
Run scenarios sequentially, without other builds, tests, or benchmark processes.
The benchmark is deliberately excluded from `test-all` and required CI checks.
The normal Rust suite separately checks equal Storage I/O counts and byte volumes
for one-step selection plus confirmation and ten-item lists at all three sizes;
there are no wall-clock performance assertions in automated tests.
