//! The command line of the interactive mode: `:` takes a doen command, as on the command line.
//! The list commands replace the item list, `note` and `todo` create an item and the other
//! commands run doen itself. A command without IDs acts on the selected item.

use std::ffi::OsString;
use std::process::Stdio;

use clap::Parser;
use clap::error::ErrorKind;

use crate::app::App;
use crate::cli::args::{Cli, Command, CreateCmd, OutputArgs};
use crate::cli::normalize;
use crate::cli::resolve::{Resolved, resolve};
use crate::commands::{create, list};
use crate::dates::{Clock, parse_range};
use crate::error::{Result, general, usage};
use crate::filter::{Query, SortField, select};
use crate::ids;
use crate::item::{Item, Kind};
use crate::output::limit;

use super::{ConfirmAction, Focus, Popup, Ui, View};

/// The item list of a list command.
pub struct Listing {
    /// The command line, for the title of the list.
    pub line: String,
    kind: ListKind,
    sort: Option<SortField>,
    reverse: bool,
    limit: Option<usize>,
}

enum ListKind {
    /// `ls`, `grep` and `trash`.
    Query(Query),
    /// `today`.
    Agenda(Query),
    /// `show`: full IDs.
    Ids(Vec<String>),
}

impl Listing {
    fn new(line: &str, kind: ListKind, output: &OutputArgs) -> Listing {
        Listing {
            line: line.to_string(),
            kind,
            sort: output.sort,
            reverse: output.reverse,
            limit: output.limit,
        }
    }

    /// The groups of the list, sorted, as the command prints them.
    pub fn groups<'a>(
        &self,
        items: Vec<&'a Item>,
        app: &App,
    ) -> Vec<(&'static str, Vec<&'a Item>)> {
        let sort = self.sort.or(app.cfg.default_sort);
        let clock = &app.clock;
        let items = items.into_iter();
        let mut groups = match &self.kind {
            ListKind::Query(q) => {
                let (todos, notes) =
                    select(items.filter(|i| q.accepts(i, clock)), sort, self.reverse);
                vec![("TODOS", todos), ("NOTES", notes)]
            }
            ListKind::Agenda(q) => list::agenda(
                items.filter(|i| q.accepts(i, clock)),
                clock.today,
                sort,
                self.reverse,
            ),
            ListKind::Ids(ids) => {
                let shown = items.filter(|i| ids.iter().any(|id| id == i.id()));
                let (todos, notes) = select(shown, sort, self.reverse);
                vec![("TODOS", todos), ("NOTES", notes)]
            }
        };
        limit(&mut groups, self.limit);
        groups
    }
}

/// Splits a command line into words as a shell does, with quotes and backslashes.
fn split(line: &str) -> std::result::Result<Vec<String>, &'static str> {
    let mut words = vec![];
    let mut word: Option<String> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => words.extend(word.take()),
            '\'' => {
                let w = word.get_or_insert_default();
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => w.push(c),
                        None => return Err("unclosed quote"),
                    }
                }
            }
            '"' => {
                let w = word.get_or_insert_default();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => w.push(c),
                            Some(c) => w.extend(['\\', c]),
                            None => return Err("unclosed quote"),
                        },
                        Some(c) => w.push(c),
                        None => return Err("unclosed quote"),
                    }
                }
            }
            '\\' => word.get_or_insert_default().extend(chars.next()),
            c => word.get_or_insert_default().push(c),
        }
    }
    words.extend(word);
    Ok(words)
}

/// The `ls` options of a filter word: a date or range (`--date`) or `#tags` (`-t`).
fn filter_word(w: &str, clock: &Clock) -> Option<[String; 2]> {
    if let Some(tags) = w.strip_prefix('#') {
        return (!tags.is_empty()).then(|| ["-t".into(), tags.into()]);
    }
    (w != "none" && parse_range(w, clock).is_ok()).then(|| ["--date".into(), w.into()])
}

fn is_kind(w: &str) -> bool {
    matches!(w, "n" | "t" | "note" | "todo")
}

/// Rewrites a filter line as an `ls` command: a bare date or range filters by date, `#tags` by
/// tags, and a last `n` or `t` lists that kind. Options, from the first word that starts with `-`,
/// are kept as they are. Other lines are returned unchanged.
fn expand_filters(words: Vec<String>, clock: &Clock) -> Vec<String> {
    let n = words
        .iter()
        .position(|w| w.starts_with('-'))
        .unwrap_or(words.len());
    let (lead, options) = words.split_at(n);
    let Some(first) = lead.first() else {
        return words;
    };
    let resolved = resolve(first);
    let (kind, filters) = if resolved == Resolved::Command("ls".into()) {
        // `ls` takes the kind itself: only the filter words change.
        let mut argv = vec![first.clone()];
        for w in &lead[1..] {
            match filter_word(w, clock) {
                Some(f) => argv.extend(f),
                None => argv.push(w.clone()),
            }
        }
        argv.extend_from_slice(options);
        return argv;
    } else if lead.len() == 1 && is_kind(first) {
        (Some(first), &lead[..0])
    } else if resolved == Resolved::Unknown && filter_word(first, clock).is_some() {
        match lead.split_last() {
            Some((last, rest)) if is_kind(last) => (Some(last), rest),
            _ => (None, lead),
        }
    } else {
        return words;
    };
    let mut argv = vec!["ls".to_string()];
    argv.extend(kind.cloned());
    for w in filters {
        match filter_word(w, clock) {
            Some(f) => argv.extend(f),
            None => return words,
        }
    }
    argv.extend_from_slice(options);
    argv
}

/// The first line of a clap error, without the `error: ` prefix.
fn clap_message(e: &clap::Error) -> String {
    let text = e.render().to_string();
    let line = text.lines().next().unwrap_or_default();
    line.strip_prefix("error: ").unwrap_or(line).to_string()
}

/// The lines of a doen output, without the `doen: ` prefix.
fn output_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|l| l.strip_prefix("doen: ").unwrap_or(l).trim_end().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn summarize(lines: &[String]) -> String {
    match lines {
        [] => String::new(),
        [l] => l.clone(),
        [l, rest @ ..] => format!("{l} (+{} more)", rest.len()),
    }
}

impl Ui {
    /// Runs a command line. Returns the ID of an item to open in the editor.
    pub(super) fn command(&mut self, line: &str) -> Option<String> {
        match self.try_command(line) {
            Ok(edit) => edit,
            Err(e) => {
                self.error(e);
                None
            }
        }
    }

    fn try_command(&mut self, line: &str) -> Result<Option<String>> {
        let words = expand_filters(split(line).map_err(usage)?, &self.app.clock);
        if words.is_empty() {
            return Ok(None);
        }
        let (cli, argv) =
            self.parse(normalize(words.into_iter().map(OsString::from).collect())?)?;
        let g = &cli.global;
        if g.dir.is_some() {
            return Err(usage("--dir is not available in the ui"));
        }
        // The globals for doen itself: the store of the ui, no colors and no prompts, since
        // the ui asks first.
        let mut full: Vec<OsString> = vec!["--dir".into(), self.app.root.clone().into()];
        if g.color.is_none() {
            full.extend(["--color".into(), "never".into()]);
        }
        if !g.yes {
            full.push("-y".into());
        }
        full.extend(argv);
        let yes = g.yes;
        let clock = &self.app.clock;
        match cli.command {
            Command::Ls(c) => {
                let q = list::query(&c.filter, c.kind, None, clock)?;
                self.show_listing(Listing::new(line, ListKind::Query(q), &c.output));
            }
            Command::Grep(c) => {
                let q = list::grep_query(&c, clock)?;
                self.show_listing(Listing::new(line, ListKind::Query(q), &c.output));
            }
            Command::Trash(c) if c.empty && !yes => self.confirm(
                "Empty the trash",
                "Delete every item in the trash permanently?".into(),
                ConfirmAction::Command(full),
            ),
            Command::Trash(c) if !c.empty => {
                let q = list::trash_query(&c, clock)?;
                self.show_listing(Listing::new(line, ListKind::Query(q), &c.output));
            }
            Command::Today(c) => {
                let q = list::today_query(&c)?;
                self.show_listing(Listing::new(line, ListKind::Agenda(q), &c.output));
            }
            Command::Show(c) => {
                let mut shown = vec![];
                for id in &c.id {
                    let n = ids::resolve(&mut self.store, id, true)?;
                    shown.push(self.store.items[n].id().to_string());
                }
                let keep = shown.first().cloned();
                self.show_listing(Listing::new(
                    line,
                    ListKind::Ids(shown),
                    &OutputArgs::default(),
                ));
                self.rebuild(keep);
            }
            Command::Tags(_) => self.focus = Focus::Tags,
            Command::Help(_) => self.popup = Some(Popup::Help { scroll: 0 }),
            Command::Edit(c) => return Ok(Some(c.id)),
            Command::Note(c) => return self.create_cmd(Kind::Note, c),
            Command::Todo(c) => return self.create_cmd(Kind::Todo, c),
            Command::Rm(c) if !yes => {
                let mut titles = vec![];
                for id in &c.id {
                    let n = ids::resolve(&mut self.store, id, true)?;
                    titles.push(format!("'{}'", self.store.items[n].title()));
                }
                let titles = titles.join(", ");
                let (title, question) = if c.force {
                    ("Delete", format!("Delete {titles} permanently?"))
                } else {
                    ("Trash", format!("Move {titles} to the trash?"))
                };
                self.confirm(title, question, ConfirmAction::Command(full));
            }
            Command::Ui | Command::Init | Command::Completion(_) | Command::Complete(_) => {
                return Err(usage("the command is not available in the ui"));
            }
            _ => self.run_doen(full),
        }
        Ok(None)
    }

    /// Creates an item, with the selected tag. Returns the ID if it is to be edited.
    fn create_cmd(&mut self, kind: Kind, mut c: CreateCmd) -> Result<Option<String>> {
        if c.title.join("").trim().is_empty() {
            return Err(usage("the title cannot be empty"));
        }
        if c.content.as_deref() == Some("-") {
            return Err(usage("-c - reads stdin, which is not available in the ui"));
        }
        let edit = c.edit;
        c.tag.extend(self.tag().map(String::from));
        let r = create::create(&self.app, kind, c);
        let id = r.as_ref().ok().cloned();
        self.after(
            r.map(|id| format!("created  {id}")),
            id.as_deref().unwrap_or(""),
        );
        Ok(id.filter(|_| edit))
    }

    /// Parses the normalized words. If IDs are missing, the selected item is the ID.
    fn parse(&self, mut argv: Vec<OsString>) -> Result<(Cli, Vec<OsString>)> {
        let parse = |a: &[OsString]| {
            Cli::try_parse_from(std::iter::once(OsString::from("doen")).chain(a.iter().cloned()))
        };
        let e = match parse(&argv) {
            Ok(cli) => return Ok((cli, argv)),
            Err(e) => e,
        };
        if e.kind() == ErrorKind::MissingRequiredArgument
            && let Some(id) = self.selected_id()
        {
            argv.push(id.into());
            if let Ok(cli) = parse(&argv) {
                return Ok((cli, argv));
            }
        }
        Err(usage(match e.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                "run 'doen help <command>' in a shell for the options".to_string()
            }
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                "the command is incomplete: run 'doen help <command>' in a shell".to_string()
            }
            _ => clap_message(&e),
        }))
    }

    fn show_listing(&mut self, listing: Listing) {
        self.listing = Some(listing);
        self.tag_state.select(Some(0));
        self.focus = Focus::Items;
        self.cursor = 0;
        self.rebuild(None);
    }

    /// Selects the view `v` instead of a listing.
    pub(super) fn clear_listing(&mut self, v: Option<View>) {
        if let Some(v) = v {
            let n = super::VIEWS.iter().position(|w| *w == v);
            self.views.select(n);
        }
        self.listing = None;
        self.cursor = 0;
        self.rebuild(None);
    }

    /// Runs doen with the arguments and reports its output.
    pub(super) fn run_doen(&mut self, argv: Vec<OsString>) {
        let keep = self.selected_id().unwrap_or_default();
        let r = std::env::current_exe()
            .and_then(|exe| {
                std::process::Command::new(exe)
                    .args(argv)
                    .stdin(Stdio::null())
                    .output()
            })
            .map_err(|e| general(format!("cannot run doen: {e}")))
            .and_then(|out| {
                let (stdout, stderr) = (output_lines(&out.stdout), output_lines(&out.stderr));
                if !out.status.success() {
                    return Err(general(
                        stderr
                            .last()
                            .cloned()
                            .unwrap_or_else(|| format!("doen failed ({})", out.status)),
                    ));
                }
                Ok(summarize(if stdout.is_empty() { &stderr } else { &stdout }))
            });
        self.after(r, &keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitting() {
        let s = |l| split(l).unwrap();
        assert_eq!(s("n -t work  call bob"), ["n", "-t", "work", "call", "bob"]);
        assert_eq!(
            s(r#"n 'it''s' "a \"b\" \c" x\ y"#),
            ["n", "its", r#"a "b" \c"#, "x y"]
        );
        assert_eq!(s("n ''"), ["n", ""]);
        assert!(s("  ").is_empty());
        assert!(split("n 'open").is_err());
    }

    #[test]
    fn filters() {
        let clock = Clock::at(
            chrono::DateTime::parse_from_rfc3339("2026-10-16T12:00:00+11:00").unwrap(),
            chrono::Weekday::Mon,
        );
        let x = |l| expand_filters(split(l).unwrap(), &clock).join(" ");
        assert_eq!(x("26-10-22"), "ls --date 26-10-22");
        assert_eq!(x("yesterday"), "ls --date yesterday");
        assert_eq!(x("#life,work"), "ls -t life,work");
        assert_eq!(x("#life #work"), "ls -t life -t work");
        assert_eq!(x("n"), "ls n");
        assert_eq!(x("t -a"), "ls t -a");
        assert_eq!(x("tomorrow #life n"), "ls n --date tomorrow -t life");
        assert_eq!(x("W #work -a"), "ls --date W -t work -a");
        assert_eq!(x("ls todo #work 1w"), "ls todo -t work --date 1w");
        // Not filter lines.
        assert_eq!(x("n tomorrow #life"), "n tomorrow #life");
        assert_eq!(x("n buy milk"), "n buy milk");
        assert_eq!(x("tomorrow n #life"), "tomorrow n #life");
        assert_eq!(x("today"), "today");
        assert_eq!(x("none"), "none");
        assert_eq!(x("#"), "#");
        assert_eq!(x("due fri"), "due fri");
    }
}
