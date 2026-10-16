//! `ls`, `grep`, `trash` and `today`.

use crate::cli::args::{GrepCmd, LsCmd, TodayCmd, TrashCmd};
use crate::commands::App;
use crate::error::{Error, Result, usage};
use crate::filter::{Query, StatusFilter, select};
use crate::item::{Item, Kind, Place, Status};
use crate::output::print_groups;
use crate::tags::TagFilter;

pub fn parse_kind(s: Option<&str>) -> Result<Option<Kind>> {
    match s {
        None => Ok(None),
        Some("note" | "n") => Ok(Some(Kind::Note)),
        Some("todo" | "t") => Ok(Some(Kind::Todo)),
        Some(w) => Err(usage(format!(
            "unexpected argument '{w}': the kind is note|n or todo|t"
        ))),
    }
}

/// Runs a list query and prints the result. Returns the number of items.
fn list(app: &App, q: &Query, output: &crate::cli::args::OutputArgs) -> Result<usize> {
    let store = app.store()?;
    let matching = store.items.iter().filter(|i| q.accepts(i, &app.clock));
    let (todos, notes) = select(matching, output, app.cfg.default_sort.as_deref())?;
    let n = todos.len() + notes.len();
    print_groups(app, &[("TODOS", todos), ("NOTES", notes)], output);
    Ok(n)
}

pub fn ls(app: &App, c: LsCmd) -> Result<()> {
    let q = Query::new(&c.filter, parse_kind(c.kind.as_deref())?, None, &app.clock)?;
    list(app, &q, &c.output)?;
    Ok(())
}

pub fn grep(app: &App, c: GrepCmd) -> Result<()> {
    if c.filter.query.is_some() {
        return Err(usage("grep takes the pattern as an argument, not with -q"));
    }
    let q = Query::new(
        &c.filter,
        parse_kind(c.kind.as_deref())?,
        Some(&c.pattern),
        &app.clock,
    )?;
    match list(app, &q, &c.output)? {
        0 => Err(Error::NoMatches),
        _ => Ok(()),
    }
}

pub fn trash(app: &App, mut c: TrashCmd) -> Result<()> {
    if c.empty {
        let mut tx = app.begin()?;
        let trashed: Vec<usize> = (0..tx.store.items.len())
            .filter(|&n| tx.store.items[n].place == Place::Trash)
            .collect();
        if trashed.is_empty() {
            eprintln!("doen: the trash is empty");
            return tx.finish(app);
        }
        for &n in &trashed {
            eprintln!("  {}", super::item_line(&tx.store.items[n]));
        }
        app.confirm(&format!("Delete {} items permanently?", trashed.len()))?;
        for &n in &trashed {
            let item = tx.store.items[n].clone();
            tx.store.delete(&item, &mut tx.changes)?;
            tx.changes
                .lines
                .push(format!("deleted  {}  {}", item.id(), item.title()));
        }
        return tx.finish(app);
    }
    if c.filter.all || c.filter.archived {
        return Err(usage("trash lists only the trash"));
    }
    c.filter.trashed = true;
    let q = Query::new(&c.filter, parse_kind(c.kind.as_deref())?, None, &app.clock)?;
    list(app, &q, &c.output)?;
    Ok(())
}

pub fn today(app: &App, c: TodayCmd) -> Result<()> {
    let tags = TagFilter::new(&c.tag, &c.exclude_tag).map_err(usage)?;
    let q = Query::scope(vec![Place::Root], StatusFilter::Any, tags);
    let store = app.store()?;
    let today = app.clock.today;
    let open_todo = |i: &&Item| i.meta.current_status() == Some(Status::Open);
    let base: Vec<&Item> = store
        .items
        .iter()
        .filter(|i| q.accepts(i, &app.clock))
        .collect();
    let overdue = base
        .iter()
        .copied()
        .filter(open_todo)
        .filter(|i| i.meta.due.is_some_and(|d| d < today));
    let due = base
        .iter()
        .copied()
        .filter(open_todo)
        .filter(|i| i.meta.due == Some(today));
    let notes = base
        .iter()
        .copied()
        .filter(|i| i.kind() == Kind::Note && i.meta.date == today);
    let mut output = c.output.clone();
    let limit = output.limit.take();
    let (overdue, _) = select(overdue, &output, app.cfg.default_sort.as_deref())?;
    let (due, _) = select(due, &output, app.cfg.default_sort.as_deref())?;
    let (_, notes) = select(notes, &output, app.cfg.default_sort.as_deref())?;
    let mut groups = vec![("OVERDUE", overdue), ("DUE TODAY", due), ("NOTES", notes)];
    if let Some(mut left) = limit {
        for (_, g) in &mut groups {
            g.truncate(left);
            left -= g.len();
        }
    }
    print_groups(app, &groups, &output);
    Ok(())
}
