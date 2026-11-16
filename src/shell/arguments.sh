# Parse the public grammar before synchronization or backend access.
_dirstory_parse() {
    local dirstory_name="$1" dirstory_max=<MAX_COUNT>
    local dirstory_left dirstory_right dirstory_digit dirstory_limit

    # The first argument names the calling function; the rest come from the user.
    shift
    _dirstory_action=navigate
    _dirstory_count=1

    if [ "$dirstory_name" = dh ]; then
        case "$#:${1-}" in
            0:) _dirstory_action=pick; return 0 ;;
            1:-h|1:--help)
                _dirstory_action=help
                printf '%s\n' 'Usage: dh [-h|--help]' 'Open interactive directory history.'
                return 0 ;;
            *) printf '%s\n' 'dirstory: invalid arguments; use dh --help' >&2; return 2 ;;
        esac
    fi

    case "${1-}" in
        -i|--interactive)
            if [ "$#" -ne 1 ]; then
                printf 'dirstory: invalid arguments; use %s --help\n' "$dirstory_name" >&2
                return 2
            fi
            _dirstory_action=pick
            return 0 ;;
    esac

    # Recognize help and list options before validating the optional count.
    case "${1-}" in
        -h|--help)
            if [ "$#" -ne 1 ]; then
                printf 'dirstory: invalid arguments; use %s --help\n' "$dirstory_name" >&2
                return 2
            fi

            _dirstory_action=help
            printf '%s\n' "<HELP>"
            return 0 ;;
        -l|--list)
            _dirstory_action=list
            _dirstory_count=10
            shift

            if [ "$#" -gt 1 ]; then
                printf 'dirstory: invalid arguments; use %s --help\n' "$dirstory_name" >&2
                return 2
            fi ;;
        *)
            if [ "$#" -gt 1 ]; then
                printf 'dirstory: invalid arguments; use %s --help\n' "$dirstory_name" >&2
                return 2
            fi ;;
    esac

    if [ "$#" -eq 1 ]; then
        _dirstory_count=$1
    fi

    # Reject empty values and anything other than decimal digits.
    case "$_dirstory_count" in
        ''|*[!0-9]*)
            printf 'dirstory: N must be a decimal integer between 0 and %s\n' "$dirstory_max" >&2
            return 2 ;;
    esac

    # Remove leading zeros without interpreting the number as shell arithmetic.
    _dirstory_count=${_dirstory_count#"${_dirstory_count%%[!0]*}"}
    _dirstory_count=${_dirstory_count:-0}

    # Compare lengths first; equally long values are compared digit by digit.
    if [ "${#_dirstory_count}" -gt "${#dirstory_max}" ]; then
        printf 'dirstory: N must be a decimal integer between 0 and %s\n' "$dirstory_max" >&2
        return 2
    fi

    if [ "${#_dirstory_count}" -eq "${#dirstory_max}" ]; then
        dirstory_left=$_dirstory_count
        dirstory_right=$dirstory_max

        while [ -n "$dirstory_left" ]; do
            dirstory_digit=${dirstory_left%"${dirstory_left#?}"}
            dirstory_limit=${dirstory_right%"${dirstory_right#?}"}

            if [ "$dirstory_digit" -gt "$dirstory_limit" ]; then
                printf 'dirstory: N must be a decimal integer between 0 and %s\n' "$dirstory_max" >&2
                return 2
            fi

            [ "$dirstory_digit" -ge "$dirstory_limit" ] || break
            dirstory_left=${dirstory_left#?}
            dirstory_right=${dirstory_right#?}
        done
    fi
}
