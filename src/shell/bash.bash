<ARGUMENTS>

# Use the shell's PID to identify its history session.
export DIRSTORY_SHELL_PID=$BASHPID
dirstory internal ensure "$PWD"

# Preserve the owning shell and last observed directory across reinitialization.
_dirstory_owner=${_dirstory_owner:-$BASHPID}
_dirstory_last_pwd=${_dirstory_last_pwd:-$PWD}

# Track changes only in the owning shell, not in its subshells.
_dirstory_parent() {
    [[ "$BASHPID" = "$_dirstory_owner" ]]
}

<COMMON>

# Preserve the builtin's arguments and record a successful change immediately.
<COMMAND>() {
    local dirstory_result

    if builtin cd "$@"; then
        _dirstory_sync || return 1
        return 0
    else
        dirstory_result=$?
        return "$dirstory_result"
    fi
}

# Preserve existing commands as opaque array entries; never evaluate them here.
_dirstory_register_prompt() {
    [[ $- == *i* ]] || return 0
    local dirstory_declaration dirstory_attributes dirstory_item

    # Inspect the declaration before treating PROMPT_COMMAND as an array.
    if ! dirstory_declaration=$(builtin declare -p PROMPT_COMMAND 2>/dev/null); then
        dirstory_declaration=
    fi
    dirstory_attributes=${dirstory_declaration#declare -}
    dirstory_attributes=${dirstory_attributes%% *}

    case "$dirstory_attributes" in
        *r*|*A*|*n*|*i*)
            printf '%s\n' 'dirstory: unsupported PROMPT_COMMAND declaration; using the cd wrapper only' >&2
            return 0 ;;
    esac

    # Preserve a scalar command as the first array entry.
    case "$dirstory_attributes" in
        *a*) ;;
        *)
            if [[ -v PROMPT_COMMAND ]]; then
                PROMPT_COMMAND=("$PROMPT_COMMAND")
            else
                PROMPT_COMMAND=()
            fi ;;
    esac

    # Register the observer only once, including after reinitialization.
    for dirstory_item in "${PROMPT_COMMAND[@]}"; do
        [[ "$dirstory_item" != _dirstory_observe ]] || return 0
    done

    PROMPT_COMMAND+=(_dirstory_observe)
}

_dirstory_register_prompt
