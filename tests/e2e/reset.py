"""Public reset and recovery through the existing shell adapters."""

import os
import selectors
import shlex
import subprocess
import time

from hooks import Console
from picker import Screen


class ResetScenarios:
    """Keep recovery independent of synchronization and preserve session ownership."""

    def reset_history(self):
        return self.root / "runtime/dirstory/tmux-scenario/history.bin"

    def test_reset_discards_both_directions_and_stales_tokens(self):
        token = '(head -n 1 token)' if self.shell == "fish" else '"$(head -n 1 token)"'
        states, details = self.run_script([
            "cd a", "cd ../b", "b", "dirstory internal select back 0 > token",
            "dh --reset", f"dirstory internal commit {token}", "cd ../c", "b", "f",
        ])
        self.assert_state(states[2], "a", ("",), ("b",), details=details)
        self.assert_state(states[4], "a", details=details)
        self.assertEqual(self.command_outputs[4], "", details)
        self.assert_state(states[5], "a", status=1, details=details)
        self.assert_state(states[6], "c", ("a",), details=details)
        self.assert_state(states[7], "a", (), ("c",), details=details)
        self.assertEqual(states[8], states[6], details)

    def test_reset_recovery_missing_corrupt_and_interrupted_history(self):
        other_env = dict(self.env, TMUX_PANE="%other")
        subprocess.run(["dirstory", "internal", "ensure", "/other"], env=other_env,
                       check=True, timeout=self.timeout)
        other = self.root / "runtime/dirstory/tmux-other/history.bin"
        untouched = other.read_bytes()

        for damage in ("missing", "corrupt", "interrupted"):
            with self.subTest(damage=damage):
                read = "read -l go" if self.shell == "fish" else "IFS= read -r go"
                script = self.init() + "\ncd a\nprintf '__RESET_READY__\\n'\n" + read
                if damage != "missing":
                    script += "\ncd ../b\n" + self.snapshot(history=False)
                script += "\ndh --reset\n" + self.snapshot() + "\ndh\ncd ../c\nb\n" + self.snapshot() + "\ntrue"

                with self.picker_process(script) as (child, master, _):
                    ready = b""
                    deadline = time.monotonic() + self.timeout
                    with selectors.DefaultSelector() as selector:
                        selector.register(child.stdout, selectors.EVENT_READ)
                        while b"__RESET_READY__\n" not in ready:
                            remaining = deadline - time.monotonic()
                            self.assertGreater(remaining, 0, ready)
                            self.assertTrue(selector.select(remaining), ready)
                            data = os.read(child.stdout.fileno(), 65536)
                            self.assertTrue(data, ready)
                            ready += data

                    history = self.reset_history()
                    lock = history.with_name("history.lock")
                    inode = lock.stat().st_ino
                    if damage == "missing":
                        history.unlink()
                    elif damage == "corrupt":
                        history.write_bytes(b"broken header")
                    else:
                        data = bytearray(history.read_bytes())
                        data[16:24] = (1).to_bytes(8, "little")
                        history.write_bytes(data)

                    # The documented optional diagnostic copy is made while the
                    # owning shell is paused and no other writer is active.
                    if history.exists():
                        (self.root / "damaged.bin").write_bytes(history.read_bytes())

                    os.write(master, b"go\n")
                    screen = Screen()
                    self.screen_until(master, screen, lambda text: "Directory history" in text and "1/1" in text)
                    os.write(master, b"\x1b")
                    out, err = child.communicate(timeout=self.timeout)
                    states = self.parse_snapshot(os.fsdecode(ready + out))
                    if damage == "missing":
                        initial, reset_index = "a", 0
                    else:
                        initial, reset_index = "b", 1
                        # Native hooks preserve the successful cd status even
                        # when recording fails; wrappers report that failure.
                        expected_status = 0 if self.shell in ("zsh", "fish") else 1
                        self.assertEqual(states[0].status, expected_status, err)
                        self.assertEqual(states[0].pwd, str(self.root / "b"), err)
                        self.assertIn(b"dh --reset", err)

                    self.assert_state(states[reset_index], initial, details=err)
                    self.assert_state(states[reset_index + 1], initial, (), ("c",), details=err)
                    self.assertEqual(lock.stat().st_ino, inode)
                    self.assertEqual(other.read_bytes(), untouched)

    def reset_unobserved_change(self):
        if self.shell == "zsh":
            return "builtin cd -q a"
        if self.shell == "fish":
            return "functions -e _dirstory_observe\nbuiltin cd a"
        return self.builtin() + " a"

    def test_reset_uses_unobserved_pwd_without_old_visits(self):
        states, details = self.run_script([
            self.reset_unobserved_change() + "\ndh --reset", self.init(), "cd ../b", "b",
        ])
        self.assert_state(states[0], "a", details=details)
        self.assertEqual(self.command_outputs[0], "", details)
        self.assert_state(states[1], "a", details=details)
        self.assert_state(states[2], "b", ("a",), details=details)
        self.assert_state(states[3], "a", (), ("b",), details=details)

    def test_reset_prompt_does_not_reintroduce_old_visits(self):
        if self.shell not in ("bash", "zsh", "fish"):
            self.skipTest("sh has no prompt integration")

        console = Console(self)
        console.step(self.reset_unobserved_change() + "; dh --reset")
        state, details = self.backend_snapshot(console)
        self.assert_state(state, "a", details=details)
        console.step(self.init())
        console.step("cd ../b")
        console.step("b")
        state, details = self.backend_snapshot(console)
        self.assert_state(state, "a", (), ("b",), details=details)

    def test_reset_invalid_arguments_and_help_never_call_backend(self):
        if self.shell == "fish":
            spy = "function dirstory; printf 'called\\n' >> calls; return 1; end"
            pending = "set -g _dirstory_last_pwd /pending"
        else:
            spy = "dirstory() { printf 'called\\n' >> calls; return 1; }"
            pending = "_dirstory_last_pwd=/pending"

        invalid = ["dh --reset 1", "dh --reset query", "dh --reset --help", "dh --help --reset",
                   "dh --reset -i", "dh --reset --reset", "b --reset", "f --reset"]
        states, details = self.run_script([spy, pending] + invalid + ["dh --help"], history=False)
        self.assertEqual([s.status for s in states[2:-1]], [2] * len(invalid), details)
        self.assertEqual(states[-1].status, 0, details)
        self.assertFalse((self.root / "calls").exists(), details)
        self.assertIn("cannot be recovered", self.command_outputs[-1], details)
        self.assertIn("all shells sharing", self.command_outputs[-1], details)

    def test_reset_failure_preserves_integration_and_history(self):
        self.run_script(["cd a"])
        before = self.reset_history().read_bytes()
        if self.shell == "fish":
            pending = "set -g _dirstory_last_pwd /pending"
            spy = "function dirstory; printf 'reset failed\\n' >&2; return 42; end"
            check = 'test "$_dirstory_last_pwd" = /pending'
        else:
            pending = "_dirstory_last_pwd=/pending"
            spy = "dirstory() { printf 'reset failed\\n' >&2; return 42; }"
            check = 'test "$_dirstory_last_pwd" = /pending'

        states, details = self.run_script([pending, spy, "dh --reset", check], history=False)
        self.assertEqual(states[2].status, 1, details)
        self.assertEqual(states[3].status, 0, details)
        self.assertEqual(self.command_outputs[2], "", details)
        self.assertEqual(self.reset_history().read_bytes(), before)

    def test_reset_configuration_error_preserves_state(self):
        self.run_script(["cd a"])
        before = self.reset_history().read_bytes()
        pending = "set -g _dirstory_last_pwd /pending" if self.shell == "fish" else "_dirstory_last_pwd=/pending"
        (self.root / "config/dirstory.json").write_text("{invalid json")

        states, details = self.run_script([
            pending, "dh --reset", 'test "$_dirstory_last_pwd" = /pending',
        ], history=False)
        self.assertEqual(states[1].status, 1, details)
        self.assertEqual(states[2].status, 0, details)
        self.assertEqual(self.command_outputs[1], "", details)
        self.assertEqual(self.reset_history().read_bytes(), before)

    def test_reset_subshell_cannot_change_owner_history(self):
        if self.shell in ("sh", "dash"):
            self.skipTest("sh retains its existing wrapper-only integration model")

        history = shlex.quote(str(self.reset_history()))
        if self.shell == "fish":
            # Fish substitutions run builtins in the owning process. Use a real
            # child process inheriting the same functions and owner identity.
            commands = [
                "functions _dirstory_parent _dirstory_parse _dirstory_navigate dh > child.fish",
                "printf 'dh --reset\\n' >> child.fish",
                "env _dirstory_owner=$fish_pid fish --no-config child.fish",
            ]
        else:
            commands = ["(dh --reset)", 'ignored=$(dh --reset)']

        states, details = self.run_script(["cd a", "cd ../b", "b", f"cp {history} {shlex.quote(str(self.root / 'before.bin'))}"] + commands)
        for state in states[-(1 if self.shell == "fish" else 2):]:
            self.assert_state(state, "a", ("",), ("b",), status=1, details=details)
        self.assertEqual(self.reset_history().read_bytes(), (self.root / "before.bin").read_bytes())

    def test_reset_preserves_symlink_and_special_pwd(self):
        special = "quote' ž $(touch INJECTED); *"
        (self.root / special).mkdir()
        for name in ("link", special):
            with self.subTest(path=name):
                states, details = self.run_script([
                    "cd " + shlex.quote(str(self.root / name)), "dh --reset",
                    "cd " + shlex.quote(str(self.root / "b")), "b",
                ])
                self.assert_state(states[1], name, details=details)
                self.assertEqual(self.command_outputs[1], "", details)
                self.assert_state(states[2], "b", (name,), details=details)
                self.assert_state(states[3], name, (), ("b",), details=details)
                self.assertFalse((self.root / "INJECTED").exists())

    def test_reset_rejects_lf_without_mutation(self):
        self.run_script([])
        before = self.reset_history().read_bytes()
        path = self.root / "bad\npath"
        path.mkdir()
        states, details = self.run_script([
            self.builtin() + " " + shlex.quote(str(path)), "dh --reset",
        ], history=False)
        self.assertEqual(states[1].pwd, str(path), details)
        self.assertEqual(states[1].status, 1, details)
        self.assertEqual(self.command_outputs[1], "", details)
        self.assertEqual(self.reset_history().read_bytes(), before)
