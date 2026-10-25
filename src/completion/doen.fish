# fish completion for doen: the candidates come from doen itself.
complete -c doen -f -a "(doen __complete line -- (commandline -opc)[2..] (commandline -ct) 2>/dev/null)"
