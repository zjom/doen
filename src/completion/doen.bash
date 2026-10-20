# bash completion for doen
_doen() {
    local cur prev cmd
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmd="${COMP_WORDS[1]}"
    local cmds="note todo ls today show grep tags edit title schedule due date priority tag done drop reopen archive unarchive rm restore trash init check completion help"
    if [[ $COMP_CWORD -eq 1 && $cur != -* ]]; then
        COMPREPLY=($(compgen -W "$cmds" -- "$cur"))
        return
    fi
    case "$prev" in
        -t|--tag|-x|--exclude-tag)
            COMPREPLY=($(compgen -W "$(doen __complete tags 2>/dev/null)" -- "$cur")); return ;;
        -T|--template)
            COMPREPLY=($(compgen -W "none $(doen __complete templates 2>/dev/null)" -- "$cur")); return ;;
        -w|--date|-S|--scheduled|-d|--due|--on|--closed|--created)
            COMPREPLY=($(compgen -W "$(doen __complete dates 2>/dev/null) W M Y LW LM LY" -- "$cur")); return ;;
        -p|--priority) COMPREPLY=($(compgen -W "1 2 3 none" -- "$cur")); return ;;
        -s|--status) COMPREPLY=($(compgen -W "open done drop closed any" -- "$cur")); return ;;
        --sort) COMPREPLY=($(compgen -W "due scheduled date created closed title status priority" -- "$cur")); return ;;
        --color) COMPREPLY=($(compgen -W "auto always never" -- "$cur")); return ;;
        --dir) COMPREPLY=($(compgen -d -- "$cur")); return ;;
    esac
    if [[ $cur == -* ]]; then
        COMPREPLY=($(compgen -W "--dir --color --yes --help -s -t -x -w -S -d -p -q -E -a -n -l -r --scheduled --created --closed --overdue --archived --trashed --sort --ids --porcelain --json -c -T -e --on --raw --path --empty --force --fix --no-header --no-footer" -- "$cur"))
        return
    fi
    case "$cmd" in
        tag) if [[ $COMP_CWORD -eq 2 ]]; then COMPREPLY=($(compgen -W "add rm" -- "$cur")); return; fi
             if [[ $COMP_CWORD -eq 3 ]]; then COMPREPLY=($(compgen -W "$(doen __complete tags 2>/dev/null)" -- "$cur")); return; fi ;;
        priority) if [[ $COMP_CWORD -eq 2 ]]; then COMPREPLY=($(compgen -W "1 2 3 none" -- "$cur")); return; fi ;;
        completion) COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur")); return ;;
        help) COMPREPLY=($(compgen -W "$cmds" -- "$cur")); return ;;
        schedule|due|date) if [[ $COMP_CWORD -eq 2 ]]; then COMPREPLY=($(compgen -W "$(doen __complete dates 2>/dev/null)" -- "$cur")); return; fi ;;
        note|n|todo|t|init|check|today|tags|grep) return ;;
        ls|"") COMPREPLY=($(compgen -W "note todo" -- "$cur")); return ;;
    esac
    COMPREPLY=($(compgen -W "$(doen __complete ids 2>/dev/null | cut -f1)" -- "$cur"))
}
complete -F _doen doen
