# Tracking state survives repeated initialization.
dirstory internal ensure "$PWD"
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

# Repeated events and navigation must not create duplicate visits.
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

function _dirstory_prompt --on-event fish_prompt
    set -l previous_status $status
    _dirstory_sync; or true
    return $previous_status
end

<CUSTOM_COMMAND>
function _dirstory_navigate
    _dirstory_parent; or return 1
    _dirstory_sync; or return 1
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
    set -g _dirstory_navigating 1
    set -l result 0
    if builtin cd "$output[2]"
        if test "$PWD" != "$output[2]"
            printf '%s\n' 'dirstory: directory change did not reach the selected path' >&2
            set result 1
        else if dirstory internal commit "$output[1]"
            set -g _dirstory_last_pwd "$PWD"
        else
            set result $status
        end
    else
        set result $status
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
