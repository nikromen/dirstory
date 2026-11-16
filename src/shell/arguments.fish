# Parse the public grammar before synchronization or backend access.
function _dirstory_parse
    set -l dirstory_name $argv[1]
    set -e argv[1]
    set -l maximum <MAX_COUNT>

    # Start with one navigation step, then interpret any supplied option.
    set -g _dirstory_action navigate
    set -g _dirstory_count 1

    if set -q argv[1]
        switch "$argv[1]"
            case -h --help
                if test (count $argv) -ne 1
                    printf 'dirstory: invalid arguments; use %s --help\n' $dirstory_name >&2
                    return 2
                end

                set -g _dirstory_action help
                printf '%s\n' "<HELP>"
                return 0
            case -l --list
                set -g _dirstory_action list
                set -g _dirstory_count 10
                set -e argv[1]
        end
    end

    # Accept at most one remaining argument as the count.
    if test (count $argv) -gt 1
        printf 'dirstory: invalid arguments; use %s --help\n' $dirstory_name >&2
        return 2
    end

    if set -q argv[1]
        set -g _dirstory_count "$argv[1]"
    end

    # Reject empty values and anything other than decimal digits.
    if not string match -qr '^[0-9]+$' -- "$_dirstory_count"
        printf 'dirstory: N must be a decimal integer between 0 and %s\n' $maximum >&2
        return 2
    end

    # Remove leading zeros without interpreting the count as shell arithmetic.
    set -g _dirstory_count (string replace -r '^0+' '' -- "$_dirstory_count")
    if test -z "$_dirstory_count"
        set -g _dirstory_count 0
    end

    # Compare lengths first; equally long values are compared digit by digit.
    set -l length (string length -- "$_dirstory_count")
    set -l max_length (string length -- $maximum)

    if test $length -gt $max_length
        printf 'dirstory: N must be a decimal integer between 0 and %s\n' $maximum >&2
        return 2
    end

    if test $length -eq $max_length
        set -l index 1
        while test $index -le $max_length
            set -l digit (string sub -s $index -l 1 -- "$_dirstory_count")
            set -l limit (string sub -s $index -l 1 -- $maximum)

            if test $digit -gt $limit
                printf 'dirstory: N must be a decimal integer between 0 and %s\n' $maximum >&2
                return 2
            end

            if test $digit -lt $limit
                break
            end

            set index (math $index + 1)
        end
    end
end
