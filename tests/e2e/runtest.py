#!/usr/bin/env python3
"""Named, isolated shell scenarios with bounded process-group lifetimes."""

import contextlib
from dataclasses import dataclass
import json
import math
import os
from pathlib import Path
import pty
import selectors
import shlex
import shutil
import signal
import subprocess
import tempfile
import time
import unittest

SHELLS = ("sh", "dash", "bash", "zsh", "fish")
MARKER = "__DIRSTORY_SNAPSHOT__"
try:
    TIMEOUT = float(os.environ.get("E2E_TIMEOUT", "10"))
except ValueError as error:
    raise SystemExit("E2E_TIMEOUT must be a positive number") from error
if not math.isfinite(TIMEOUT) or TIMEOUT <= 0:
    raise SystemExit("E2E_TIMEOUT must be a finite positive number")
BINARY = shutil.which(os.environ.get("DIRSTORY_BIN", "dirstory"))
if not BINARY:
    raise SystemExit("Build dirstory and set DIRSTORY_BIN")
BINARY = str(Path(BINARY).resolve())


@dataclass
class State:
    """Observable command result, independent of the history file encoding."""

    status: int
    pwd: str
    back: list[str]
    forward: list[str]


def parse_states(output):
    """Extract snapshots while allowing native cd output before each marker."""
    states = []
    for chunk in output.split(MARKER + "\n")[1:]:
        header, rest = chunk.split("\nBACK\n", 1)
        back, rest = rest.split("FORWARD\n", 1)
        forward, _ = rest.split("END\n", 1)
        status, pwd = header.split("\n", 1)
        states.append(State(int(status), pwd, back.splitlines(), forward.splitlines()))
    return states


class ShellCase(unittest.TestCase):
    """One shell and scenario, with its own configuration and runtime files."""

    shell = ""

    def setUp(self):
        if not shutil.which(self.shell):
            if os.environ.get("DIRSTORY_REQUIRE_SHELLS") == "1":
                self.fail(f"Required shell unavailable: {self.shell}")
            self.skipTest(f"Shell unavailable: {self.shell}")
        self.temp = tempfile.TemporaryDirectory(prefix="dirstory-e2e-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("a", "b", "c", "home", "config", "runtime", "target"):
            (self.root / name).mkdir()
        (self.root / "link").symlink_to(self.root / "target", target_is_directory=True)
        self.env = dict(os.environ)
        self.env.update(
            PATH=str(Path(BINARY).parent) + os.pathsep + os.environ.get("PATH", ""),
            HOME=str(self.root / "home"),
            XDG_CONFIG_HOME=str(self.root / "config"),
            XDG_RUNTIME_DIR=str(self.root / "runtime"),
            TMUX_PANE="%scenario",
        )
        # Do not let startup scripts or cd search paths affect the reference shell.
        for key in ("BASH_ENV", "ENV", "CDPATH", "ZDOTDIR"):
            self.env.pop(key, None)
        self.configure("tmux")

    def configure(self, mode):
        (self.root / "config/dirstory.json").write_text(
            json.dumps({"mode": mode, "unique_top": True})
        )

    def init(self, name="cd"):
        dialect = "sh" if self.shell == "dash" else self.shell
        cmd = f"dirstory init {dialect} --command {shlex.quote(name)}"
        return f"{cmd} | source" if self.shell == "fish" else f'eval "$({cmd})"'

    def builtin(self):
        return "command cd" if self.shell in ("sh", "dash") else "builtin cd"

    def snapshot(self, history=True):
        if self.shell == "fish":
            save = "set -l result $status"
            status = "$result"
        else:
            save = "result=$?"
            status = '"$result"'
        back = "b -l 999" if history else ":"
        forward = "f -l 999" if history else ":"
        return (
            f"{save}\nprintf '{MARKER}\\n%s\\n%s\\nBACK\\n' {status} \"$PWD\"\n"
            f"{back}\nprintf 'FORWARD\\n'\n{forward}\nprintf 'END\\n'"
        )

    def script(self, commands, initialize=True, history=True, name="cd"):
        lines = [self.init(name)] if initialize else []
        for command in commands:
            lines.extend(("printf '__DIRSTORY_COMMAND__\\n'", command, self.snapshot(history)))
        lines.append("true")  # Keep the parent shell alive through the last child call.
        return "\n".join(lines)

    def run_script(self, commands, initialize=True, history=True, name="cd"):
        script = self.script(commands, initialize, history, name)
        with subprocess.Popen(
            [self.shell, "-c", script], cwd=self.root, env=self.env,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
        ) as child:
            self.addCleanup(self.stop_process, child)
            try:
                out, err = child.communicate(timeout=TIMEOUT)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                out, err = child.communicate()
                self.fail(f"{self.shell} timeout after {TIMEOUT}s\n{script}\n{out!r}\n{err!r}")
        details = f"{self.shell}: exit={child.returncode}\nSCRIPT:\n{script}\nSTDOUT:\n{os.fsdecode(out)}\nSTDERR:\n{os.fsdecode(err)}"
        self.assertEqual(child.returncode, 0, details)
        try:
            states = parse_states(os.fsdecode(out))
        except (ValueError, IndexError) as error:
            self.fail(f"Invalid snapshot: {error}\n{details}")
        self.assertEqual(len(states), len(commands), details)
        self.command_outputs = [chunk.split(MARKER + "\n", 1)[0]
                                for chunk in os.fsdecode(out).split("__DIRSTORY_COMMAND__\n")[1:]]
        return states, details

    def paths(self, *names):
        return [str(self.root / name) if name else str(self.root) for name in names]

    def assert_state(self, state, pwd, back=(), forward=(), status=0, details=""):
        self.assertEqual(state, State(status, self.paths(pwd)[0], self.paths(*back), self.paths(*forward)), details)

    def test_navigation_and_branching(self):
        states, details = self.run_script(["cd a", "cd ../b", "cd ../c", "b 2", "f", "cd ../a", "b", "f"])
        expected = [
            ("a", [""], []), ("b", ["a", ""], []), ("c", ["b", "a", ""], []),
            ("a", [""], ["b", "c"]), ("b", ["a", ""], ["c"]),
            ("a", ["b", "a", ""], []), ("b", ["a", ""], ["a"]),
            ("a", ["b", "a", ""], []),
        ]
        for state, (pwd, back, forward) in zip(states, expected):
            self.assert_state(state, pwd, back, forward, details=details)

    def test_boundaries_zero_and_list_order(self):
        states, details = self.run_script(["b", "f", "cd a", "cd ../b", "b 999", "b 0", "f 999", "f 0", "b -l 0", "f -l 0"])
        self.assertNotEqual(states[0].status, 0, details)
        self.assertNotEqual(states[1].status, 0, details)
        self.assert_state(states[4], "", (), ("a", "b"), details=details)
        self.assertEqual(states[4], states[5], details)
        self.assert_state(states[6], "b", ("a", ""), details=details)
        for state in states[7:]:
            self.assertEqual(state, states[6], details)

    def test_list_limits_and_order(self):
        states, details = self.run_script(["cd a", "cd ../b", "cd ../c", "b -l 0", "b -l 1", "b -l 999", "b 2", "f -l 1", "f -l 999", "b -l"])
        self.assertEqual(self.command_outputs[3], "", details)
        self.assertEqual(self.command_outputs[4], self.paths("b")[0] + "\n", details)
        self.assertEqual(self.command_outputs[5], "".join(path + "\n" for path in self.paths("b", "a", "")), details)
        self.assertEqual(self.command_outputs[7], self.paths("b")[0] + "\n", details)
        self.assertEqual(self.command_outputs[8], "".join(path + "\n" for path in self.paths("b", "c")), details)
        self.assertEqual(self.command_outputs[9], str(self.root) + "\n", details)
        for state in states[3:6]:
            self.assertEqual(state, states[2], details)
        for state in states[7:]:
            self.assertEqual(state, states[6], details)

    def test_failed_cd_preserves_forward_history(self):
        states, details = self.run_script(["cd a", "cd ../b", "b", "cd ../missing"])
        self.assertNotEqual(states[-1].status, 0, details)
        self.assertEqual(states[-1].pwd, states[-2].pwd, details)
        self.assertEqual(states[-1].back, states[-2].back, details)
        self.assertEqual(states[-1].forward, states[-2].forward, details)

    def test_deleted_navigation_target(self):
        states, details = self.run_script(["cd a", "cd ../b", "b", f"rmdir {shlex.quote(str(self.root / 'b'))}", "f"])
        self.assertNotEqual(states[-1].status, 0, details)
        self.assertEqual(states[-1].pwd, states[-2].pwd, details)
        self.assertEqual(states[-1].back, states[-2].back, details)
        self.assertEqual(states[-1].forward, states[-2].forward, details)

    def test_invalid_arguments_preserve_history(self):
        commands = ["cd a", "cd ../b"] + [f"{cmd} {arg}" for cmd in ("b", "f") for arg in ("-1", "nope", "--unknown", "-l nope")]
        states, details = self.run_script(commands)
        for state in states[2:]:
            self.assertNotEqual(state.status, 0, details)
            self.assertEqual((state.pwd, state.back, state.forward), (states[1].pwd, states[1].back, states[1].forward), details)

    def test_same_directory_and_reinitialization(self):
        states, details = self.run_script(["cd a", "cd .", self.init(), "b", "f"])
        self.assertEqual(states[0], states[1], details)
        self.assertEqual(states[0], states[2], details)
        self.assert_state(states[3], "", (), ("a",), details=details)
        self.assertEqual(states[0], states[4], details)

    def test_custom_wrapper(self):
        states, details = self.run_script(["cd a", "cdir ../b", "b", "f"], name="cdir")
        self.assert_state(states[0], "a", details=details)  # Ordinary cd remains unwrapped.
        self.assert_state(states[1], "b", ("a",), details=details)
        self.assert_state(states[2], "a", (), ("b",), details=details)
        self.assertEqual(states[1], states[3], details)

    def test_native_cd_arguments_and_symlinks(self):
        args = ["a", "../b", "", "-", "-L " + shlex.quote(str(self.root / "link")), "-P " + shlex.quote(str(self.root / "link")), "../missing", "--bad-option"]
        reference, _ = self.run_script([f"{self.builtin()} {arg}" for arg in args], initialize=False, history=False)
        actual, details = self.run_script([f"cd {arg}" for arg in args])
        visits = [str(self.root)]
        for expected, observed in zip(reference, actual):
            self.assertEqual((observed.status, observed.pwd), (expected.status, expected.pwd), details)
            if expected.status == 0 and visits[-1] != expected.pwd:
                visits.append(expected.pwd)
            self.assertEqual(observed.back, list(reversed(visits[:-1])), details)
            self.assertEqual(observed.forward, [], details)

    def test_literal_path_characters_do_not_execute(self):
        names = ["a space", "žluťoučký", "quote'and\"double", "$(touch INJECTED)", ";touch INJECTED", "star*question?"]
        for name in names:
            (self.root / name).mkdir()
        commands = ["cd " + shlex.quote(str(self.root / name)) for name in names] + ["b", "f"]
        states, details = self.run_script(commands)
        for index, state in enumerate(states[:len(names)]):
            self.assert_state(state, names[index], list(reversed([""] + names[:index])), details=details)
        self.assertFalse(list(self.root.rglob("INJECTED")), details)
        self.assertEqual(states[-1], states[len(names)-1], details)

    def test_help_preserves_history(self):
        states, details = self.run_script(["cd a", "b --help", "f --help"])
        self.assertEqual(states[0], states[1], details)
        self.assertEqual(states[0], states[2], details)

    def session_isolation(self, mode):
        """Keep two parents alive while navigating separate histories in PTYs."""
        self.configure(mode)
        with contextlib.ExitStack() as stack:
            processes = []
            for name in ("a", "b"):
                master, slave = pty.openpty()
                stack.callback(os.close, master)
                stack.callback(os.close, slave)
                read = "read -l go" if self.shell == "fish" else "IFS= read -r go"
                script = self.init() + f"\ncd {name}\n" + self.snapshot() + "\nprintf '__READY__\\n'\n" + read + "\nb\n" + self.snapshot() + "\nf\n" + self.snapshot() + "\ntrue"
                child = subprocess.Popen([self.shell, "-c", script], cwd=self.root, env=self.env,
                    stdin=slave, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
                stack.callback(self.stop_process, child)
                processes.append((child, master, name, script))
            outputs = {}
            with selectors.DefaultSelector() as selector:
                for child, _, _, _ in processes:
                    selector.register(child.stdout, selectors.EVENT_READ, child.pid)
                    outputs[child.pid] = b""
                deadline = time.monotonic() + TIMEOUT
                while selector.get_map():
                    remaining = deadline - time.monotonic()
                    self.assertGreater(remaining, 0, "Parent shell readiness timed out")
                    for key, _ in selector.select(remaining):
                        data = os.read(key.fd, 65536)
                        self.assertTrue(data, f"Parent {key.data} exited before readiness")
                        outputs[key.data] += data
                        if b"__READY__\n" in outputs[key.data]:
                            selector.unregister(key.fileobj)
            files = list((self.root / "runtime/dirstory").glob("*/history.bin"))
            self.assertEqual(len(files), 2, f"{mode} must separate the two live shells")
            for child, master, name, script in processes:
                os.write(master, b"go\n")
                try:
                    out, err = child.communicate(timeout=TIMEOUT)
                except subprocess.TimeoutExpired:
                    self.fail(f"{mode}/{self.shell} navigation timed out\n{script}")
                details = f"{mode}/{self.shell}\n{script}\n{out!r}\n{err!r}"
                self.assertEqual(child.returncode, 0, details)
                states = parse_states(os.fsdecode(outputs[child.pid] + out))
                self.assertEqual(len(states), 3, details)
                self.assert_state(states[0], name, ("",), details=details)
                self.assert_state(states[1], "", (), (name,), details=details)
                self.assertEqual(states[0], states[2], details)

    @staticmethod
    def stop_process(child):
        # Descendants may outlive an already-exited parent shell.
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.communicate(timeout=TIMEOUT)

    def test_tty_session_isolation(self):
        self.session_isolation("tty")

    def test_ppid_session_isolation(self):
        self.session_isolation("ppid")

    def test_tmux_session_isolation(self):
        for pane, path in (("%first", "/first"), ("%second", "/second")):
            env = dict(self.env, TMUX_PANE=pane)
            subprocess.run([BINARY, "internal", "ensure", path], env=env, check=True, timeout=TIMEOUT)
        outputs = []
        for pane in ("%first", "%second"):
            result = subprocess.run([BINARY, "internal", "select", "back", "0"], env=dict(self.env, TMUX_PANE=pane), check=True, capture_output=True, timeout=TIMEOUT)
            outputs.append(result.stdout.splitlines()[1])
        self.assertEqual(outputs, [b"/first", b"/second"])


def main():
    """Build named tests without collecting the abstract shell fixture itself."""
    selected = os.environ.get("E2E_SHELL", "")
    if selected and selected not in SHELLS:
        raise SystemExit(f"E2E_SHELL must be one of {SHELLS}")
    pattern = os.environ.get("E2E_FILTER", "")
    suite = unittest.TestSuite()
    for shell in SHELLS:
        if selected and shell != selected:
            continue
        case = type(shell, (ShellCase,), {"shell": shell})
        for name in unittest.defaultTestLoader.getTestCaseNames(case):
            if not pattern or pattern in f"{shell}/{name}":
                suite.addTest(case(name))
    if suite.countTestCases() == 0:
        raise SystemExit("No E2E scenarios match the filters")
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())


if __name__ == "__main__":
    main()
