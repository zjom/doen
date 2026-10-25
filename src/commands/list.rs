use chrono::NaiveDate;
use regex::Regex;

use crate::app::App;
use crate::banner::{print_footer, print_header};
use crate::cli::args::{FilterArgs, GrepCmd, LsCmd, OutputArgs, TodayCmd, TrashCmd};
use crate::dates::{Clock, Range, parse_range};
use crate::error::{Error, Result, usage};
use crate::filter::{Query, SortField, StatusFilter, TextMatch, select};
use crate::item::{Item, Kind, Place, Status, parse_priority};
use crate::output::{Format, limit, print_groups};
use crate::tags::TagFilter;
use crate::term::stdout_is_terminal;

fn range(arg: Option<&str>, clock: &Clock) -> Result<Option<Range>> {
    arg.map(|s| parse_range(s, clock).map_err(usage))
        .transpose()
}

pub fn query(
    f: &FilterArgs,
    kind: Option<Kind>,
    text: Option<&str>,
    clock: &Clock,
) -> Result<Query> {
    let implies_todo = f.status.is_some()
        || f.scheduled.is_some()
        || f.due.is_some()
        || f.priority.is_some()
        || f.overdue
        || f.closed.is_some();
    if implies_todo && kind == Some(Kind::Note) {
        return Err(usage(
            "-s, -S, -d, -p, --overdue and --closed apply only to todos",
        ));
    }
    let kind = if implies_todo { Some(Kind::Todo) } else { kind };
    if [f.all, f.archived, f.trashed]
        .iter()
        .filter(|b| **b)
        .count()
        > 1
    {
        return Err(usage("use only one of -a, --archived and --trashed"));
    }
    let mut due = range(f.due.as_deref(), clock)?;
    if f.overdue {
        if due.is_some() {
            return Err(usage("--overdue cannot be used with -d"));
        }
        due = Some(Range::Days {
            start: None,
            end: clock.today.pred_opt(),
        });
        if f.status.is_some_and(|s| s != StatusFilter::Open) {
            return Err(usage("--overdue selects open todos"));
        }
    }
    let status = f.status.unwrap_or(if f.closed.is_some() {
        StatusFilter::Closed
    } else if f.overdue {
        StatusFilter::Open
    } else if f.all || f.archived || f.trashed {
        StatusFilter::Any
    } else {
        StatusFilter::Open
    });
    let places = if f.trashed {
        vec![Place::Trash]
    } else if f.archived {
        vec![Place::Archive]
    } else if f.all || (status != StatusFilter::Open && kind == Some(Kind::Todo)) {
        vec![Place::Root, Place::Archive]
    } else {
        vec![Place::Root]
    };
    let text = match text.or(f.query.as_deref()) {
        None if f.regex => return Err(usage("-E requires -q")),
        None => None,
        Some(q) if f.regex => Some(TextMatch::Re(
            Regex::new(&format!("(?i){q}")).map_err(|e| usage(format!("invalid regex: {e}")))?,
        )),
        Some(q) => Some(TextMatch::Sub(q.to_lowercase())),
    };
    Ok(Query {
        kind,
        places,
        status,
        tags: TagFilter::new(&f.tags.tag, &f.tags.exclude_tag).map_err(usage)?,
        date: range(f.date.as_deref(), clock)?,
        created: range(f.created.as_deref(), clock)?,
        scheduled: range(f.scheduled.as_deref(), clock)?,
        due,
        priority: f.priority.as_deref().map(priorities).transpose()?,
        closed: range(f.closed.as_deref(), clock)?,
        text,
    })
}

/// Parses a `-p` value: priorities and `none`, separated by commas.
fn priorities(s: &str) -> Result<Vec<Option<u8>>> {
    s.split(',')
        .map(|p| match p.trim() {
            "none" => Ok(None),
            p => parse_priority(p)
                .map(Some)
                .ok_or_else(|| usage(format!("invalid priority '{p}': use 1, 2, 3 or none"))),
        })
        .collect()
}

/// Limits and prints groups of items. Returns the number of items printed.
fn print(app: &App, mut groups: Vec<(&str, Vec<&Item>)>, output: &OutputArgs) -> usize {
    limit(&mut groups, output.limit);
    print_groups(app, &groups, output.format.format(), output.long);
    groups.iter().map(|(_, g)| g.len()).sum()
}

/// Runs a list query and prints the result. Returns the number of items.
fn list(app: &App, q: &Query, output: &OutputArgs) -> Result<usize> {
    let store = app.store(&q.places)?;
    let matching = store.items.iter().filter(|i| q.accepts(i, &app.clock));
    let (todos, notes) = select(
        matching,
        output.sort.or(app.cfg.default_sort),
        output.reverse,
    );
    Ok(print(app, vec![("TODOS", todos), ("NOTES", notes)], output))
}

pub fn ls(app: &App, c: LsCmd) -> Result<()> {
    let q = query(&c.filter, c.kind, None, &app.clock)?;
    // The banner and the cheatsheet are for people: only on a terminal, never with a machine format.
    let decorate = c.output.format.format() == Format::Human && stdout_is_terminal();
    if decorate && app.cfg.ls_header && !c.no_header {
        print_header(app);
    }
    list(app, &q, &c.output)?;
    if decorate && app.cfg.ls_footer && !c.no_footer {
        print_footer(app);
    }
    Ok(())
}

pub fn grep_query(c: &GrepCmd, clock: &Clock) -> Result<Query> {
    if c.filter.query.is_some() {
        return Err(usage("grep takes the pattern as an argument, not with -q"));
    }
    query(&c.filter, c.kind, Some(&c.pattern), clock)
}

pub fn grep(app: &App, c: GrepCmd) -> Result<()> {
    let q = grep_query(&c, &app.clock)?;
    match list(app, &q, &c.output)? {
        0 => Err(Error::Silent(5)),
        _ => Ok(()),
    }
}

pub fn trash_query(c: &TrashCmd, clock: &Clock) -> Result<Query> {
    if c.filter.all || c.filter.archived {
        return Err(usage("trash lists only the trash"));
    }
    let f = FilterArgs {
        trashed: true,
        ..c.filter.clone()
    };
    query(&f, c.kind, None, clock)
}

pub fn trash(app: &App, c: TrashCmd) -> Result<()> {
    let q = trash_query(&c, &app.clock)?;
    list(app, &q, &c.output)?;
    Ok(())
}

/// The items that `doen today` sorts into the groups of `agenda`.
pub fn today_query(c: &TodayCmd) -> Result<Query> {
    Ok(Query {
        places: vec![Place::Root],
        status: StatusFilter::Any,
        tags: TagFilter::new(&c.tags.tag, &c.tags.exclude_tag).map_err(usage)?,
        ..Query::default()
    })
}

pub fn today(app: &App, c: TodayCmd) -> Result<()> {
    let q = today_query(&c)?;
    let store = app.store(&q.places)?;
    let base = store.items.iter().filter(|i| q.accepts(i, &app.clock));
    let groups = agenda(
        base,
        app.clock.today,
        c.output.sort.or(app.cfg.default_sort),
        c.output.reverse,
    );
    print(app, groups, &c.output);
    Ok(())
}

/// The groups of `doen today`: overdue todos, todos due today, scheduled todos, today's notes.
pub fn agenda<'a>(
    items: impl Iterator<Item = &'a Item>,
    today: NaiveDate,
    field: Option<SortField>,
    reverse: bool,
) -> Vec<(&'static str, Vec<&'a Item>)> {
    let open_todo = |i: &&Item| i.meta.current_status() == Some(Status::Open);
    let base: Vec<&Item> = items.collect();
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
    // Open todos whose scheduled day has come, and that are not already listed as due.
    let scheduled = base
        .iter()
        .copied()
        .filter(open_todo)
        .filter(|i| i.meta.scheduled.is_some_and(|d| d <= today))
        .filter(|i| i.meta.due.is_none_or(|d| d > today));
    let notes = base
        .iter()
        .copied()
        .filter(|i| i.kind() == Kind::Note && i.meta.date == today);
    vec![
        ("OVERDUE", select(overdue, field, reverse).0),
        ("DUE TODAY", select(due, field, reverse).0),
        ("SCHEDULED", select(scheduled, field, reverse).0),
        ("NOTES", select(notes, field, reverse).1),
    ]
}
