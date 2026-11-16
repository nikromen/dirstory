"""Controlling-terminal tests with protocol output captured separately."""

import contextlib
import fcntl
import os
import pty
import selectors
import signal
import struct
import subprocess
import termios
import time


class PickerScenarios:
    """Exercise the real picker and the shared shell navigation protocol."""

    @contextlib.contextmanager
    def picker_process(self, script, size=(24, 80)):
        master, slave = pty.openpty()
        before = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", *size, 0, 0))

        def controlling_terminal():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        child = subprocess.Popen(
            [self.shell, "-c", script], cwd=self.root, env=self.env,
            stdin=slave, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            preexec_fn=controlling_terminal,
        )
        try:
            yield child, master, slave
            self.assertEqual(termios.tcgetattr(slave), before, "terminal flags leaked")
        finally:
            self.stop_process(child)
            os.close(slave)
            os.close(master)

    def terminal_until(self, master, needle):
        output = b""
        deadline = time.monotonic() + self.timeout
        with selectors.DefaultSelector() as selector:
            selector.register(master, selectors.EVENT_READ)
            while needle not in output:
                remaining = deadline - time.monotonic()
                self.assertGreater(remaining, 0, repr(output))
                self.assertTrue(selector.select(remaining), repr(output))
                output += os.read(master, 65536)
        return output

    def test_picker_shell_commands(self):
        for command in ("b -i", "f -i", "b --interactive", "f --interactive", "dh"):
            for keys, destination in ((b"\x1b[A\r", "a"), (b"\r", "b"),
                                      (b"\x1b", "b"), (b"\x03", "b"), (b"q", "b")):
                with self.subTest(command=command, keys=keys):
                    script = self.init() + "\ncd a\ncd ../b\n" + command + "\n" + self.snapshot()
                    script += "\ncd ../c\n" + self.snapshot() + "\ntrue"
                    with self.picker_process(script) as (child, master, _):
                        self.terminal_until(master, b"Directory")
                        os.write(master, keys)
                        out, err = child.communicate(timeout=self.timeout)
                        self.assertEqual(child.returncode, 0, err)
                        self.assertNotIn(b"\x1b", out, out)
                        states = self.parse_snapshot(os.fsdecode(out))
                        self.assertEqual(states[0].status, 0, err)
                        self.assertEqual(states[0].pwd, str(self.root / destination), err)
                        self.assertEqual(states[1].pwd, str(self.root / "c"), err)
                        self.assertEqual(states[1].back[0], str(self.root / destination), err)
                        self.assertEqual(states[1].forward, [], err)

    def test_picker_unavailable_path(self):
        script = self.init() + "\ncd a\ncd ../b\n" + "dh\n" + self.snapshot() + "\ncd ../c\n" + self.snapshot() + "\ntrue"
        with self.picker_process(script) as (child, master, _):
            self.terminal_until(master, b"Directory")
            (self.root / "a").rmdir()
            os.write(master, b"\x1b[A\r")
            out, err = child.communicate(timeout=self.timeout)
            states = self.parse_snapshot(os.fsdecode(out))
            self.assertEqual(states[0].status, 1, err)
            self.assertEqual(states[0].pwd, str(self.root / "b"), err)
            self.assertEqual(states[0].back[0], str(self.root / "a"), err)
            self.assertEqual(states[1].back[0], str(self.root / "b"), err)

    def test_picker_resize_and_small_terminal(self):
        script = self.init() + "\ncd a\ncd ../b\ndh\n" + self.snapshot() + "\ntrue"
        with self.picker_process(script, (3, 19)) as (child, master, slave):
            self.terminal_until(master, b"Enlarge")
            os.write(master, b"\r\x1b[A")
            time.sleep(0.05)
            self.assertIsNone(child.poll(), "Enter accepted a hidden selection")
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
            os.killpg(child.pid, signal.SIGWINCH)
            self.terminal_until(master, b"Directory")
            os.write(master, b"\r")
            out, err = child.communicate(timeout=self.timeout)
            states = self.parse_snapshot(os.fsdecode(out))
            self.assertEqual(states[0].pwd, str(self.root / "a"), err)
        with self.picker_process(script, (3, 19)) as (child, master, _):
            self.terminal_until(master, b"Enlarge")
            os.write(master, b"\x03")
            child.communicate(timeout=self.timeout)

    def test_picker_concurrent_writer_rejects_stale_token(self):
        script = self.init() + "\ncd a\ncd ../b\ndh\n" + self.snapshot(history=False) + "\ntrue"
        with self.picker_process(script) as (child, master, _):
            self.terminal_until(master, b"Directory")
            result = subprocess.run(
                ["dirstory", "internal", "visit", "--from", str(self.root / "b"), "--to", str(self.root / "c")],
                env=self.env, capture_output=True, timeout=self.timeout,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            history = self.root / "runtime/dirstory/tmux-scenario/history.bin"
            newer = history.read_bytes()
            os.write(master, b"\x1b[A\r")
            out, err = child.communicate(timeout=self.timeout)
            states = self.parse_snapshot(os.fsdecode(out))
            self.assertEqual(states[0].status, 1, err)
            self.assertEqual(states[0].pwd, str(self.root / "a"), err)
            self.assertEqual(history.read_bytes(), newer)

    def test_picker_protocol_and_no_terminal(self):
        script = self.init() + "\ncd a\ndirstory internal pick\ntrue"
        with self.picker_process(script) as (child, master, _):
            self.terminal_until(master, b"Directory")
            os.write(master, b"\r")
            out, err = child.communicate(timeout=self.timeout)
            token, path = out.splitlines()
            self.assertRegex(token, rb"^[0-9]+:[0-9]+$")
            self.assertEqual(path, os.fsencode(self.root / "a"), err)
        with self.picker_process(script) as (child, master, _):
            self.terminal_until(master, b"Directory")
            os.write(master, b"\x1b")
            out, err = child.communicate(timeout=self.timeout)
            self.assertEqual(out, b"", err)
        result = subprocess.run([self.shell, "-c", script], cwd=self.root,
                                env=self.env, capture_output=True, start_new_session=True,
                                timeout=self.timeout)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"controlling terminal", result.stderr)

    def test_picker_preserves_raw_path_bytes(self):
        path = os.fsencode(self.root) + b"/raw-\xff-\t-\x1b[31m"
        subprocess.run([b"dirstory", b"internal", b"reset", path],
                       env=self.env, check=True, timeout=self.timeout)
        with self.picker_process("dirstory internal pick") as (child, master, _):
            screen = self.terminal_until(master, b"Directory")
            os.write(master, b"\r")
            out, err = child.communicate(timeout=self.timeout)
            self.assertEqual(out.splitlines()[1], path, err)
            # Display escapes controls, while the protocol keeps the bytes.
            self.assertNotIn(b"\x1b[31m", screen)

    def test_picker_arguments_before_backend(self):
        commands = ["dh x", "dh -i", "dh --help x", "b -i 2", "f --interactive -l", "b -l -i"]
        states, details = self.run_script(commands + ["dh -h", "dh --help"], initialize=True)
        for state in states[:len(commands)]:
            self.assertEqual(state.status, 2, details)
        for state in states[len(commands):]:
            self.assertEqual(state.status, 0, details)
