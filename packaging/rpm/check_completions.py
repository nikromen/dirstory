#!/usr/bin/env python3
"""Check installed completion grammar and first-use loading without a backend."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile


def run(shell, script, args, env):
    result = subprocess.run([shell, "-c", script, "--", *args], env=env,
                            text=True, capture_output=True, timeout=15, check=True)
    return {line.split("\t", 1)[0] for line in result.stdout.splitlines() if line}


def main():
    root = Path(sys.argv[1]).resolve()
    share = root / "usr/share"
    files = {
        "bash": share / "bash-completion/completions/dirstory",
        "zsh": share / "zsh/site-functions/_dirstory",
        "fish": share / "fish/vendor_completions.d/dirstory.fish",
    }
    cases = [
        (["dirstory", ""], {"init", "-h", "--help", "-V", "--version"}),
        (["dirstory", "init", ""], {"sh", "bash", "zsh", "fish", "-h", "--help", "-c", "--command"}),
        (["dirstory", "init", "bash", ""], {"-h", "--help", "-c", "--command"}),
        (["dirstory", "init", "--command", ""], {"cd"}),
        (["dirstory", "init", "--command", "cdir", "fish", ""], {"-h", "--help"}),
        (["dirstory", "init", "--help", ""], set()),
        (["dirstory", "internal", ""], set()),
        (["b", ""], {"-h", "--help", "-l", "--list", "-i", "--interactive"}),
        (["f", ""], {"-h", "--help", "-l", "--list", "-i", "--interactive"}),
        (["b", "--list", ""], set()),
        (["f", "2", ""], set()),
        (["b", "--interactive", ""], set()),
        (["dh", ""], {"-h", "--help", "--reset"}),
        (["dh", "--reset", ""], set()),
        (["dh", "--help", ""], set()),
    ]

    with tempfile.TemporaryDirectory(prefix="dirstory-completions-") as temporary:
        calls = Path(temporary) / "backend-called"
        env = dict(os.environ, HOME=temporary, XDG_CONFIG_HOME=temporary,
                   DIRSTORY_COMPLETION_CALLS=str(calls),
                   BASH_COMPLETION_USER_DIR=str(share / "bash-completion"))
        scripts = {
            "bash": '''dirstory() { echo called >> "$DIRSTORY_COMPLETION_CALLS"; return 99; }
source "$1"
shift
COMP_WORDS=("$@")
COMP_CWORD=$((${#COMP_WORDS[@]} - 1))
_dirstory_complete
printf '%s\\n' "${COMPREPLY[@]}"''',
            "zsh": '''dirstory() { echo called >> "$DIRSTORY_COMPLETION_CALLS"; return 99; }
compadd() { [[ $1 != -- ]] || shift; print -rl -- "$@"; }
completion=$1
shift
words=("$@")
CURRENT=${#words[@]}
source "$completion"''',
            "fish": '''function dirstory; echo called >> "$DIRSTORY_COMPLETION_CALLS"; return 99; end
source "$argv[1]"
complete -C "$argv[2]"''',
        }

        for shell, completion in files.items():
            for words, expected in cases:
                args = [str(completion), " ".join(words)] if shell == "fish" else [str(completion), *words]
                actual = run(shell, scripts[shell], args, env)
                # Fish offers switches only after a dash, unlike positional words.
                if shell == "fish":
                    positional = {word for word in expected if not word.startswith("-")}
                    assert actual == positional, (shell, words, actual, positional)
                    options = run(shell, scripts[shell], [str(completion), " ".join(words) + "-"], env)
                    expected = {word for word in expected if word.startswith("-")}
                    actual = options
                assert actual == expected, (shell, words, actual, expected)

        for name in ("b", "f", "dh"):
            loaded = run("bash", '''source /usr/share/bash-completion/bash_completion
_completion_loader "$1"
complete -p "$1"''', [name], env)
            assert any("_dirstory_complete" in line for line in loaded), (name, loaded)
            loaded = run("fish", '''function dirstory; echo called >> "$DIRSTORY_COMPLETION_CALLS"; return 99; end
set -p fish_complete_path "$argv[1]"
function $argv[2]; end
complete -C "$argv[2] -"''', [str(files["fish"].parent), name], env)
            expected = "--reset" if name == "dh" else "--interactive"
            assert expected in loaded, (name, loaded)

        registered = run("zsh", '''fpath=("$1" $fpath)
autoload -Uz compinit
compinit -D -i
for name in dirstory b f dh; do print -r -- "$name:${_comps[$name]}"; done''',
                         [str(files["zsh"].parent)], env)
        assert registered == {f"{name}:_dirstory" for name in ("dirstory", "b", "f", "dh")}, registered
        assert not calls.exists(), "Loading completions invoked the backend"

    print("Completion grammar and first-use loading passed for Bash, Zsh and Fish")


if __name__ == "__main__":
    main()
