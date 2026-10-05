#!/usr/bin/env python3
"""Isolated shell integration tests; require an already built dirstory binary."""
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

BINARY = shutil.which(os.environ.get("DIRSTORY_BIN", "dirstory"))
if not BINARY:
    raise SystemExit("Build dirstory and set DIRSTORY_BIN or add its directory to PATH")
BINARY = str(Path(BINARY).resolve())

POSIX = r'''
eval "$(dirstory init SHELL)" || exit 1
check() { if ! "$@"; then echo "Failed: $*" >&2; exit 1; fi; }
check cd "$TEST_ROOT/a space"
check cd "$TEST_ROOT/b;literal"
check b
check test "$PWD" = "$TEST_ROOT/a space"
check f
check test "$PWD" = "$TEST_ROOT/b;literal"
check b 100
check test "$PWD" = "$TEST_ROOT"
check f 100
check test "$PWD" = "$TEST_ROOT/b;literal"
check b
if cd "$TEST_ROOT/missing"; then exit 1; fi
check test "$(f -l 1)" = "$TEST_ROOT/b;literal"
check cd "$TEST_ROOT/c$(literal)"
if f; then exit 1; fi
check test "$(b -l 1)" = "$TEST_ROOT/a space"
check cd "$PWD"
check test "$(b -l 1)" = "$TEST_ROOT/a space"
check b 0
check test "$PWD" = "$TEST_ROOT/c$(literal)"
check b
rmdir "$TEST_ROOT/c$(literal)"
if f; then exit 1; fi
check test "$(f -l 1)" = "$TEST_ROOT/c$(literal)"
check test "$PWD" = "$TEST_ROOT/a space"
check b --help
check f --help
'''.replace('$(literal)', r'\$(literal)')

FISH = r'''
dirstory init fish | source
function check
    $argv
    or begin
        echo "Failed: $argv" >&2
        exit 1
    end
end
check cd "$TEST_ROOT/a space"
check cd "$TEST_ROOT/b;literal"
check b
check test "$PWD" = "$TEST_ROOT/a space"
check f
check test "$PWD" = "$TEST_ROOT/b;literal"
check b 100
check test "$PWD" = "$TEST_ROOT"
check f 100
check test "$PWD" = "$TEST_ROOT/b;literal"
check b
if cd "$TEST_ROOT/missing"
    exit 1
end
check test (f -l 1) = "$TEST_ROOT/b;literal"
check cd "$TEST_ROOT/c\$(literal)"
if f
    exit 1
end
check test (b -l 1) = "$TEST_ROOT/a space"
check cd "$PWD"
check test (b -l 1) = "$TEST_ROOT/a space"
check b 0
check test "$PWD" = "$TEST_ROOT/c\$(literal)"
check b
rmdir "$TEST_ROOT/c\$(literal)"
if f
    exit 1
end
check test (f -l 1) = "$TEST_ROOT/c\$(literal)"
check test "$PWD" = "$TEST_ROOT/a space"
check b --help
check f --help
'''

for shell in ("sh", "dash", "bash", "zsh", "fish"):
    executable = shutil.which(shell)
    if not executable:
        if os.environ.get("DIRSTORY_REQUIRE_SHELLS") == "1":
            raise SystemExit(f"Required shell unavailable: {shell}")
        print(f"SKIP {shell}: executable unavailable", flush=True)
        continue
    with tempfile.TemporaryDirectory(prefix="dirstory-e2e-") as root:
        base = Path(root)
        for name in ("a space", "b;literal", "c$(literal)", "config", "runtime", "home"):
            (base / name).mkdir()
        (base / "config/dirstory.json").write_text('{"mode":"tmux","unique_top":true}')
        env = dict(os.environ, PATH=str(Path(BINARY).parent) + os.pathsep + os.environ.get("PATH", ""), TEST_ROOT=root, HOME=str(base / "home"),
                   XDG_CONFIG_HOME=str(base / "config"), XDG_RUNTIME_DIR=str(base / "runtime"), TMUX_PANE="%test")
        script = FISH if shell == "fish" else POSIX.replace("SHELL", "sh" if shell == "dash" else shell)
        subprocess.run([executable, "-c", script], cwd=root, env=env, check=True)
        # Verify CLI reset and obsolete stack command removal in the same session.
        subprocess.run([BINARY, "internal", "reset", root], env=env, check=True)
        result = subprocess.run([BINARY, "internal", "list", "back", "10"], env=env, capture_output=True, check=True)
        assert result.stdout == b""
        assert subprocess.run([BINARY, "stack", "--stack-type", "backward", "empty"], env=env, capture_output=True).returncode != 0
        data = (base / "runtime/dirstory/test/history.bin").read_bytes()
        assert struct.unpack_from("<Q", data, 24)[0] == 64
        print(f"PASS {shell}", flush=True)
