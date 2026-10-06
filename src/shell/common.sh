# Record only observed changes in the owning shell, outside history navigation.
_dirstory_sync() {
    _dirstory_parent || return 0
    if [ "${_dirstory_navigating:-0}" = 1 ] ||
       [ "${_dirstory_syncing:-0}" = 1 ] ||
       [ "$_dirstory_last_pwd" = "$PWD" ]; then
        return 0
    fi
    _dirstory_syncing=1
    local dirstory_result=0
    if dirstory internal visit --from "$_dirstory_last_pwd" --to "$PWD"; then
        _dirstory_last_pwd="$PWD"
    else
        dirstory_result=$?
    fi
    _dirstory_syncing=0
    return "$dirstory_result"
}

# Keep prompt/hook status independent of history writes.
_dirstory_observe() {
    local dirstory_previous_status=$?
    _dirstory_sync || :
    return "$dirstory_previous_status"
}

_dirstory_navigate() {
    _dirstory_parent || return 1
    <SYNC>
    local dirstory_direction="$1" dirstory_output dirstory_token
    local dirstory_target dirstory_nl dirstory_result
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
    _dirstory_navigating=1
    if builtin cd "$dirstory_target"; then
        if [ "$PWD" != "$dirstory_target" ]; then
            printf '%s\n' 'dirstory: directory change did not reach the selected path' >&2
            dirstory_result=1
        elif dirstory internal commit "$dirstory_token"; then
            _dirstory_last_pwd="$PWD"
            dirstory_result=0
        else
            dirstory_result=$?
        fi
    else
        dirstory_result=$?
    fi
    _dirstory_navigating=0
    return "$dirstory_result"
}

b() { _dirstory_navigate back "$@"; }

f() { _dirstory_navigate forward "$@"; }
