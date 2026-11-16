#!/usr/bin/env python3
"""Warm-cache, end-to-end shell latency using the existing E2E PTY fixture."""

import argparse
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import shlex
import statistics
import struct
import subprocess
import sys
import time

E2E = Path(__file__).resolve().parents[1] / "e2e"
sys.path.insert(0, str(E2E))
from hooks import Console  # noqa: E402
from runtest import BINARY, ShellCase  # noqa: E402

WARMUPS = 10
REPETITIONS = 100
SIZES = (100, 10_000, 100_000)
OPERATIONS = (":", "cd", "b", "f", "b --list 10", "f --list 10")


class BenchmarkCase(ShellCase):
    """Retain PTY stdin while loading the existing Fish integration."""

    def init(self, name="cd"):
        if self.shell != "fish":
            return super().init(name)

        script = shlex.quote(str(self.root / "init.fish"))
        return f"dirstory init fish --command {shlex.quote(name)} > {script}; source {script}"


class Fixture:
    """Generate one history, then restore only metadata between timed commands."""

    def __init__(self, case, console, mode, count):
        self.case, self.console, self.count = case, console, count
        self.paths = [case.root / "a", case.root / "b"]
        raw = [os.fsencode(path) for path in self.paths]
        assert len(raw[0]) == len(raw[1])
        self.record_size = 16 + len(raw[0])
        self.end = 64 + count * self.record_size

        tty = os.readlink(f"/proc/{console.child.pid}/fd/0")
        identifier = {
            "tty": "tty-" + os.fsencode(tty).hex(),
            "pid": f"pid-{console.child.pid}",
            "tmux": "tmux-scenario",
        }[mode]
        self.directory = case.root / "runtime/dirstory" / identifier
        self.history = self.directory / "history.bin"
        self.lock = self.directory / "history.lock"
        assert self.history.is_file(), f"Shell did not initialize {self.history}"

        records = [struct.pack("<Q", len(path)) + path + struct.pack("<Q", len(path)) for path in raw]
        body = b"".join(records[index % 2] for index in range(count))
        self.prepare_pwd(count // 2)
        with self.lock.open("rb") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            with self.history.open("r+b") as history:
                history.write(self.header(count // 2))
                history.write(body)
                history.truncate(self.end)

        self.validate_snapshot()

    def offset(self, index):
        return 64 + index * self.record_size

    def header(self, index):
        # Format v1: magic, version, dirty, cursor, data end, revision, reserved.
        return struct.pack("<8s7Q", b"DIRSTORY", 1, 0, self.offset(index), self.end, 1, 0, 0)

    def prepare_pwd(self, index):
        path = shlex.quote(str(self.paths[index % 2]))
        if self.case.shell == "fish":
            command = (f"set -g _dirstory_navigating 1; builtin cd {path}; "
                       "set -g _dirstory_last_pwd $PWD; set -g _dirstory_navigating 0")
        else:
            command = (f"_dirstory_navigating=1; builtin cd {path}; "
                       '_dirstory_last_pwd="$PWD"; _dirstory_navigating=0')
        self.console.step(command)

    def validate_snapshot(self):
        output = self.case.root / "fixture-snapshot.txt"
        self.console.step("dirstory internal snapshot > " + shlex.quote(str(output)))
        self.check_status(self.count // 2)

        with output.open("rb") as snapshot:
            for index in range(self.count):
                assert snapshot.readline() == f"1:{self.offset(index)}\n".encode(), index
                assert snapshot.readline() == (b"1\n" if index == self.count // 2 else b"0\n"), index
                assert snapshot.readline() == os.fsencode(self.paths[index % 2]) + b"\n", index
            assert snapshot.read() == b"", "Unexpected snapshot records"
        output.unlink()

    def prepare(self, operation):
        if operation in ("cd", "b"):
            index = self.count - 1
        elif operation == "f":
            index = self.count - 2
        else:
            index = self.count // 2

        self.prepare_pwd(index)
        # The shell is idle at its prompt; use its normal lock and retain both
        # file inodes. Navigation modifies only the header; cd appends one record.
        with self.lock.open("rb") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            with self.history.open("r+b") as history:
                history.write(self.header(index))
                history.truncate(self.end)
        return index

    def command(self, operation, index):
        if operation == "cd":
            return "cd " + shlex.quote(str(self.paths[(index + 1) % 2]))
        if "--list" in operation:
            return operation + " > /dev/null"
        return operation

    def check_status(self, expected_index):
        # Read the previous command's status/PWD after its prompt, outside the
        # timed interval, using the same snapshot protocol as shell E2E tests.
        output = self.console.step(self.case.snapshot(history=False))
        states = self.case.parse_snapshot(output)
        assert len(states) == 1, output
        assert states[0].status == 0, output
        assert states[0].pwd == str(self.paths[expected_index % 2]), output

    def verify(self, operation, index, output):
        expected = {"cd": index + 1, "b": index - 1, "f": index + 1}.get(operation, index)
        self.check_status(expected)
        end = self.end + (self.record_size if operation == "cd" else 0)
        revision = 2 if operation in ("cd", "b", "f") else 1

        with self.history.open("rb") as history:
            header = struct.unpack("<8s7Q", history.read(64))
            history.seek(0, os.SEEK_END)
            assert history.tell() == end, (operation, output)
        assert header == (b"DIRSTORY", 1, 0, self.offset(expected), end, revision, 0, 0), (operation, header, output)


def version(command):
    return subprocess.check_output(command, text=True).splitlines()[0]


def save(report, path):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(report, indent=2) + "\n")
    temporary.replace(path)


def measure(console, fixture, operation):
    samples = []
    for repetition in range(WARMUPS + REPETITIONS):
        index = fixture.prepare(operation)
        command = (fixture.command(operation, index) + "\n").encode()

        started = time.perf_counter_ns()
        os.write(console.master, command)
        output = console.read_prompt()
        elapsed = time.perf_counter_ns() - started

        fixture.verify(operation, index, output)
        if repetition >= WARMUPS:
            samples.append(elapsed)

    return samples


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shell", choices=("bash", "zsh", "fish"), default=os.environ.get("BENCH_SHELL") or None)
    parser.add_argument("--mode", choices=("tty", "pid", "tmux"), default=os.environ.get("BENCH_MODE") or None)
    parser.add_argument("--size", type=int, choices=SIZES, default=int(os.environ["BENCH_SIZE"]) if os.environ.get("BENCH_SIZE") else None)
    args = parser.parse_args()

    matrix = [("tty", count) for count in SIZES] + [("pid", 10_000), ("tmux", 10_000)]
    matrix = [(mode, count) for mode, count in matrix
              if (not args.mode or mode == args.mode) and (not args.size or count == args.size)]
    if not matrix:
        parser.error("The selected mode/size is outside the benchmark matrix")

    binary = Path(BINARY)
    if binary.parent.name != "release":
        parser.error("Set DIRSTORY_BIN to the release binary (or run just bench)")

    output_dir = Path(os.environ.get("CARGO_TARGET_DIR", "target")) / "bench"
    output_dir.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now(timezone.utc)
    output = output_dir / ("baseline-" + timestamp.strftime("%Y%m%dT%H%M%S%fZ") + ".json")
    report = {
        "started_utc": timestamp.isoformat(), "complete": False,
        "metric": "total PTY response including prompt handling; warm caches; no baseline subtraction",
        "p95_method": "nearest rank (ceil(0.95 * n))",
        "warmups": WARMUPS, "repetitions": REPETITIONS,
        "rust_version": version(["rustc", "--version"]),
        "binary": str(binary), "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "fish_initialization": "generated integration sourced from a file to retain PTY stdin in tty mode",
        "shell_lifetime": "fresh shell per mode/size; kept alive for all warmups and samples",
        "runtime_policy": "isolated temporary directories; removed after each shell finishes",
        "results": [],
    }
    save(report, output)
    print(f"Results: {output}", flush=True)
    print(f"{'shell':<6} {'mode':<5} {'visits':>7} {'command':<14} {'median ms':>10} {'p95 ms':>10}", flush=True)

    for shell in ([args.shell] if args.shell else ("bash", "zsh", "fish")):
        case = BenchmarkCase()
        case.shell = shell
        case.setUp()
        try:
            shell_version = version([shell, "--version"])
            for mode, count in matrix:
                case.configure(mode)
                console = Console(case)
                try:
                    fixture = Fixture(case, console, mode, count)
                    for operation in OPERATIONS:
                        samples = measure(console, fixture, operation)
                        median = statistics.median(samples) / 1_000_000
                        p95 = sorted(samples)[math.ceil(0.95 * len(samples)) - 1] / 1_000_000
                        result = {
                            "shell": shell, "shell_version": shell_version,
                            "mode": mode, "visits": count, "command": operation,
                            "runtime_directory": str(fixture.directory.resolve()),
                            "history_file": str(fixture.history.resolve()),
                            "lock_file": str(fixture.lock.resolve()),
                            "path_bytes": fixture.record_size - 16,
                            "median_ms": median, "p95_ms": p95, "samples_ns": samples,
                        }
                        report["results"].append(result)
                        save(report, output)
                        print(f"{shell:<6} {mode:<5} {count:>7} {operation:<14} {median:>10.3f} {p95:>10.3f}", flush=True)
                finally:
                    console.close()
        finally:
            case.doCleanups()

    report["complete"] = True
    report["finished_utc"] = datetime.now(timezone.utc).isoformat()
    save(report, output)
    print(f"Saved {len(report['results'])} scenarios and their raw samples to {output}", flush=True)


if __name__ == "__main__":
    main()
