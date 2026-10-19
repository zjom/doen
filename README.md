# doen

doen is a notebook cli designed to actually get shit doen.

doen is portable, flexible and minimal.

inspired by [nb](https://github.com/xwmx/nb)

## installation

```sh
cargo install doen

# or
nix run github:zjom/doen
```

### home-manager

```nix
{
  inputs.doen.url = "github:zjom/doen";

  # in your home-manager modules:
  imports = [ inputs.doen.homeManagerModules.default ];

  programs.doen = {
    enable = true;
    notesDir = "${config.home.homeDirectory}/notes";
    settings = {
      week_start = "sun";
      default_tags = [ "inbox" ];
      git_autocommit = true;
    };
    templates.meeting = ./templates/meeting.md;
  };
}
```

`settings` accepts every key of `config.yaml`. A key you do not set keeps doen's default.

## usage

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
  priority <p|none> <id>...  set or clear the priority (1-3, 3 highest)
  date <date> <id>...        set the date of the item
  tag add|rm <tags> <id>...  add or remove tags

Todo status:
  done|d [--on <date>] <id>...
  drop [--on <date>] <id>...
  reopen [--on <date>] <id>...

Lifecycle:
  archive <id>...            unarchive <id>...
  rm [-f] <id>...            move to the trash (-f: delete permanently)
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

## development

```sh
nix develop
```
