# fish completion for doen
set -l cmds note todo ls today show grep tags edit title schedule due date priority tag done drop reopen archive unarchive rm restore trash init check completion help
complete -c doen -f
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a note -d 'create a note'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a todo -d 'create a todo'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a ls -d 'list items'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a today -d 'agenda'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a show -d 'print items'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a grep -d 'full-text search'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a tags -d 'list tags'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a edit -d 'open in the editor'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a title -d 'change the title'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a schedule -d 'set the scheduled date'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a due -d 'set the due date'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a date -d 'set the date'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a priority -d 'set the priority'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a tag -d 'add or remove tags'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a done -d 'mark done'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a drop -d 'mark not doing'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a reopen -d 'back to open'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a archive -d 'archive'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a unarchive -d 'unarchive'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a rm -d 'move to the trash'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a restore -d 'restore from the trash'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a trash -d 'list or empty the trash'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a init -d 'create the store'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a check -d 'validate the store'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a completion -d 'print a completion script'
complete -c doen -n "not __fish_seen_subcommand_from $cmds" -a help -d 'show help'

# IDs, with the titles as descriptions.
complete -c doen -n "__fish_seen_subcommand_from show edit title done drop reopen archive unarchive rm restore" -a "(doen __complete ids 2>/dev/null)"
complete -c doen -n "__fish_seen_subcommand_from tag; and __fish_seen_subcommand_from add rm" -a "(doen __complete ids 2>/dev/null)"
complete -c doen -n "__fish_seen_subcommand_from tag; and not __fish_seen_subcommand_from add rm" -a "add rm"
complete -c doen -n "__fish_seen_subcommand_from schedule due date" -a "(doen __complete dates 2>/dev/null) (doen __complete ids 2>/dev/null)"
complete -c doen -n "__fish_seen_subcommand_from priority" -a "1 2 3 none (doen __complete ids 2>/dev/null)"
complete -c doen -n "__fish_seen_subcommand_from completion" -a "bash zsh fish"
complete -c doen -n "__fish_seen_subcommand_from help" -a "$cmds"
complete -c doen -n "__fish_seen_subcommand_from ls trash" -a "note todo"

# Options.
complete -c doen -l dir -r -F -d 'store directory'
complete -c doen -l color -x -a 'auto always never' -d 'color mode'
complete -c doen -s y -l yes -d 'skip confirmation'
complete -c doen -s t -l tag -x -a "(doen __complete tags 2>/dev/null)" -d 'tags'
complete -c doen -s x -l exclude-tag -x -a "(doen __complete tags 2>/dev/null)" -d 'exclude tags'
complete -c doen -s T -l template -x -a "none (doen __complete templates 2>/dev/null)" -d 'template'
complete -c doen -s w -l date -x -a "(doen __complete dates 2>/dev/null) W M Y LW LM LY" -d 'date'
complete -c doen -s S -l scheduled -x -a "(doen __complete dates 2>/dev/null) W M Y LW LM LY" -d 'scheduled date'
complete -c doen -s d -l due -x -a "(doen __complete dates 2>/dev/null) W M Y LW LM LY" -d 'due date'
complete -c doen -l on -x -a "(doen __complete dates 2>/dev/null)" -d 'date of the status change'
complete -c doen -l closed -x -a "(doen __complete dates 2>/dev/null) W M Y LW LM LY" -d 'closed range'
complete -c doen -l created -x -a "(doen __complete dates 2>/dev/null) W M Y LW LM LY" -d 'created range'
complete -c doen -s p -l priority -x -a '1 2 3 none' -d 'priority (3 highest)'
complete -c doen -s s -l status -x -a 'open done drop closed any' -d 'status'
complete -c doen -l sort -x -a 'due scheduled date created closed title status priority' -d 'sort field'
complete -c doen -s c -l content -x -d 'body text'
complete -c doen -s q -l query -x -d 'text'
complete -c doen -s n -l limit -x -d 'limit'
complete -c doen -s e -l edit -d 'edit after creation'
complete -c doen -s E -l regex -d 'regular expression'
complete -c doen -s a -l all -d 'include archived'
complete -c doen -s r -l reverse -d 'reverse'
complete -c doen -s l -l long -d 'long output'
complete -c doen -l overdue -d 'overdue todos'
complete -c doen -l archived -d 'only archived'
complete -c doen -l trashed -d 'only trash'
complete -c doen -l ids -d 'print only IDs'
complete -c doen -l porcelain -d 'stable output'
complete -c doen -l json -d 'JSON output'
complete -c doen -l raw -d 'print files as stored'
complete -c doen -l path -d 'print paths'
complete -c doen -l empty -d 'empty the trash'
complete -c doen -l fix -d 'repair the store'
complete -c doen -n "__fish_seen_subcommand_from rm" -s f -l force -d 'delete permanently'
complete -c doen -n "__fish_seen_subcommand_from ls" -l no-header -d 'no banner'
complete -c doen -n "__fish_seen_subcommand_from ls" -l no-footer -d 'no cheatsheet'
