use crate::enums::Shell;

pub fn generate_template<'a>(shell: &'a Shell, command: &'a str) -> String {
    let sh = r#"
<COMMAND>() {
    args=""
    while true; do
        case "$1" in
            -*)
                args="$args $1"
                shift
                ;;
            "")
                dirstory stack --stack-type forward empty
                dirstory stack --stack-type backward push "$(pwd)"
                builtin cd $args
                break
                ;;
            *)
                args="$args $1"
                shift
                ;;
        esac
    done
}

b() {
    help_message="Usage: b [OPTIONS] [NUMBER]
Options:
    -h, --help          Display this help message
    -l, --list          Show last N directories you visited

Arguments:
    NUMBER              Visit the Nth directory in the stack [default: 1]"

    n=1
    case "$1" in
        -h|--help)
            echo "$help_message"
            return 0
            ;;
        -l|--list)
            dirstory stack --stack-type backward list "$2"
            return 0
            ;;
        [0-9]*)
            n="$1"
            ;;
        "")
            ;;
        *)
            echo "Invalid option: $1"
            echo "$help_message"
            return 1
            ;;
    esac

    cd_to=$(dirstory navigate back "$n")
    if [ -n "$cd_to" ]; then
        builtin cd "$cd_to"
    else
        echo "No directory to go back to" >&2
        return 1
    fi
}

f() {
    help_message="Usage: f [OPTIONS] [NUMBER]
Options:
    -h, --help          Display this help message
    -l, --list          Show last N directories you visited

Arguments:
    NUMBER              Visit the Nth directory in the stack [default: 1]"

    n=1
    case "$1" in
        -h|--help)
            echo "$help_message"
            return 0
            ;;
        -l|--list)
            dirstory stack --stack-type forward list "$2"
            return 0
            ;;
        [0-9]*)
            n="$1"
            ;;
        "")
            ;;
        *)
            echo "Invalid option: $1"
            echo "$help_message"
            return 1
            ;;
    esac

    cd_to=$(dirstory navigate forward "$n")
    if [ -n "$cd_to" ]; then
        builtin cd "$cd_to"
    else
        echo "No directory to go forward to" >&2
        return 1
    fi
}
"#;

    let bash_zsh: &str = r#"
<COMMAND>() {
    local args=()
    while true; do
        case $1 in
            -*)
                args+=("$1")
                shift
                ;;
            "")
                dirstory stack --stack-type forward empty
                dirstory stack --stack-type backward push "$(pwd)"
                builtin cd "${args[@]}"
                break
                ;;
            *)
                args+=("$1")
                shift
                ;;
        esac
    done
}

b() {
    local help_message="Usage: b [OPTIONS] [NUMBER]
Options:
    -h, --help          Display this help message
    -l, --list          Show last N directories you visited

Arguments:
    NUMBER              Visit the Nth directory in the stack [default: 1]"

    local n=1
    case $1 in
        -h|--help)
            echo "$help_message"
            return 0
            ;;
        -l|--list)
            dirstory stack --stack-type backward list $2
            return 0
            ;;
        [0-9]*)
            n=$1
            ;;
        "")
            ;;
        *)
            echo "Invalid option: $1"
            echo "$help_message"
            return 1
    esac

    local cd_to=$(dirstory navigate back $n)
    if [ -n "$cd_to" ]; then
        builtin cd "$cd_to"
    else
        echo "No directory to go back to" >&2
        return 1
    fi
}

f() {
    local help_message="Usage: f [OPTIONS] [NUMBER]
Options:
    -h, --help          Display this help message
    -l, --list          Show last N directories you visited

Arguments:
    NUMBER              Visit the Nth directory in the stack [default: 1]"

    local n=1
    case $1 in
        -h|--help)
            echo "$help_message"
            return 0
            ;;
        -l|--list)
            dirstory stack --stack-type forward list $2
            return 0
            ;;
        [0-9]*)
            n=$1
            ;;
        "")
            ;;
        *)
            echo "Invalid option: $1"
            echo "$help_message"
            return 1
    esac

    local cd_to=$(dirstory navigate forward $n)
    if [ -n "$cd_to" ]; then
        builtin cd "$cd_to"
    else
        echo "No directory to go forward to" >&2
        return 1
    fi
}
"#;

    let fish = r#"
function <COMMAND>
    set --local args
    while true
        switch $argv[1]
            case -*
                set args "$args $argv[1]"
                set argv $argv[2..-1]
            case ""
                dirstory stack --stack-type forward empty
                dirstory stack --stack-type backward push (pwd)
                builtin cd $args
                break
            case '*'
                set args "$args $argv[1]"
                set argv $argv[2..-1]
        end
    end
end

function b
    set --local help_message "Usage: b [OPTIONS] [NUMBER]
Options:
    -h, --help          Display this help message
    -l, --list          Show last N directories you visited

Arguments:
    NUMBER              Visit the Nth directory in the stack [default: 1]"

    set --local n 1
    switch $argv[1]
        case -h --help
            echo $help_message
            return 0
        case -l --list
            dirstory stack --stack-type backward list $argv[2]
            return 0
        case '[0-9]*'
            set --local n $argv[1]
        case ''
            # No arguments provided
        case '*'
            echo "Invalid option: $argv[1]"
            echo $help_message
            return 1
    end

    set --local cd_to (dirstory navigate back $n)
    if test -n "$cd_to"
        builtin cd $cd_to
    else
        echo "No directory to go back to" >&2
        return 1
    end
end

function f
    set --local help_message "Usage: f [OPTIONS] [NUMBER]
Options:
    -h, --help          Display this help message
    -l, --list          Show last N directories you visited

Arguments:
    NUMBER              Visit the Nth directory in the stack [default: 1]"

    set -l n 1
    switch $argv[1]
        case -h --help
            echo $help_message
            return 0
        case -l --list
            dirstory stack --stack-type forward list $argv[2]
            return 0
        case '[0-9]*'
            set n $argv[1]
        case ''
            # No arguments provided
        case '*'
            echo "Invalid option: $argv[1]"
            echo $help_message
            return 1
    end

    set -l cd_to (dirstory navigate forward $n)
    if test -n "$cd_to"
        builtin cd $cd_to
    else
        echo "No directory to go forward to" >&2
        return 1
    end
end
"#;

    let chosen_template = match shell {
        Shell::Sh => sh,
        Shell::Bash | Shell::Zsh => bash_zsh,
        Shell::Fish => fish,
    };

    chosen_template.replace("<COMMAND>", command)
}
