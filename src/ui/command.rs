//! The command line of the interactive mode: `:` takes a doen command, as on the command line.
//! The list commands replace the item list, `note` and `todo` create an item and the other
//! commands run as on the command line, in a transaction of the ui. `check` shows its report in a
//! popup. A command without IDs acts on the selected item. `closed` is only in the ui: it lists
//! closed todos, as `ls --closed`.

use std::ffi::OsString;

use clap::error::ErrorKind;

use crate::app::App;
use crate::cli::args::{Cli, Command, CreateCmd, OutputArgs, TagAction, TrashCmd};
use crate::cli::resolve::{Resolved, resolve};
use crate::cli::{command_word, normalize, try_parse};
use crate::commands::{self, check, create, list};
use crate::complete::UNAVAILABLE;
use crate::dates::{Clock, parse_range};
use crate::error::{Error, Result, usage};
use crate::filter::{Query, SortField};
use crate::ids;
use crate::item::{Item, Kind};
use crate::output::limit;

use super::{ConfirmAction, EMPTY_TRASH, Focus, Popup, Ui, changes_summary, with_notices};

/// The item list of a list command.
pub struct Listing {
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
    fn new(kind: ListKind, output: &OutputArgs) -> Listing {
        Listing {
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
                list::groups(items.filter(|i| q.accepts(i, clock)), sort, self.reverse)
            }
            ListKind::Agenda(q) => list::agenda(
                items.filter(|i| q.accepts(i, clock)),
                clock.today,
                sort,
                self.reverse,
            ),
            ListKind::Ids(ids) => {
                let shown = items.filter(|i| ids.iter().any(|id| id == i.id()));
                list::groups(shown, sort, self.reverse)
            }
        };
        limit(&mut groups, self.limit);
        groups
    }
}

/// Splits a command line into words as a shell does, with quotes and backslashes.
pub(super) fn split(line: &str) -> std::result::Result<Vec<String>, &'static str> {
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
/// are kept as they are. `closed [range]` lists as `ls --closed [range]`, newest first unless
/// `--sort` or `-r` is given. Other lines are returned unchanged.
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
    let (kind, filters) = if first == "closed" {
        let mut argv = vec!["ls".to_string(), "--closed".to_string()];
        let mut rest = &lead[1..];
        if let Some(r) = rest.first().filter(|r| !r.starts_with('#') && !is_kind(r)) {
            argv.push(r.clone());
            rest = &rest[1..];
        }
        for w in rest {
            match filter_word(w, clock) {
                Some(f) => argv.extend(f),
                // The kind goes before `--closed`, which would take it as the range.
                None if is_kind(w) => argv.insert(1, w.clone()),
                None => argv.push(w.clone()),
            }
        }
        let sorted = options.iter().any(|o| {
            o == "-r" || o == "--sort" || o.starts_with("--sort=") || o.starts_with("--rev")
        });
        if !sorted {
            argv.extend(["--sort", "closed", "-r"].map(String::from));
        }
        argv.extend_from_slice(options);
        return argv;
    } else if resolved == Resolved::Command("ls".into()) {
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

/// The IDs of a command that changes items.
fn ids(cmd: &Command) -> &[String] {
    match cmd {
        Command::Title(c) => std::slice::from_ref(&c.id),
        Command::Schedule(c) | Command::Due(c) | Command::Date(c) => &c.id,
        Command::Priority(c) => &c.id,
        Command::Tag {
            action: TagAction::Add(t) | TagAction::Rm(t),
        } => &t.id,
        Command::Done(c) | Command::Drop(c) | Command::Reopen(c) => &c.id,
        Command::Archive(c) | Command::Unarchive(c) | Command::Restore(c) => &c.id,
        Command::Rm(c) => &c.id,
        _ => &[],
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
        let argv = normalize(words.into_iter().map(OsString::from).collect())?;
        let name = argv.get(command_word(&argv)).and_then(|w| w.to_str());
        if name.is_some_and(|n| UNAVAILABLE.contains(&n)) {
            return Err(usage("the command is not available in the ui"));
        }
        let cli = self.parse(argv)?;
        let g = &cli.global;
        if g.dir.is_some() {
            return Err(usage("--dir is not available in the ui"));
        }
        if ids(&cli.command).iter().any(|id| id == "-") {
            return Err(usage("- reads stdin, which is not available in the ui"));
        }
        // `--color` does nothing in the ui.
        let yes = g.yes;
        let clock = &self.app.clock;
        match cli.command {
            Command::Ls(c) => {
                let q = list::query(&c.filter, c.kind, None, clock)?;
                self.show_listing(Listing::new(ListKind::Query(q), &c.output));
            }
            Command::Grep(c) => {
                let q = list::grep_query(&c, clock)?;
                self.show_listing(Listing::new(ListKind::Query(q), &c.output));
            }
            cmd @ Command::Trash(TrashCmd { empty: true, .. }) if !yes => {
                self.confirm(EMPTY_TRASH.into(), ConfirmAction::Command(Box::new(cmd)))
            }
            Command::Trash(c) if !c.empty => {
                let q = list::trash_query(&c, clock)?;
                self.show_listing(Listing::new(ListKind::Query(q), &c.output));
            }
            Command::Today(c) => {
                let q = list::today_query(&c)?;
                self.show_listing(Listing::new(ListKind::Agenda(q), &c.output));
            }
            Command::Show(c) => {
                let mut shown = vec![];
                for id in &c.id {
                    let n = ids::resolve(&mut self.store, id, true)?;
                    shown.push(self.store.items[n].id().to_string());
                }
                let keep = shown.first().cloned();
                self.show_listing(Listing::new(ListKind::Ids(shown), &OutputArgs::default()));
                self.rebuild(keep);
            }
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
                let question = if c.force {
                    format!("Delete {titles} permanently?")
                } else {
                    format!("Move {titles} to the trash?")
                };
                self.confirm(question, ConfirmAction::Command(Box::new(Command::Rm(c))));
            }
            cmd => self.run_command(cmd, yes),
        }
        Ok(None)
    }

    /// Creates an item. Returns the ID if it is to be edited.
    pub(super) fn create_cmd(&mut self, kind: Kind, c: CreateCmd) -> Result<Option<String>> {
        if c.title.join("").trim().is_empty() {
            return Err(usage("the title cannot be empty"));
        }
        if c.content.as_deref() == Some("-") {
            return Err(usage("-c - reads stdin, which is not available in the ui"));
        }
        let edit = c.edit;
        let app = &self.app;
        let r = create::Draft::new(app, kind, c).and_then(|draft| {
            let mut tx = app.begin()?;
            let id = create::create(app, &mut tx, &draft)?;
            Ok((id, tx.close(app)))
        });
        let id = r.as_ref().ok().map(|(id, _)| id.clone());
        let msg = r.map(|(id, changes)| {
            super::message(std::iter::once(format!("created  {id}")).chain(changes.notices))
        });
        self.after(msg, id.as_deref().unwrap_or(""));
        Ok(id.filter(|_| edit))
    }

    /// Parses the normalized words. If IDs are missing, the selected item is the ID.
    fn parse(&self, mut argv: Vec<OsString>) -> Result<Cli> {
        let parse = |a: &[OsString]| try_parse::<Cli>(a.iter().cloned());
        let e = match parse(&argv) {
            Ok(cli) => return Ok(cli),
            Err(e) => e,
        };
        if e.kind() == ErrorKind::MissingRequiredArgument
            && let Some(id) = self.selected_id()
        {
            argv.push(id.into());
            if let Ok(cli) = parse(&argv) {
                return Ok(cli);
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
        self.focus = Focus::Items;
        self.cursor = 0;
        self.rebuild(None);
    }

    /// Returns from a listing to the open todos.
    pub(super) fn clear_listing(&mut self) {
        self.listing = None;
        self.cursor = 0;
        self.rebuild(None);
    }

    /// Runs a command that changes the store. With `confirmed`, the confirmations are
    /// answered yes; else a confirmation is the question of `check`, which the ui asks in a popup
    /// after the report.
    pub(super) fn run_command(&mut self, cmd: Command, confirmed: bool) {
        let keep = self.selected_id().unwrap_or_default();
        let mut asked = None;
        let app = &self.app;
        let (r, changes) = self.changes(|tx| {
            commands::dispatch(app, tx, cmd, &mut |tx, _, question| {
                if confirmed {
                    return Ok(());
                }
                let paths = check::temp_files(&tx.store.root);
                asked = Some((
                    question.to_string(),
                    paths.into_iter().map(|(p, _)| p).collect(),
                ));
                Err(Error::Declined("cancelled".into()))
            })
        });
        let msg = match r {
            // Problems that `check` did not fix: the report shows them.
            Ok(()) | Err(Error::Silent(_)) => match changes_summary(&changes) {
                m if m.is_empty() && changes.report.is_empty() => Ok("no problems".into()),
                m => Ok(m),
            },
            Err(e) => Err(with_notices(e, changes.notices)),
        };
        self.after(msg, &keep);
        if !changes.report.is_empty() {
            let next = asked.map(|(message, paths)| {
                let action = ConfirmAction::DeleteTemp(paths);
                Box::new(Popup::Confirm { message, action })
            });
            self.popup = Some(Popup::Report {
                title: "Check".into(),
                lines: changes.report,
                scroll: 0,
                next,
            });
        }
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
        assert_eq!(x("closed"), "ls --closed --sort closed -r");
        assert_eq!(
            x("closed 1w #work"),
            "ls --closed 1w -t work --sort closed -r"
        );
        assert_eq!(x("closed #work -r"), "ls --closed -t work -r");
        assert_eq!(x("closed t"), "ls t --closed --sort closed -r");
        assert_eq!(x("closed --sort title"), "ls --closed --sort title");
    }
}
