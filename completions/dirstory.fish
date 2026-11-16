# Only inspect the command line; never initialize history or call the backend.
function _dirstory_completion_state
    set -l words (commandline -opc)
    set -l name (string replace -r '^.*/' '' -- $words[1])
    if test (count $words) -eq 1
        switch $name
            case b f
                echo navigation
            case dh
                echo picker
            case '*'
                echo root
        end
        return
    end

    test "$name" = dirstory; or return
    test "$words[2]" = init; or return
    set -l shell_seen 0
    set -l command_seen 0
    set -l needs_command 0
    for word in $words[3..-1]
        if test $needs_command -eq 1
            set needs_command 0
            continue
        end

        switch $word
            case -c --command
                test $command_seen -eq 0; or return
                set command_seen 1
                set needs_command 1
            case '--command=*'
                test $command_seen -eq 0; or return
                set command_seen 1
            case sh bash zsh fish
                test $shell_seen -eq 0; or return
                set shell_seen 1
            case '*'
                return
        end
    end

    if test $needs_command -eq 1
        echo value
        return
    end

    echo init
    test $shell_seen -eq 1; or echo shell
    test $command_seen -eq 1; or echo command
end

complete -c dirstory -f
complete -c dirstory -n 'contains -- root (_dirstory_completion_state)' -a init -d 'Print shell integration'
complete -c dirstory -n 'contains -- root (_dirstory_completion_state)' -s V -l version -d 'Show version'
complete -c dirstory -n 'contains -- root (_dirstory_completion_state)' -s h -l help -d 'Show help'
complete -c dirstory -n 'contains -- init (_dirstory_completion_state)' -s h -l help -d 'Show help'
complete -c dirstory -n 'contains -- shell (_dirstory_completion_state)' -a 'sh bash zsh fish'
complete -c dirstory -n 'contains -- command (_dirstory_completion_state)' -s c -l command -r -a cd -d 'Directory-change command'
complete -c dirstory -n 'contains -- value (_dirstory_completion_state)' -a cd

for name in b f
    complete -c $name -f
    complete -c $name -n 'contains -- navigation (_dirstory_completion_state)' -s h -l help -d 'Show help'
    complete -c $name -n 'contains -- navigation (_dirstory_completion_state)' -s l -l list -d 'List neighboring visits'
    complete -c $name -n 'contains -- navigation (_dirstory_completion_state)' -s i -l interactive -d 'Open history picker'
end

complete -c dh -f
complete -c dh -n 'contains -- picker (_dirstory_completion_state)' -s h -l help -d 'Show help'
complete -c dh -n 'contains -- picker (_dirstory_completion_state)' -l reset -d 'Discard this session history'
