# Record only observed changes in the owning shell, outside history navigation.
_dirstory_sync() {
    _dirstory_parent || return 0

    # Skip unchanged directories, navigation, and a synchronization in progress.
    if [ "${_dirstory_navigating:-0}" = 1 ] ||
       [ "${_dirstory_syncing:-0}" = 1 ] ||
       [ "$_dirstory_last_pwd" = "$PWD" ]; then
        return 0
    fi

    # Keep the previous directory if recording fails, so a later event can retry.
    _dirstory_syncing=1
    local dirstory_result=0

    if dirstory internal visit --from "$_dirstory_last_pwd" --to "$PWD"; then
        _dirstory_last_pwd="$PWD"
    else
        dirstory_result=1
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

    _dirstory_parent || return 1

    # Recovery must bypass synchronization and validation of the old history.
    if [ "$_dirstory_action" = reset ]; then
        dirstory internal reset "$PWD" || return 1
        _dirstory_last_pwd="$PWD"
        return 0
    fi

    <SYNC>

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

    # Suppress visit recording while changing directory during navigation.
    _dirstory_navigating=1

    # Confirm the selected position only after cd reaches the target.
    if builtin cd "$dirstory_target"; then
        if [ "$PWD" != "$dirstory_target" ]; then
            printf '%s\n' 'dirstory: directory change did not reach the selected path' >&2
            dirstory_result=1
        elif dirstory internal commit "$dirstory_token"; then
            _dirstory_last_pwd="$PWD"
            dirstory_result=0
        else
            dirstory_result=1
        fi
    else
        dirstory_result=1
    fi

    _dirstory_navigating=0
    return "$dirstory_result"
}

b() { _dirstory_navigate back "$@"; }

f() { _dirstory_navigate forward "$@"; }

dh() { _dirstory_navigate pick "$@"; }
