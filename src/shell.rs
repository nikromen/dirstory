use crate::enums::Shell;

pub fn generate_template(shell: &Shell, command: &str) -> String {
    let posix = r#"
dirstory internal ensure "$PWD"
<COMMAND>() {
    <LOCAL> dirstory_old="$PWD" dirstory_status
    <CD> "$@"
    dirstory_status=$?
    if [ "$dirstory_status" -eq 0 ]; then
        dirstory internal visit --from "$dirstory_old" --to "$PWD" || return 1
    fi
    return "$dirstory_status"
}
_dirstory_navigate() {
    <LOCAL> dirstory_direction="$1" dirstory_output dirstory_token dirstory_target dirstory_nl
    shift
    case "${1-}" in
        -h|--help)
            echo "Usage: b/f [OPTIONS] [NUMBER]
Options:
    -h, --help          Display this help message
    -l, --list          Show last N directories in this direction
Arguments:
    NUMBER              Number of steps [default: 1]"
            return 0 ;;
        -l|--list) dirstory internal list "$dirstory_direction" "${2:-10}"; return $? ;;
    esac
    dirstory_output=$(dirstory internal select "$dirstory_direction" "${1:-1}") || return 1
    if [ -z "$dirstory_output" ]; then
        echo "No directory to go $dirstory_direction to" >&2
        return 1
    fi
    dirstory_nl='
'
    dirstory_token=${dirstory_output%%"$dirstory_nl"*}
    dirstory_target=${dirstory_output#*"$dirstory_nl"}
    <CD> "$dirstory_target" || return $?
    dirstory internal commit "$dirstory_token"
}
b() { _dirstory_navigate back "$@"; }
f() { _dirstory_navigate forward "$@"; }
"#;
    let fish = r#"
dirstory internal ensure "$PWD"
function <COMMAND>
    set -l dirstory_old "$PWD"
    builtin cd $argv
    set -l dirstory_status $status
    if test $dirstory_status -eq 0
        dirstory internal visit --from "$dirstory_old" --to "$PWD"; or return 1
    end
    return $dirstory_status
end
function _dirstory_navigate
    set -l direction $argv[1]
    set -e argv[1]
    switch "$argv[1]"
        case -h --help
            printf '%s\n' 'Usage: b/f [-h|--help] [-l|--list N] [NUMBER]'
            return 0
        case -l --list
            set -l n 10
            if set -q argv[2]
                set n $argv[2]
            end
            dirstory internal list $direction $n
            return $status
    end
    set -l n 1
    if set -q argv[1]
        set n $argv[1]
    end
    set -l output (dirstory internal select $direction $n)
    if test $status -ne 0
        return 1
    end
    if test (count $output) -ne 2
        printf 'No directory to go %s to\n' $direction >&2
        return 1
    end
    builtin cd "$output[2]"; or return $status
    dirstory internal commit "$output[1]"
end
function b
    _dirstory_navigate back $argv
end
function f
    _dirstory_navigate forward $argv
end
"#;
    let template = match shell {
        Shell::Fish => fish.to_string(),
        Shell::Sh => posix
            .replace("<LOCAL>", "local")
            .replace("<CD>", "command cd"),
        Shell::Bash | Shell::Zsh => posix
            .replace("<LOCAL>", "local")
            .replace("<CD>", "builtin cd"),
    };
    template.replace("<COMMAND>", command)
}
