"""Hook scenarios, including real interactive prompts driven through a PTY."""

import contextlib
import errno
import fcntl
import os
import selectors
import subprocess
import termios
import time


class Console:
    """One interactive shell with deterministic prompts and bounded reads."""

    def __init__(self, case, setup=""):
        self.case = case
        self.master, slave = os.openpty()
        attributes = termios.tcgetattr(slave)
        attributes[3] &= ~termios.ECHO
        termios.tcsetattr(slave, termios.TCSANOW, attributes)
        args = {
            "bash": ["bash", "--noprofile", "--norc", "-i"],
            "zsh": ["zsh", "-f", "-i"],
            "fish": ["fish", "--no-config", "-i", "-C",
                "function fish_prompt; end; function _test_ready --on-event fish_prompt; printf '\\n__READY__\\n'; end"],
        }[case.shell]
        env = dict(case.env, TERM="dumb", PS1="__READY__\n", RPS1="")
        self.child = subprocess.Popen(args, cwd=case.root, env=env,
            stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
            preexec_fn=lambda: fcntl.ioctl(0, termios.TIOCSCTTY, 0))
        os.close(slave)
        case.addCleanup(self.close)
        if case.shell == "zsh":
            os.write(self.master, b"unsetopt zle; PS1=$'__READY__\\n'; RPS1=''\n")
        self.read_prompt()
        self.step(setup + "; " + case.init() if setup else case.init())

    def close(self):
        if getattr(self, "closed", False):
            return
        self.closed = True
        self.case.stop_process(self.child)
        with contextlib.suppress(OSError):
            os.close(self.master)

    def read_prompt(self):
        """Read until a prompt, never depending on a fixed sleep."""
        deadline = time.monotonic() + self.case.timeout
        output = b""
        with selectors.DefaultSelector() as selector:
            selector.register(self.master, selectors.EVENT_READ)
            while b"__READY__\n" not in output.replace(b"\r\n", b"\n"):
                remaining = deadline - time.monotonic()
                self.case.assertGreater(remaining, 0, f"Prompt timeout: {output!r}")
                self.case.assertTrue(selector.select(remaining), f"Prompt timeout: {output!r}")
                try:
                    data = os.read(self.master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        data = b""
                    else:
                        raise
                self.case.assertTrue(data, f"Shell exited: {output!r}")
                output += data
        return output.decode(errors="replace").replace("\r\n", "\n")

    def step(self, command):
        os.write(self.master, (command.replace("\n", "; ") + "\n").encode())
        return self.read_prompt()


class HookScenarios:
    """Mixed into fixtures for Bash, Zsh and Fish only."""

    def test_builtin_changes_and_same_path(self):
        states, details = self.run_script(["builtin cd a", "builtin cd .", "b", "f"])
        self.assert_state(states[0], "a", ("",), details=details)
        self.assertEqual(states[0], states[1], details)
        self.assert_state(states[2], "", (), ("a",), details=details)
        self.assertEqual(states[0], states[3], details)

    def test_subshell_and_command_substitution_are_isolated(self):
        if self.shell == "fish":
            # Fish substitutions execute builtins in this shell, so use another
            # process to exercise a genuine isolated directory change.
            commands = ["fish --no-config -c 'cd a'", "set ignored (fish --no-config -c 'cd a; pwd')"]
        else:
            commands = ["(cd a)", "ignored=$(builtin cd a; pwd)"]
        states, details = self.run_script(commands + ["cd b", "b", "f"])
        for state in states[:2]:
            self.assert_state(state, "", details=details)
        self.assert_state(states[2], "b", ("",), details=details)
        self.assertEqual(states[2], states[-1], details)

    def test_existing_hooks_and_reinitialization(self):
        if self.shell == "fish":
            setup = "function user_hook --on-variable PWD; set -g user_calls (math $user_calls + 1); end; set -g user_calls 0"
            check = "test $user_calls -eq 2"
        elif self.shell == "zsh":
            setup = "user_calls=0; chpwd() { (( user_calls+=1 )); }; user_hook() { :; }; chpwd_functions=(user_hook)"
            check = "test $user_calls -eq 2; test ${#chpwd_functions[@]} -eq 2"
        else:
            setup = ":"
            check = ":"
        states, details = self.run_script([setup, self.init(), "cd a", "cd ../b", check])
        self.assert_state(states[-1], "b", ("a", ""), details=details)

    def test_navigation_failure_clears_suppression(self):
        states, details = self.run_script(["cd a", "cd ../b", "b", "rmdir ../b", "f", "cd ../c", "b"])
        self.assertNotEqual(states[4].status, 0, details)
        self.assert_state(states[5], "c", ("a", ""), details=details)
        self.assert_state(states[6], "a", ("",), ("c",), details=details)

    def test_interactive_prompt_tracks_builtin_change(self):
        console = Console(self)
        console.step("builtin cd a")
        output = console.step(self.snapshot())
        states = self.parse_snapshot(output)
        self.assert_state(states[0], "a", ("",), details=output)
        console.step("b")
        console.step("f")
        output = console.step(self.snapshot())
        self.assert_state(self.parse_snapshot(output)[0], "a", ("",), details=output)

    def test_interactive_prompt_preserves_status(self):
        console = Console(self)
        console.step("false")
        command = "printf '__STATUS__%s\\n' $status" if self.shell == "fish" else "printf '__STATUS__%s\\n' \"$?\""
        output = console.step(command)
        self.assertIn("__STATUS__1\n", output)

    def test_unchanged_observation_does_not_spawn_backend(self):
        log = str(self.root / "calls")
        if self.shell == "fish":
            spy = f"function dirstory; printf '%s\\n' $argv[2] >> '{log}'; command dirstory $argv; end"
            observe = "_dirstory_prompt"
        else:
            spy = f"dirstory() {{ printf '%s\\n' \"$2\" >> '{log}'; command dirstory \"$@\"; }}"
            observe = "_dirstory_observe"
        self.run_script([spy, observe, observe, "cd a", observe, observe, "b", observe, "f", observe])
        calls = (self.root / "calls").read_text().splitlines()
        self.assertEqual(calls.count("visit"), 1, calls)
        self.assertEqual(calls.count("commit"), 2, calls)

    def test_failed_visit_keeps_observation_retryable(self):
        if self.shell == "fish":
            spy = "function dirstory; if test \"$argv[2]\" = visit; return 1; end; command dirstory $argv; end"
            remove = "functions -e dirstory"
            observe = "_dirstory_prompt"
            change = "builtin cd a"
        else:
            spy = 'dirstory() { if [ "$2" = visit ]; then return 1; fi; command dirstory "$@"; }'
            remove = "unfunction dirstory" if self.shell == "zsh" else "unset -f dirstory"
            observe = "_dirstory_observe"
            change = "cd a"
        states, details = self.run_script([spy, change, remove, observe, "b", "f"])
        self.assert_state(states[3], "a", ("",), details=details)
        self.assert_state(states[4], "", (), ("a",), details=details)
        self.assertEqual(states[3], states[5], details)

    def test_failed_commit_clears_navigation_suppression(self):
        if self.shell == "fish":
            spy = "function dirstory; if test \"$argv[2]\" = commit; return 1; end; command dirstory $argv; end"
            remove = "functions -e dirstory"
        else:
            spy = 'dirstory() { if [ "$2" = commit ]; then return 1; fi; command dirstory "$@"; }'
            remove = "unfunction dirstory" if self.shell == "zsh" else "unset -f dirstory"
        states, details = self.run_script(["cd a", "cd ../b", spy, "b", remove, "cd ../c", "b", "f"])
        self.assertNotEqual(states[3].status, 0, details)
        self.assertEqual(states[3].pwd, str(self.root / "a"), details)
        self.assertEqual(states[5].pwd, str(self.root / "c"), details)
        expected = "b" if self.shell == "zsh" else "a"
        self.assertEqual(states[6].pwd, str(self.root / expected), details)
        self.assertEqual(states[7].pwd, str(self.root / "c"), details)


class BashScenarios:
    """PROMPT_COMMAND composition and Bash-specific observation boundaries."""

    def test_prompt_command_registration_forms(self):
        setups = [
            "unset PROMPT_COMMAND",
            "PROMPT_COMMAND='printf \"__USER__\\n\"'",
            "PROMPT_COMMAND=('printf \"__FIRST__\\n\"' 'printf \"__SECOND__\\n\"')",
        ]
        for setup in setups:
            with self.subTest(setup=setup):
                console = Console(self, setup)
                output = console.step("declare -p PROMPT_COMMAND")
                self.assertEqual(output.count('"_dirstory_observe"'), 1, output)
                console.step(self.init())
                output = console.step("declare -p PROMPT_COMMAND")
                self.assertEqual(output.count('"_dirstory_observe"'), 1, output)
                output = console.step(":")
                if "__USER__" in setup:
                    self.assertIn("__USER__\n", output)
                if "__FIRST__" in setup:
                    self.assertLess(output.index("__FIRST__\n"), output.index("__SECOND__\n"))
                console.close()

    def test_prompt_command_unsupported_declarations(self):
        setups = [
            "readonly PROMPT_COMMAND=':'",
            "unset PROMPT_COMMAND; declare -A PROMPT_COMMAND=([original]=':')",
            "unset PROMPT_COMMAND; target=':'; declare -n PROMPT_COMMAND=target",
        ]
        for setup in setups:
            with self.subTest(setup=setup):
                console = Console(self, setup)
                output = console.step("declare -p PROMPT_COMMAND")
                self.assertNotIn("_dirstory_observe", output)
                console.step("cd a")
                output = console.step(self.snapshot())
                self.assert_state(self.parse_snapshot(output)[0], "a", ("",), details=output)
                console.close()

    def test_prompt_preserves_status_for_existing_user_command(self):
        console = Console(self, "PROMPT_COMMAND='printf \"__USER_STATUS__%s\\n\" \"$?\"'")
        output = console.step("false")
        self.assertIn("__USER_STATUS__1\n", output)

    def test_builtin_intermediate_changes_are_not_reconstructed(self):
        console = Console(self)
        console.step("builtin cd a; builtin cd ../b")
        output = console.step(self.snapshot())
        self.assert_state(self.parse_snapshot(output)[0], "b", ("",), details=output)


class NativeHookScenarios:
    """Directory callbacks must not confirm a redirected navigation target."""

    def test_navigation_redirect_does_not_commit(self):
        if self.shell == "fish":
            redirect = 'function redirect --on-variable PWD; if test "$PWD" = "' + str(self.root / "a") + '"; builtin cd "' + str(self.root / "c") + '"; end; end'
            remove = "functions -e redirect"
        else:
            redirect = 'chpwd() { if [[ "$PWD" = "' + str(self.root / "a") + '" ]]; then builtin cd "' + str(self.root / "c") + '"; fi; }'
            remove = "unfunction chpwd"
        states, details = self.run_script(["cd a", "cd ../b", redirect, "b", remove, "cd ../a", "b"])
        self.assertNotEqual(states[3].status, 0, details)
        self.assertEqual(states[3].pwd, str(self.root / "c"), details)
        # Fish's list calls repair the redirected position; Zsh has no fallback.
        expected = "c" if self.shell == "fish" else "b"
        self.assertEqual(states[-1].pwd, str(self.root / expected), details)
        if self.shell == "zsh":
            self.assertEqual(states[3].back, states[2].back, details)
            self.assertEqual(states[3].forward, states[2].forward, details)


class ZshScenarios(NativeHookScenarios):
    """Zsh deliberately respects explicit hook suppression."""

    def test_cd_q_is_not_tracked(self):
        states, details = self.run_script(["builtin cd -q a"])
        self.assert_state(states[0], "a", details=details)


class FishScenarios(NativeHookScenarios):
    """Fallback checks repair a missed variable event without inventing visits."""

    def test_missing_variable_event_is_repaired_before_navigation(self):
        states, details = self.run_script(["functions -e _dirstory_observe", "builtin cd a", "b", "f"])
        self.assert_state(states[1], "a", ("",), details=details)
        self.assert_state(states[2], "", (), ("a",), details=details)
        self.assertEqual(states[1], states[3], details)

    def test_missing_variable_event_is_repaired_at_prompt(self):
        console = Console(self)
        console.step("functions -e _dirstory_observe; builtin cd a")
        output = console.step("dirstory internal list back 99")
        self.assertIn(str(self.root) + "\n", output)

    def test_builtin_in_substitution_changes_the_owning_fish(self):
        states, details = self.run_script(["set ignored (builtin cd a; pwd)", "b", "f"])
        self.assert_state(states[0], "a", ("",), details=details)
        self.assert_state(states[1], "", (), ("a",), details=details)
        self.assertEqual(states[0], states[2], details)
