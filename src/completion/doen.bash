# bash completion for doen: the candidates come from doen itself.
_doen() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    COMPREPLY=($(doen __complete line -- "${COMP_WORDS[@]:1:COMP_CWORD-1}" "$cur" 2>/dev/null | cut -f1))
    # A directory goes on without a space.
    if [[ ${#COMPREPLY[@]} -eq 1 && ${COMPREPLY[0]} == */ ]]; then
        compopt -o nospace
    fi
}
complete -F _doen doen
