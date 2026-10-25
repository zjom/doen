//! Completion of the command line: the candidates for the word before the cursor, from the
//! commands and options of doen, the IDs and tags of the store, dates and templates.

use std::collections::BTreeSet;

use clap::{Arg, ArgAction, CommandFactory};

use crate::app::App;
use crate::cli::args::Cli;
use crate::cli::resolve::{Resolved, resolve};
use crate::commands::completion::{DATES, templates};
use crate::store::Store;

use super::command::split;
use super::input::Input;

/// The commands that the ui does not take.
const UNAVAILABLE: &[&str] = &["interactive", "init", "tags", "completion"];
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

/// The candidates for the word before the cursor.
pub struct Menu {
    /// The byte index in the line where the word starts.
    pub start: usize,
    pub candidates: Vec<Candidate>,
    /// The candidate that is in the line, if one is.
    pub selected: Option<usize>,
}

impl Menu {
    /// The menu for the word before the cursor, or `None` if nothing completes it.
    pub fn new(input: &Input, store: &Store, app: &App) -> Option<Menu> {
        let before = &input.text()[..input.cursor()];
        let start = before
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        let word = &before[start..];
        if word.contains(['\'', '"', '\\']) {
            return None;
        }
        let words = split(&before[..start]).ok()?;
        let candidates = candidates(&words, word, store, app);
        (!candidates.is_empty()).then_some(Menu {
            start,
            candidates,
            selected: None,
        })
    }

    /// Puts the next candidate, or the previous one if `back`, in the line. An only candidate is
    /// accepted with a space after it, which ends the menu: the result is `None` then.
    pub fn cycle(mut self, input: &mut Input, back: bool) -> Option<Menu> {
        let n = self.candidates.len();
        if n == 1 {
            input.replace(self.start, &format!("{} ", self.candidates[0].value));
            return None;
        }
        let i = match (self.selected, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        self.selected = Some(i);
        input.replace(self.start, &self.candidates[i].value);
        Some(self)
    }
}

/// The candidates for `word`, after the complete `words`.
fn candidates(words: &[String], word: &str, store: &Store, app: &App) -> Vec<Candidate> {
    let mut cli = Cli::command();
    cli.build();
    let ls = cli.find_subcommand("ls").expect("ls is a command");
    // A filter line, as `closed 1w` or `tomorrow #work`, lists as `ls`.
    let (mut cmd, mut rest, mut filters) = match words.split_first() {
        None if word.starts_with('#') => return tags(word, store),
        None if word.starts_with('-') => return options(ls, word),
        None => return commands(&cli, word),
        Some((first, _)) if first.starts_with('-') => (ls, words, true),
        Some((first, rest)) => match resolve(first) {
            Resolved::Command(name) => (
                cli.find_subcommand(&name).expect("a resolved command"),
                rest,
                name == "ls",
            ),
            Resolved::Unknown => (ls, rest, true),
            Resolved::Ambiguous(_) => return vec![],
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
        return values(arg, word, store, app);
    }
    if word.starts_with('-') {
        return options(cmd, word);
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
    arg.map_or(vec![], |a| values(a, word, store, app))
}

fn matching(word: &str, all: impl IntoIterator<Item = Candidate>) -> Vec<Candidate> {
    all.into_iter()
        .filter(|c| c.value.starts_with(word))
        .collect()
}

fn help(arg: &Arg) -> String {
    arg.get_help().map_or(String::new(), ToString::to_string)
}

fn commands(cli: &clap::Command, word: &str) -> Vec<Candidate> {
    let mut all = subcommands(cli, word);
    all.retain(|c| !UNAVAILABLE.contains(&c.value.as_str()));
    all.extend(matching(word, [candidate("closed", "List closed todos")]));
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
    let mut short = w[1..].chars();
    let c = short.next().filter(|_| short.next().is_none())?;
    cmd.get_arguments().find(|a| a.get_short() == Some(c))
}

fn options(cmd: &clap::Command, word: &str) -> Vec<Candidate> {
    let all = cmd
        .get_arguments()
        .filter(|a| !a.is_positional() && !a.is_hide_set())
        .filter(|a| !HIDDEN_OPTIONS.contains(&a.get_id().as_str()))
        .filter_map(|a| Some(candidate(format!("--{}", a.get_long()?), help(a))));
    matching(word, all)
}

/// The values of an option or a positional, by its possible values or its value name.
fn values(arg: &Arg, word: &str, store: &Store, app: &App) -> Vec<Candidate> {
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
        "COMMAND" => commands(&Cli::command(), word),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn values(line: &str) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path(), &[], false).unwrap();
        let app = App {
            root: dir.path().into(),
            cfg: Default::default(),
            clock: crate::dates::Clock::at(
                chrono::Local::now().fixed_offset(),
                chrono::Weekday::Mon,
            ),
            yes: false,
            color: false,
        };
        Menu::new(&Input::new(line), &store, &app).map_or(vec![], |m| {
            m.candidates.into_iter().map(|c| c.value).collect()
        })
    }

    #[test]
    fn completing() {
        assert_eq!(values("to"), ["todo", "today"]);
        assert_eq!(values("cl"), ["closed"]);
        assert_eq!(values("u"), ["unarchive"]);
        assert_eq!(values("ls --so"), ["--sort"]);
        assert_eq!(values("ls --sort ti"), ["title"]);
        assert_eq!(values("tag "), ["add", "rm"]);
        assert_eq!(values("due tom"), ["tomorrow"]);
        assert_eq!(values("done --on y"), ["yesterday"]);
        assert_eq!(values("closed L"), ["LW", "LM", "LY"]);
        assert_eq!(values("ls t"), ["todo", "today", "tomorrow", "tue", "thu"]);
        assert_eq!(values("n -T no"), ["none"]);
        assert_eq!(values("priority "), ["1", "2", "3", "none"]);
        assert!(values("n buy mi").is_empty());
        assert!(values("n 'buy mi").is_empty());
    }
}
