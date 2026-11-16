# Shared completion entry point; b/f/dh also autoload this file via symlinks.
_dirstory_complete() {
    local current=${COMP_WORDS[COMP_CWORD]} name=${COMP_WORDS[0]##*/}
    local index word shell_seen=0 command_seen=0 needs_command=0
    COMPREPLY=()

    case "$name" in
        b|f)
            [ "$COMP_CWORD" -eq 1 ] || return 0
            COMPREPLY=( $(compgen -W '-h --help -l --list -i --interactive' -- "$current") )
            return 0 ;;
        dh)
            [ "$COMP_CWORD" -eq 1 ] || return 0
            COMPREPLY=( $(compgen -W '-h --help --reset' -- "$current") )
            return 0 ;;
    esac

    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=( $(compgen -W 'init -h --help -V --version' -- "$current") )
        return 0
    fi
    [ "${COMP_WORDS[1]}" = init ] || return 0

    for ((index=2; index<COMP_CWORD; index++)); do
        word=${COMP_WORDS[index]}
        if [ "$needs_command" -eq 1 ]; then
            needs_command=0
            continue
        fi

        case "$word" in
            -c|--command)
                [ "$command_seen" -eq 0 ] || return 0
                command_seen=1
                needs_command=1 ;;
            --command=*)
                [ "$command_seen" -eq 0 ] || return 0
                command_seen=1 ;;
            sh|bash|zsh|fish)
                [ "$shell_seen" -eq 0 ] || return 0
                shell_seen=1 ;;
            *) return 0 ;;
        esac
    done

    if [ "$needs_command" -eq 1 ]; then
        COMPREPLY=( $(compgen -W cd -- "$current") )
        return 0
    fi

    local choices='-h --help'
    [ "$shell_seen" -eq 1 ] || choices+=' sh bash zsh fish'
    [ "$command_seen" -eq 1 ] || choices+=' -c --command'
    COMPREPLY=( $(compgen -W "$choices" -- "$current") )
}

complete -F _dirstory_complete dirstory b f dh
