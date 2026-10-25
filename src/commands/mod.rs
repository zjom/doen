pub mod check;
pub mod completion;
pub mod create;
pub mod edit;
pub mod init;
pub mod lifecycle;
pub mod list;
pub mod meta;
pub mod show;
pub mod status;
pub mod tags;

use std::io::Read;

use crate::app::{App, Tx};
use crate::cli::args::{Command, DateCmd, StatusCmd, TagAction};
use crate::error::{Error, Result, general, usage};
use crate::ids;
use crate::item::{DateField, Kind, Status, parse_priority_or_none};
use crate::store::Store;
use lifecycle::Lifecycle;

/// The confirmation of a destructive change: the items, by index, and the question. `Ok` goes
/// on, `Declined` cancels.
pub type Confirm<'a> = dyn FnMut(&mut Tx, &[usize], &str) -> Result<()> + 'a;

/// Runs a mutating command. `confirm` is asked before destructive changes.
pub fn dispatch(app: &App, tx: &mut Tx, cmd: Command, confirm: &mut Confirm) -> Result<()> {
    let today = app.clock.today;
    let date = |tx: &mut Tx, field: DateField, c: DateCmd| {
        let value = field.parse(&c.date, &app.clock)?;
        let idx = resolve_ids(&mut tx.store, &c.id, false)?;
        meta::set_date(tx, field, value, &c.date, &idx, today)
    };
    let status = |tx: &mut Tx, status: Status, c: StatusCmd| {
        let on = status::parse_on(c.on.as_deref(), &app.clock)?;
        let idx = resolve_ids(&mut tx.store, &c.id, false)?;
        status::set(tx, &app.cfg, status, on, &idx)
    };
    let lifecycle = |tx: &mut Tx, op: Lifecycle, ids: &[String]| {
        let idx = resolve_ids(&mut tx.store, ids, op == Lifecycle::Restore)?;
        lifecycle::apply(tx, op, &idx, today)
    };
    match cmd {
        Command::Title(c) => {
            let title = meta::parse_title(&c.title)?;
            let n = resolve_ids(&mut tx.store, std::slice::from_ref(&c.id), false)?[0];
            meta::set_title(tx, n, &title)
        }
        Command::Schedule(c) => date(tx, DateField::Scheduled, c),
        Command::Due(c) => date(tx, DateField::Due, c),
        Command::Date(c) => date(tx, DateField::Date, c),
        Command::Priority(c) => {
            let value = parse_priority_or_none(&c.priority).map_err(usage)?;
            let idx = resolve_ids(&mut tx.store, &c.id, false)?;
            meta::set_priority(tx, value, &idx)
        }
        Command::Tag { action } => {
            let (add, t) = match action {
                TagAction::Add(t) => (true, t),
                TagAction::Rm(t) => (false, t),
            };
            let list = meta::parse_tags(&t.tags)?;
            let idx = resolve_ids(&mut tx.store, &t.id, false)?;
            meta::tag(tx, add, &list, &idx)
        }
        Command::Done(c) => status(tx, Status::Done, c),
        Command::Drop(c) => status(tx, Status::Drop, c),
        Command::Reopen(c) => status(tx, Status::Open, c),
        Command::Archive(c) => lifecycle(tx, Lifecycle::Archive, &c.id),
        Command::Unarchive(c) => lifecycle(tx, Lifecycle::Unarchive, &c.id),
        Command::Restore(c) => lifecycle(tx, Lifecycle::Restore, &c.id),
        Command::Rm(c) if c.force => {
            let idx = resolve_ids(&mut tx.store, &c.id, true)?;
            confirm(
                tx,
                &idx,
                &lifecycle::question("Delete", idx.len(), " permanently"),
            )?;
            lifecycle::delete(tx, &idx)
        }
        Command::Rm(c) => {
            let idx = resolve_ids(&mut tx.store, &c.id, false)?;
            confirm(
                tx,
                &idx,
                &lifecycle::question("Move", idx.len(), " to the trash"),
            )?;
            lifecycle::apply(tx, Lifecycle::Trash, &idx, today)
        }
        Command::Trash(c) if c.empty => {
            let idx = lifecycle::trashed(&tx.store);
            if !idx.is_empty() {
                confirm(
                    tx,
                    &idx,
                    &lifecycle::question("Delete", idx.len(), " permanently"),
                )?;
            }
            lifecycle::empty_trash(tx)
        }
        Command::Check(c) => check::run(tx, c.fix, confirm),
        _ => Err(general("not a mutating command")),
    }
}

/// Resolves every ID before any change. The argument `-` reads whitespace-separated IDs from
/// stdin. Duplicate IDs resolve once.
pub fn resolve_ids(store: &mut Store, args: &[String], trash: bool) -> Result<Vec<usize>> {
    let mut expanded = vec![];
    let mut stdin_read = false;
    for a in args {
        if a != "-" {
            expanded.push(a.clone());
        } else if !stdin_read {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            expanded.extend(s.split_whitespace().map(String::from));
            stdin_read = true;
        }
    }
    if expanded.is_empty() {
        return Err(usage("no IDs given"));
    }
    let mut out = vec![];
    for a in &expanded {
        let n = ids::resolve(store, a, trash)?;
        if !out.contains(&n) {
            out.push(n);
        }
    }
    Ok(out)
}

pub fn require_todos(store: &Store, idx: &[usize], what: &str) -> Result<()> {
    match idx
        .iter()
        .map(|&n| &store.items[n])
        .find(|i| i.kind() == Kind::Note)
    {
        Some(i) => Err(only_todos(i.id(), what)),
        None => Ok(()),
    }
}

/// The error of a todo-only change, as `a priority`, to the note `id`.
pub fn only_todos(id: &str, what: &str) -> Error {
    usage(format!("{id} is a note: only todos have {what}"))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;
    use crate::cli::args::Cli;
    use crate::cli::try_parse;
    use crate::error::Error;
    use crate::item::Place;
    use crate::store::Changes;
    use crate::testing::{app, create};

    struct Run {
        result: Result<()>,
        changes: Changes,
        questions: Vec<String>,
    }

    /// Runs the command line `line` in a transaction. `accept` answers the confirmations.
    fn run(app: &App, line: &str, accept: bool) -> Run {
        let argv = line.split(' ').map(OsString::from);
        let cmd = try_parse::<Cli>(argv).unwrap().command;
        let mut questions = vec![];
        let mut tx = app.begin().unwrap();
        let result = dispatch(app, &mut tx, cmd, &mut |_, _, q| {
            questions.push(q.to_string());
            match accept {
                true => Ok(()),
                false => Err(Error::Declined("cancelled".into())),
            }
        });
        Run {
            result,
            changes: tx.close(app),
            questions,
        }
    }

    fn shown(r: &Run) -> Vec<String> {
        r.changes.shown.iter().map(|c| c.line()).collect()
    }

    fn place(app: &App, id: &str) -> Place {
        let mut store = app.store(&[]).unwrap();
        let n = ids::resolve(&mut store, id, true).unwrap();
        store.items[n].place
    }

    #[test]
    fn dates() {
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path());
        let id = create(&app, Kind::Todo, "x");
        let r = run(&app, &format!("due yesterday {id}"), true);
        assert!(r.result.is_ok());
        assert_eq!(shown(&r), [format!("due  {id}  x")]);
        assert_eq!(r.changes.notices, ["warning: the due date is in the past"]);
        let r = run(&app, &format!("due 26-10-15 {id}"), true);
        assert!(shown(&r).is_empty());
        assert_eq!(
            r.changes.notices[1],
            format!("warning: {id}: due is already 26-10-15")
        );
        let note = create(&app, Kind::Note, "n");
        let r = run(&app, &format!("schedule fri {id} {note}"), true);
        assert_eq!(r.result.unwrap_err().exit_code(), 2);
        assert!(r.changes.paths.is_empty());
    }

    #[test]
    fn priority_status_and_tags() {
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path());
        let id = create(&app, Kind::Todo, "x");
        run(&app, &format!("priority 2 {id}"), true);
        let r = run(&app, &format!("priority 2 {id}"), true);
        assert_eq!(
            r.changes.notices,
            [format!("warning: {id}: priority is already 2")]
        );
        assert_eq!(shown(&run(&app, &format!("done {id}"), true)).len(), 1);
        assert_eq!(place(&app, &id), Place::Archive);
        let r = run(&app, &format!("done {id}"), true);
        assert_eq!(
            r.changes.notices,
            [format!("warning: {id} is already done")]
        );
        run(&app, &format!("tag add a,b {id}"), true);
        let r = run(&app, &format!("tag add a {id}"), true);
        assert_eq!(
            r.changes.notices,
            [format!("warning: {id}: the item already has these tags")]
        );
        let r = run(&app, &format!("tag rm b {id}"), true);
        assert_eq!(shown(&r), [format!("untag  {id}  x")]);
    }

    #[test]
    fn lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path());
        let id = create(&app, Kind::Note, "x");
        let r = run(&app, &format!("restore {id}"), true);
        assert_eq!(
            r.changes.notices,
            [format!("warning: {id} is not in the trash")]
        );
        run(&app, &format!("archive {id}"), true);
        let r = run(&app, &format!("archive {id}"), true);
        assert_eq!(
            r.changes.notices,
            [format!("warning: {id} is already archived")]
        );
        let r = run(&app, &format!("rm {id}"), false);
        assert!(matches!(r.result, Err(Error::Declined(_))));
        assert_eq!(r.questions, ["Move this item to the trash?"]);
        assert_eq!(place(&app, &id), Place::Archive);
        run(&app, &format!("rm {id}"), true);
        assert_eq!(place(&app, &id), Place::Trash);
        let r = run(&app, "trash --empty", true);
        assert_eq!(r.questions, ["Delete this item permanently?"]);
        assert_eq!(shown(&r), [format!("deleted  {id}  x")]);
        let r = run(&app, "trash --empty", true);
        assert!(r.questions.is_empty());
        assert_eq!(r.changes.notices, ["the trash is empty"]);
    }

    #[test]
    fn check_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path());
        let id = create(&app, Kind::Todo, "leftover");
        let mut store = app.store(&[]).unwrap();
        let n = ids::resolve(&mut store, &id, false).unwrap();
        let tmp = dir.path().join(".doen-tmp-abc.md");
        std::fs::copy(&store.items[n].path, &tmp).unwrap();
        let r = run(&app, "check", false);
        assert!(matches!(r.result, Err(Error::Silent(1))));
        assert_eq!(
            r.changes.report,
            [format!("{}: temporary file: leftover", tmp.display())]
        );
        assert_eq!(
            r.questions,
            ["Delete 1 temporary file? An open editor may still use it."]
        );
        assert!(tmp.exists());
        let r = run(&app, "check", true);
        assert!(r.result.is_ok());
        assert_eq!(r.changes.report[1], format!("{}: deleted", tmp.display()));
        assert!(!tmp.exists());
    }
}
