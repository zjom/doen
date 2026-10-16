pub mod args;
pub mod resolve;

use std::ffi::OsString;

use clap::{CommandFactory, FromArgMatches};

use crate::commands::{self, App};
use crate::error::{Error, Result};
use args::*;
use resolve::Resolved;

pub const TOP_HELP: &str = "\
doen — a notebook for the terminal

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
";

/// Parses the arguments of one command with clap.
fn parse<T: CommandFactory + FromArgMatches>(name: &str, args: &[OsString]) -> Result<T> {
    let cmd = T::command()
        .no_binary_name(true)
        .name(format!("doen {name}"))
        .bin_name(format!("doen {name}"));
    let res = cmd
        .try_get_matches_from(args)
        .and_then(|m| T::from_arg_matches(&m));
    res.map_err(|e| {
        let _ = e.print();
        Error::Silent(if e.use_stderr() { 2 } else { 0 })
    })
}

pub fn command_help(name: &str) -> Option<clap::Command> {
    let cmd = match name {
        "note" | "todo" => CreateCmd::command(),
        "ls" => LsCmd::command(),
        "today" => TodayCmd::command(),
        "show" => ShowCmd::command(),
        "grep" => GrepCmd::command(),
        "tags" => TagsCmd::command(),
        "edit" => EditCmd::command(),
        "title" => TitleCmd::command(),
        "due" | "date" => DateCmd::command(),
        "tag" => TagCmd::command(),
        "done" | "drop" | "reopen" => StatusCmd::command(),
        "archive" | "unarchive" | "rm" | "restore" => IdsCmd::command(),
        "trash" => TrashCmd::command(),
        "init" => InitCmd::command(),
        "check" => CheckCmd::command(),
        "completion" => CompletionCmd::command(),
        "help" => HelpCmd::command(),
        _ => return None,
    };
    Some(
        cmd.name(format!("doen {name}"))
            .bin_name(format!("doen {name}")),
    )
}

fn takes_value(a: &str) -> bool {
    a == "--dir" || a == "--color"
}

pub fn run(argv: Vec<OsString>) -> Result<()> {
    // Global options before the command word.
    let mut pre: Vec<OsString> = vec![];
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].to_string_lossy().to_string();
        if takes_value(&a) {
            pre.extend(argv[i..(i + 2).min(argv.len())].iter().cloned());
            i += 2;
        } else if a.starts_with("--dir=") || a.starts_with("--color=") || a == "-y" || a == "--yes"
        {
            pre.push(argv[i].clone());
            i += 1;
        } else if a == "--version" || a == "-V" {
            println!("doen {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        } else if a == "-h" || a == "--help" {
            print!("{TOP_HELP}");
            return Ok(());
        } else {
            break;
        }
    }
    let rest = &argv[i.min(argv.len())..];
    let (name, tail): (&str, &[OsString]) = match rest.first().map(|a| a.to_string_lossy()) {
        None => ("ls", rest),
        Some(w) if w.starts_with('-') => ("ls", rest),
        Some(w) => match resolve::resolve(&w) {
            Resolved::Command(n) => (n, &rest[1..]),
            Resolved::Ambiguous(c) => {
                return Err(Error::Usage(format!(
                    "'{w}' is ambiguous: {}",
                    c.join(", ")
                )));
            }
            Resolved::Unknown => {
                let mut msg = format!("unknown command '{w}'");
                if commands::looks_like_id(&pre, &w) {
                    msg.push_str(&format!(". To show the item, run 'doen show {w}'"));
                }
                return Err(Error::Usage(msg));
            }
        },
    };
    let mut args = pre;
    args.extend(tail.iter().cloned());
    let a = &args;
    match name {
        "note" | "todo" => {
            let c: CreateCmd = parse(name, a)?;
            commands::create::run(&App::new(&c.global)?, name, c)
        }
        "ls" => {
            let c: LsCmd = parse(name, a)?;
            commands::list::ls(&App::new(&c.global)?, c)
        }
        "grep" => {
            let c: GrepCmd = parse(name, a)?;
            commands::list::grep(&App::new(&c.global)?, c)
        }
        "trash" => {
            let c: TrashCmd = parse(name, a)?;
            commands::list::trash(&App::new(&c.global)?, c)
        }
        "today" => {
            let c: TodayCmd = parse(name, a)?;
            commands::list::today(&App::new(&c.global)?, c)
        }
        "show" => {
            let c: ShowCmd = parse(name, a)?;
            commands::show::show(&App::new(&c.global)?, c)
        }
        "tags" => {
            let c: TagsCmd = parse(name, a)?;
            commands::show::tags(&App::new(&c.global)?, c)
        }
        "edit" => {
            let c: EditCmd = parse(name, a)?;
            commands::edit::run(&App::new(&c.global)?, c)
        }
        "title" => {
            let c: TitleCmd = parse(name, a)?;
            commands::meta::title(&App::new(&c.global)?, c)
        }
        "due" | "date" => {
            let c: DateCmd = parse(name, a)?;
            commands::meta::date(&App::new(&c.global)?, name, c)
        }
        "tag" => {
            let c: TagCmd = parse(name, a)?;
            let (add, t) = match c.action {
                TagAction::Add(t) => (true, t),
                TagAction::Rm(t) => (false, t),
            };
            commands::meta::tag(&App::new(&t.global)?, add, t)
        }
        "done" | "drop" | "reopen" => {
            let c: StatusCmd = parse(name, a)?;
            commands::status::run(&App::new(&c.global)?, name, c)
        }
        "archive" | "unarchive" | "rm" | "restore" => {
            let c: IdsCmd = parse(name, a)?;
            commands::lifecycle::run(&App::new(&c.global)?, name, c)
        }
        "init" => {
            let c: InitCmd = parse(name, a)?;
            commands::init::run(&c.global)
        }
        "check" => {
            let c: CheckCmd = parse(name, a)?;
            commands::check::run(&App::new(&c.global)?, c)
        }
        "completion" => {
            let c: CompletionCmd = parse(name, a)?;
            commands::completion::print(c.shell);
            Ok(())
        }
        "__complete" => {
            let c: CompleteCmd = parse(name, a)?;
            commands::completion::data(&App::new(&c.global)?, &c.what)
        }
        "help" => {
            let c: HelpCmd = parse(name, a)?;
            match c.command.as_deref() {
                None => {
                    print!("{TOP_HELP}");
                    Ok(())
                }
                Some(w) => match resolve::resolve(w) {
                    Resolved::Command(n) => {
                        let mut cmd = command_help(n).expect("every command has help");
                        cmd.print_help()?;
                        Ok(())
                    }
                    _ => Err(Error::Usage(format!("unknown command '{w}'"))),
                },
            }
        }
        _ => unreachable!("unhandled command {name}"),
    }
}
