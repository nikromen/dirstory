<ARGUMENTS>

# Use the shell's PID to identify its history session.
set -gx DIRSTORY_SHELL_PID $fish_pid
dirstory internal ensure "$PWD"

# Preserve the owning shell and last observed directory across reinitialization.
if not set -q _dirstory_owner
    set -g _dirstory_owner $fish_pid
end

if not set -q _dirstory_last_pwd
    set -g _dirstory_last_pwd "$PWD"
end

function _dirstory_parent
    # Unlike Bash/Zsh, builtins in Fish substitutions can change this shell's cwd.
    test "$fish_pid" = "$_dirstory_owner"
end

# Record observed directory changes without duplicating navigation or events.
function _dirstory_sync
    _dirstory_parent; or return 0

    if test "$_dirstory_last_pwd" = "$PWD"
        return 0
    end

    if set -q _dirstory_navigating; and test "$_dirstory_navigating" = 1
        return 0
    end

    if set -q _dirstory_syncing; and test "$_dirstory_syncing" = 1
        return 0
    end

    # Keep the previous directory if recording fails, so a later event can retry.
    set -g _dirstory_syncing 1
    set -l result 0

    if dirstory internal visit --from "$_dirstory_last_pwd" --to "$PWD"
        set -g _dirstory_last_pwd "$PWD"
    else
        set result $status
    end

    set -g _dirstory_syncing 0
    return $result
end

# Named handlers are replaced, not accumulated, by another initialization.
function _dirstory_observe --on-variable PWD
    set -l previous_status $status
    _dirstory_sync; or true
    return $previous_status
end

# Reconcile any missed directory change before displaying the prompt.
function _dirstory_prompt --on-event fish_prompt
    set -l previous_status $status
    _dirstory_sync; or true
    return $previous_status
end

<CUSTOM_COMMAND>

# Navigate in either direction or list neighboring visits.
function _dirstory_navigate
    set -l direction $argv[1]
    set -e argv[1]
    set -l name b

    if test "$direction" = forward
        set name f
    end

    if test "$direction" = pick
        set name dh
    end

    # Parse user arguments before reading history.
    _dirstory_parse $name $argv; or return $status
    if test "$_dirstory_action" = help
        return 0
    end

    _dirstory_parent; or return 1

    # Recovery must bypass synchronization and validation of the old history.
    if test "$_dirstory_action" = reset
        dirstory internal reset "$PWD"; or return 1
        set -g _dirstory_last_pwd "$PWD"
        return 0
    end

    _dirstory_sync; or return 1

    if test "$_dirstory_action" = list
        dirstory internal list $direction $_dirstory_count; or return 1
        return 0
    end

    # Select a target without changing the current history position.
    set -l output
    if test "$_dirstory_action" = pick
        set output (dirstory internal pick)
        set -l pick_status $status
        if test $pick_status -ne 0
            return 1
        end
        if test (count $output) -eq 0
            return 0
        end
    else
        set output (dirstory internal select $direction $_dirstory_count)
        if test $status -ne 0
            return 1
        end
    end

    # The first line is a confirmation token; the second is the target path.
    if test (count $output) -ne 2
        printf 'dirstory: no directory to go %s to\n' $direction >&2
        return 1
    end

    # Suppress visit recording while changing directory during navigation.
    set -g _dirstory_navigating 1
    set -l result 0

    # Confirm the selected position only after cd reaches the target.
    if builtin cd "$output[2]"
        if test "$PWD" != "$output[2]"
            printf '%s\n' 'dirstory: directory change did not reach the selected path' >&2
            set result 1
        else if dirstory internal commit "$output[1]"
            set -g _dirstory_last_pwd "$PWD"
        else
            set result 1
        end
    else
        set result 1
    end

    set -g _dirstory_navigating 0
    return $result
end

function b
    _dirstory_navigate back $argv
end

function f
    _dirstory_navigate forward $argv
end

function dh
    _dirstory_navigate pick $argv
end
