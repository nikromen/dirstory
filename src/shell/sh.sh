dirstory internal ensure "$PWD"
<COMMAND>() {
    local dirstory_old="$PWD" dirstory_status
    command cd "$@"
    dirstory_status=$?
    if [ "$dirstory_status" -eq 0 ]; then
        dirstory internal visit --from "$dirstory_old" --to "$PWD" || return 1
    fi
    return "$dirstory_status"
}
_dirstory_navigate() {
    local dirstory_direction="$1" dirstory_output dirstory_token dirstory_target dirstory_nl
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
    command cd "$dirstory_target" || return $?
    dirstory internal commit "$dirstory_token"
}
b() { _dirstory_navigate back "$@"; }
f() { _dirstory_navigate forward "$@"; }
