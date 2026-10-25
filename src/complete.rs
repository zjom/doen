//! Completion: the candidates for the word before the cursor, from the commands and options of
//! doen, the IDs and tags of the store, dates and templates. The command line of the ui and the
//! shell scripts complete with it.

use std::collections::BTreeSet;
use std::ffi::OsString;

use clap::{Arg, ArgAction, CommandFactory};

use crate::app::App;
use crate::cli::args::Cli;
use crate::cli::command_word;
use crate::cli::resolve::{Resolved, resolve};
use crate::commands::completion::{DATES, templates};
use crate::store::Store;

#[derive(Clone, Copy, PartialEq)]
pub enum Context {
    /// The command line of the ui: filter lines, `closed`, and only the commands of the ui.
    Ui,
    /// A shell: every command, with global options before the command word.
    Shell,
}

/// The commands that the ui does not take.
pub const UNAVAILABLE: &[&str] = &["interactive", "init", "tags", "completion", "__complete"];
/// The options that the ui does not take or that do nothing in it.
const HIDDEN_OPTIONS: &[&str] = &["dir", "help", "version"];
const RANGES: &str = "W M Y LW LM LY";

pub struct Candidate {
    pub value: String,
    pub help: String,
}

fn candidate(value: impl Into<String>, help: impl Into<String>) -> Candidate {
    Candidate {
        value: value.into(),
        help: help.into(),
    }
}

/// The candidates for `word`, after the complete `words`.
pub fn candidates(
    ctx: Context,
    words: &[String],
    word: &str,
    store: &Store,
    app: &App,
) -> Vec<Candidate> {
    let mut cli = Cli::command();
    cli.build();
    let mut words = words;
    if ctx == Context::Shell {
        let argv: Vec<OsString> = words.iter().map(OsString::from).collect();
        let i = command_word(&argv);
        // The value of a global option, as `--dir <word>`.
        if i == words.len()
            && let Some(last) = words.last().filter(|w| !w.contains('='))
            && let Some(arg) = option(&cli, last).filter(|a| a.get_action().takes_values())
        {
            return values(ctx, arg, word, store, app);
        }
        words = &words[i..];
    }
    let ui = ctx == Context::Ui;
    let ls = cli.find_subcommand("ls").expect("ls is a command");
    // In the ui, a filter line, as `closed 1w` or `tomorrow #work`, lists as `ls`.
    let (mut cmd, mut rest, mut filters) = match words.split_first() {
        None if ui && word.starts_with('#') => return tags(word, store),
        None if word.starts_with('-') => return options(ctx, ls, word),
        None => return commands(ctx, &cli, word),
        Some((first, _)) if first.starts_with('-') => (ls, words, ui),
        Some((first, rest)) => match resolve(first) {
            Resolved::Command(name) => (
                cli.find_subcommand(&name).expect("a resolved command"),
                rest,
                ui && name == "ls",
            ),
            Resolved::Unknown if ui => (ls, rest, true),
            Resolved::Unknown | Resolved::Ambiguous(_) => return vec![],
        },
    };
    // `tag add` and `tag rm`.
    while cmd.has_subcommands() {
        filters = false;
        match rest.split_first() {
            None => return subcommands(cmd, word),
            Some((w, r)) => match cmd.find_subcommand(w) {
                Some(sub) => (cmd, rest) = (sub, r),
                None => return vec![],
            },
        }
    }
    // The option that takes the word as its value, else the number of positionals before it.
    let mut positionals = 0;
    let mut value_of = None;
    for w in rest {
        if value_of.take().is_some() {
            continue;
        }
        if w.len() > 1 && w.starts_with('-') {
            value_of = option(cmd, w).filter(|a| a.get_action().takes_values());
        } else {
            positionals += 1;
        }
    }
    if let Some(arg) = value_of {
        return values(ctx, arg, word, store, app);
    }
    if word.starts_with('-') {
        return options(ctx, cmd, word);
    }
    if filters {
        if word.starts_with('#') {
            return tags(word, store);
        }
        let kinds = ["note", "todo"].map(|k| candidate(k, ""));
        return matching(word, kinds.into_iter().chain(dates(true)));
    }
    let args: Vec<&Arg> = cmd.get_positionals().collect();
    let arg = args.get(positionals).or_else(|| {
        args.last()
            .filter(|a| matches!(a.get_action(), ArgAction::Append))
    });
    arg.map_or(vec![], |a| values(ctx, a, word, store, app))
}

fn matching(word: &str, all: impl IntoIterator<Item = Candidate>) -> Vec<Candidate> {
    all.into_iter()
        .filter(|c| c.value.starts_with(word))
        .collect()
}

fn help(arg: &Arg) -> String {
    arg.get_help().map_or(String::new(), ToString::to_string)
}

fn commands(ctx: Context, cli: &clap::Command, word: &str) -> Vec<Candidate> {
    let mut all = subcommands(cli, word);
    if ctx == Context::Ui {
        all.retain(|c| !UNAVAILABLE.contains(&c.value.as_str()));
        all.extend(matching(word, [candidate("closed", "List closed todos")]));
    }
    all
}

fn subcommands(cmd: &clap::Command, word: &str) -> Vec<Candidate> {
    let all = cmd.get_subcommands().filter(|c| !c.is_hide_set()).map(|c| {
        let about = c.get_about().map_or(String::new(), ToString::to_string);
        candidate(c.get_name(), about)
    });
    matching(word, all)
}

/// The option of a word, as `--tag` or `-t`.
fn option<'a>(cmd: &'a clap::Command, w: &str) -> Option<&'a Arg> {
    if let Some(long) = w.strip_prefix("--") {
        return cmd.get_arguments().find(|a| a.get_long() == Some(long));
    }
    let mut short = w.get(1..)?.chars();
    let c = short.next().filter(|_| short.next().is_none())?;
    cmd.get_arguments().find(|a| a.get_short() == Some(c))
}

fn options(ctx: Context, cmd: &clap::Command, word: &str) -> Vec<Candidate> {
    let all = cmd
        .get_arguments()
        .filter(|a| !a.is_positional() && !a.is_hide_set())
        .filter(|a| ctx == Context::Shell || !HIDDEN_OPTIONS.contains(&a.get_id().as_str()))
        .filter_map(|a| Some(candidate(format!("--{}", a.get_long()?), help(a))));
    matching(word, all)
}

/// The values of an option or a positional, by its possible values or its value name.
fn values(ctx: Context, arg: &Arg, word: &str, store: &Store, app: &App) -> Vec<Candidate> {
    let possible = arg.get_possible_values();
    if !possible.is_empty() {
        let all = possible.iter().filter(|p| !p.is_hide_set()).map(|p| {
            let help = p.get_help().map_or(String::new(), ToString::to_string);
            candidate(p.get_name(), help)
        });
        return matching(word, all);
    }
    let name = arg
        .get_value_names()
        .and_then(|n| n.first())
        .map_or_else(|| arg.get_id().as_str().to_uppercase(), ToString::to_string);
    match name.as_str() {
        "ID" => ids(word, store),
        "TAGS" => tags(word, store),
        "DATE" => matching(word, dates(false)),
        "RANGE" => matching(word, dates(true)),
        "P" | "PRIORITY" => matching(word, ["1", "2", "3", "none"].map(|p| candidate(p, ""))),
        "NAME" => {
            let names = std::iter::once("none".to_string()).chain(templates(app));
            matching(word, names.map(|n| candidate(n, "")))
        }
        "COMMAND" => commands(ctx, &Cli::command(), word),
        "PATH" => directories(word),
        _ => vec![],
    }
}

fn dates(ranges: bool) -> impl Iterator<Item = Candidate> {
    let ranges = if ranges { RANGES } else { "" };
    DATES
        .split(' ')
        .chain(ranges.split_whitespace())
        .map(|d| candidate(d, ""))
}

/// The items whose ID starts with the word, then those whose title contains it.
fn ids(word: &str, store: &Store) -> Vec<Candidate> {
    let word = word.to_lowercase();
    let (mut by_id, by_title): (Vec<_>, Vec<_>) = store
        .items
        .iter()
        .filter(|i| i.id().starts_with(&word) || i.title().to_lowercase().contains(&word))
        .partition(|i| i.id().starts_with(&word));
    by_id.extend(by_title);
    by_id
        .into_iter()
        .map(|i| candidate(i.id(), i.title()))
        .collect()
}

/// The tags for the last of comma-separated tags, after a `#` in a filter.
fn tags(word: &str, store: &Store) -> Vec<Candidate> {
    let head = word.rfind([',', '#']).map_or("", |i| &word[..=i]);
    let given: Vec<&str> = head.split([',', '#']).collect();
    let all: BTreeSet<&str> = store
        .items
        .iter()
        .flat_map(|i| i.meta.tags.iter().map(String::as_str))
        .filter(|t| !given.contains(t))
        .collect();
    matching(
        word,
        all.into_iter().map(|t| candidate(format!("{head}{t}"), "")),
    )
}

/// The directories whose path starts with the word, with a `/` after them. Hidden directories
/// only for a word that starts their name with a dot.
fn directories(word: &str) -> Vec<Candidate> {
    let (dir, prefix) = word.rsplit_once('/').map_or(("", word), |(d, p)| (d, p));
    let dir = if word.contains('/') {
        format!("{dir}/")
    } else {
        String::new()
    };
    let read = if dir.is_empty() { "." } else { dir.as_str() };
    let mut names: Vec<String> = std::fs::read_dir(read)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(prefix) && (prefix.starts_with('.') || !n.starts_with('.')))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|n| candidate(format!("{dir}{n}/"), ""))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{Kind, Place};
    use crate::testing::{app, create};

    fn values(store: &Store, app: &App, line: &str) -> Vec<String> {
        let mut words: Vec<String> = line.split(' ').map(String::from).collect();
        let word = words.pop().unwrap();
        candidates(Context::Shell, &words, &word, store, app)
            .into_iter()
            .map(|c| c.value)
            .collect()
    }

    #[test]
    fn shell() {
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path());
        let open = create(&app, Kind::Todo, "open");
        let gone = create(&app, Kind::Todo, "gone");
        let mut tx = app.begin().unwrap();
        let n = crate::ids::resolve(&mut tx.store, &gone, false).unwrap();
        tx.store.items[n].trash(app.clock.today);
        tx.save(n, "rm").unwrap();
        tx.close(&app);
        let store = Store::open(dir.path(), &[Place::Root, Place::Archive], false).unwrap();
        let v = |line| values(&store, &app, line);
        assert_eq!(v("pri "), ["1", "2", "3", "none"]);
        assert_eq!(v("priority 2 "), [open]);
        assert_eq!(v("--dir x due tom"), ["tomorrow"]);
        assert_eq!(v("-y --color never due +2"), ["+2d"]);
        assert_eq!(v("tag "), ["add", "rm"]);
        assert_eq!(v("in"), ["interactive", "init"]);
        assert_eq!(v("--co"), ["--color"]);
        assert!(v("--").contains(&"--dir".to_string()));
        assert_eq!(v("--color "), ["auto", "always", "never"]);
        // Not filter lines, as in the ui.
        assert!(v("cl").is_empty());
        assert!(v("tomorrow ").is_empty());
        let store = Store::open(dir.path(), &Place::ALL, false).unwrap();
        assert_eq!(values(&store, &app, "restore go"), [gone]);
    }

    #[test]
    fn directories_of_a_word() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["abc", "abd", ".hidden"] {
            std::fs::create_dir(dir.path().join(d)).unwrap();
        }
        std::fs::write(dir.path().join("abe"), "").unwrap();
        let base = dir.path().display().to_string();
        let names = |w: &str| -> Vec<String> {
            directories(&format!("{base}/{w}"))
                .into_iter()
                .map(|c| c.value[base.len() + 1..].to_string())
                .collect()
        };
        assert_eq!(names("ab"), ["abc/", "abd/"]);
        assert_eq!(names(""), ["abc/", "abd/"]);
        assert_eq!(names(".h"), [".hidden/"]);
    }
}
