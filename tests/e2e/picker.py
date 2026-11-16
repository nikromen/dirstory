"""Controlling-terminal tests with protocol output captured separately."""

import codecs
import contextlib
import fcntl
import os
import pty
import re
import selectors
import signal
import struct
import subprocess
import termios
import time


class Screen:
    """Track Ratatui's cursor-addressed output, including incremental redraws."""

    def __init__(self, rows=24, columns=80):
        self.cells = [[" "] * columns for _ in range(rows)]
        self.row = self.column = 0
        self.decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")
        self.pending = ""

    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            if self.pending.startswith("\x1b"):
                match = re.match(r"\x1b\[([0-?]*)([ -/]*)([@-~])", self.pending)
                if not match:
                    return
                parameters, _, code = match.groups()
                self.pending = self.pending[match.end():]
                if code in ("H", "f"):
                    values = [int(v or "1") for v in parameters.split(";")]
                    self.row = values[0] - 1
                    self.column = (values[1] if len(values) > 1 else 1) - 1
                elif code == "J" and parameters in ("2", "3"):
                    self.cells = [[" "] * len(row) for row in self.cells]
                elif code == "K":
                    self.cells[self.row][self.column:] = [" "] * (len(self.cells[self.row]) - self.column)
                continue

            char, self.pending = self.pending[0], self.pending[1:]
            if char == "\r":
                self.column = 0
            elif char == "\n":
                self.row += 1
            elif 0 <= self.row < len(self.cells) and 0 <= self.column < len(self.cells[0]):
                self.cells[self.row][self.column] = char
                self.column += 1

    def text(self):
        return "\n".join("".join(row) for row in self.cells)


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

    def screen_until(self, master, screen, predicate):
        deadline = time.monotonic() + self.timeout
        with selectors.DefaultSelector() as selector:
            selector.register(master, selectors.EVENT_READ)
            while not predicate(screen.text()):
                remaining = deadline - time.monotonic()
                self.assertGreater(remaining, 0, screen.text())
                self.assertTrue(selector.select(remaining), screen.text())
                data = os.read(master, 65536)
                self.assertTrue(data, screen.text())
                screen.feed(data)
        return screen.text()

    def test_picker_fuzzy_typing_editing_and_confirmation(self):
        for name in ("qalpha-confirmation", "qbeta"):
            (self.root / name).mkdir()
        q_count = "3/3" if "q" in str(self.root).lower() else "2/3"
        script = self.init() + "\ncd qalpha-confirmation\ncd ../qbeta\ndh\n" + self.snapshot()
        script += "\ncd ../c\n" + self.snapshot() + "\ntrue"

        with self.picker_process(script) as (child, master, _):
            screen = Screen()
            self.screen_until(master, screen, lambda text: "Directory history" in text and "3/3" in text)
            os.write(master, b"q")
            self.screen_until(master, screen, lambda text: "Search: q" in text and q_count in text)
            self.assertIsNone(child.poll(), "q canceled instead of becoming search text")

            os.write(master, "ž".encode())
            self.screen_until(master, screen, lambda text: "Search: qž" in text and "No matches" in text)
            os.write(master, b"\r\x7f")
            self.screen_until(master, screen, lambda text: "Search: q " in text and q_count in text)
            self.assertIsNone(child.poll(), "Enter confirmed an empty result set")

            os.write(master, b"alpha-confirmationZ")
            self.screen_until(master, screen, lambda text: "Search: qalpha-confirmationZ" in text and "0/3" in text)
            os.write(master, b"\x7f")
            self.screen_until(master, screen, lambda text: "Search: qalpha-confirmation " in text and "1/3" in text and "qalpha-confirmation" in text)

            os.write(master, b"\x15")
            self.screen_until(master, screen, lambda text: "3/3" in text and "BACK" in text)
            os.write(master, b"qalpha-confirmation")
            self.screen_until(master, screen, lambda text: "Search: qalpha-confirmation" in text and "1/3" in text)
            os.write(master, b"\r")
            out, err = child.communicate(timeout=self.timeout)
            states = self.parse_snapshot(os.fsdecode(out))
            self.assertEqual(states[0].status, 0, err)
            self.assertEqual(states[0].pwd, str(self.root / "qalpha-confirmation"), err)
            self.assertEqual(states[0].forward, [str(self.root / "qbeta")], err)
            self.assertEqual(states[1].back[0], str(self.root / "qalpha-confirmation"), err)
            self.assertNotIn(b"\x1b", out)

    def test_picker_cancel_after_typing(self):
        for keys in (b"\x1b", b"\x03"):
            script = self.init() + "\ncd a\ndh\n" + self.snapshot() + "\ntrue"
            with self.picker_process(script) as (child, master, _):
                screen = Screen()
                self.screen_until(master, screen, lambda text: "Directory history" in text)
                os.write(master, b"no-matching-visit")
                self.screen_until(master, screen, lambda text: "No matches" in text)
                os.write(master, keys)
                out, err = child.communicate(timeout=self.timeout)
                states = self.parse_snapshot(os.fsdecode(out))
                self.assertEqual(states[0].status, 0, err)
                self.assertEqual(states[0].pwd, str(self.root / "a"), err)

    def test_picker_shell_commands(self):
        for command in ("b -i", "f -i", "b --interactive", "f --interactive", "dh"):
            for keys, destination in ((b"\x1b[A\r", "a"), (b"\r", "b"),
                                      (b"\x1b", "b"), (b"\x03", "b")):
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
            # Drain through the footer so a full redraw cannot fill the PTY
            # while communicate() waits only on the separate protocol pipes.
            self.screen_until(master, Screen(), lambda text: "Directory history" in text and "Esc: cancel" in text)
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
