# doen

doen is a notebook cli designed to actually get shit doen.

doen is portable, flexible and minimal.

```sh
cargo install doen
```

```
doen — get shit doen

Usage: doen [command] [options] [args]

Create:
  note|n <title...>          create a note
  todo|t <title...>          create a todo

Read:
  [ls] [note|todo] [filters] list (default command)
  today                      overdue, due today, today's notes
  show|s <id>...             print items
  grep <pattern> [filters]   full-text search
  tags [-a]                  list tags with counts

Edit:
  edit|e <id>                open in the editor
  title <id> <title...>      change the title
  due <date|none> <id>...    set or clear the due date
  date <date> <id>...        set the date of the item
  tag add|rm <tags> <id>...  add or remove tags

Todo status:
  done|d [--on <date>] <id>...
  drop [--on <date>] <id>...
  reopen [--on <date>] <id>...

Lifecycle:
  archive <id>...            unarchive <id>...
  rm <id>...                 move to the trash
  restore <id>...            restore from the trash
  trash [--empty]            list or empty the trash

Meta:
  init                       create the store and the default config
  check [--fix]              validate the store
  completion bash|zsh|fish   print a completion script
  help [command]

Global options:
  --dir <path>  --color auto|always|never  -y, --yes  -h, --help  --version

Run 'doen help <command>' for the options of a command.
```

inspired by [nb](https://github.com/xwmx/nb)
