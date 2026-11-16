<ARGUMENTS>

# Keep all backend calls in this shell's history session.
export DIRSTORY_SHELL_PID=$$
dirstory internal ensure "$PWD"

# Record a visit only after the shell successfully changes directory.
<COMMAND>() {
    local dirstory_old="$PWD" dirstory_status

    command cd "$@"
    dirstory_status=$?

    if [ "$dirstory_status" -eq 0 ]; then
        dirstory internal visit --from "$dirstory_old" --to "$PWD" || return 1
    fi

    return "$dirstory_status"
}

# Navigate in either direction or list neighboring visits.
_dirstory_navigate() {
    local dirstory_direction="$1" dirstory_output dirstory_token
    local dirstory_target dirstory_nl dirstory_result _dirstory_action _dirstory_count
    local dirstory_name=b

    [ "$dirstory_direction" != forward ] || dirstory_name=f
    [ "$dirstory_direction" != pick ] || dirstory_name=dh
    shift

    # Parse user arguments before reading history.
    _dirstory_parse "$dirstory_name" "$@" || return $?
    [ "$_dirstory_action" != help ] || return 0

    if [ "$_dirstory_action" = reset ]; then
        dirstory internal reset "$PWD" || return 1
        return 0
    fi

    if [ "$_dirstory_action" = list ]; then
        dirstory internal list "$dirstory_direction" "$_dirstory_count" || return 1
        return 0
    fi

    # Select a target without changing the current history position.
    if [ "$_dirstory_action" = pick ]; then
        dirstory_output=$(dirstory internal pick) || return 1
        [ -n "$dirstory_output" ] || return 0
    else
        dirstory_output=$(dirstory internal select "$dirstory_direction" "$_dirstory_count") || return 1
    fi
    if [ -z "$dirstory_output" ]; then
        printf 'dirstory: no directory to go %s to\n' "$dirstory_direction" >&2
        return 1
    fi

    # The first line is a confirmation token; the remaining text is the path.
    dirstory_nl='
'
    dirstory_token=${dirstory_output%%"$dirstory_nl"*}
    dirstory_target=${dirstory_output#*"$dirstory_nl"}

    # Confirm the selected history position only after cd succeeds.
    command cd "$dirstory_target" || return 1
    if [ "$PWD" != "$dirstory_target" ]; then
        printf "%s\n" "dirstory: directory change did not reach the selected path" >&2
        return 1
    fi
    dirstory internal commit "$dirstory_token" || return 1
}

b() { _dirstory_navigate back "$@"; }
f() { _dirstory_navigate forward "$@"; }

dh() { _dirstory_navigate pick "$@"; }
