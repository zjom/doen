//! Scope, filters and sorting for the list commands.

use std::cmp::Ordering;

use clap::ValueEnum;
use regex::Regex;

use crate::dates::{Clock, Range};
use crate::item::{Item, Kind, Place, Status};
use crate::tags::TagFilter;

#[derive(Clone, Copy, Debug, Default, PartialEq, ValueEnum)]
pub enum StatusFilter {
    #[default]
    Open,
    Done,
    Drop,
    Closed,
    Any,
}

impl StatusFilter {
    fn accepts(self, s: Status) -> bool {
        match self {
            StatusFilter::Open => s == Status::Open,
            StatusFilter::Done => s == Status::Done,
            StatusFilter::Drop => s == Status::Drop,
            StatusFilter::Closed => s != Status::Open,
            StatusFilter::Any => true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
pub enum SortField {
    Due,
    Date,
    Created,
    Closed,
    Title,
    Status,
}

pub enum TextMatch {
    Sub(String),
    Re(Regex),
}

#[derive(Default)]
pub struct Query {
    pub kind: Option<Kind>,
    pub places: Vec<Place>,
    pub status: StatusFilter,
    pub tags: TagFilter,
    pub date: Option<Range>,
    pub created: Option<Range>,
    pub due: Option<Range>,
    pub closed: Option<Range>,
    pub text: Option<TextMatch>,
}

impl Query {
    pub fn accepts(&self, item: &Item, clock: &Clock) -> bool {
        let m = &item.meta;
        if self.kind.is_some_and(|k| k != m.kind) || !self.places.contains(&item.place) {
            return false;
        }
        if let Some(s) = m.current_status()
            && !self.status.accepts(s)
        {
            return false;
        }
        if !self.tags.accepts(&m.tags) {
            return false;
        }
        if self
            .date
            .as_ref()
            .is_some_and(|r| !r.contains(Some(m.date)))
            || self
                .created
                .as_ref()
                .is_some_and(|r| !r.contains(Some(clock.local_day(&m.created))))
            || self
                .due
                .as_ref()
                .is_some_and(|r| m.kind == Kind::Note || !r.contains(m.due))
            || self
                .closed
                .as_ref()
                .is_some_and(|r| !r.contains(m.closed_on()))
        {
            return false;
        }
        match &self.text {
            None => true,
            Some(TextMatch::Sub(s)) => item.body.to_lowercase().contains(s),
            Some(TextMatch::Re(re)) => re.is_match(&item.body),
        }
    }
}

/// Compares two optional keys. Items without the key come last, also when reversed.
fn cmp_opt<K: Ord>(a: Option<K>, b: Option<K>, reverse: bool) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => {
            if reverse {
                b.cmp(&a)
            } else {
                a.cmp(&b)
            }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn cmp_field(a: &Item, b: &Item, field: SortField, reverse: bool) -> Ordering {
    let (ma, mb) = (&a.meta, &b.meta);
    let note_field = |f: SortField, k: Kind| match (f, k) {
        (SortField::Due | SortField::Closed | SortField::Status, Kind::Note) => SortField::Date,
        _ => f,
    };
    let (fa, fb) = (note_field(field, ma.kind), note_field(field, mb.kind));
    if fa != fb {
        // A todo and a note in the same group never happen; keep the order stable.
        return Ordering::Equal;
    }
    match fa {
        SortField::Due => cmp_opt(ma.due, mb.due, reverse),
        SortField::Date => cmp_opt(Some(ma.date), Some(mb.date), reverse),
        SortField::Created => cmp_opt(Some(ma.created), Some(mb.created), reverse),
        SortField::Closed => cmp_opt(ma.closed_on(), mb.closed_on(), reverse),
        SortField::Title => cmp_opt(
            Some(a.title().to_lowercase()),
            Some(b.title().to_lowercase()),
            reverse,
        ),
        SortField::Status => cmp_opt(ma.current_status(), mb.current_status(), reverse),
    }
}

/// Sorts one group (todos or notes).
pub fn sort(items: &mut [&Item], field: Option<SortField>, reverse: bool) {
    items.sort_by(|a, b| match field {
        Some(f) => cmp_field(a, b, f, reverse)
            .then_with(|| cmp_opt(Some(a.meta.created), Some(b.meta.created), reverse)),
        None if a.kind() == Kind::Todo => cmp_opt(a.meta.due, b.meta.due, reverse)
            .then_with(|| cmp_opt(Some(a.meta.created), Some(b.meta.created), reverse)),
        None => cmp_opt(Some(a.meta.date), Some(b.meta.date), !reverse)
            .then_with(|| cmp_opt(Some(a.meta.created), Some(b.meta.created), !reverse)),
    });
}

/// Splits the matching items into sorted todos and notes, and applies the limit.
pub fn select<'a>(
    items: impl Iterator<Item = &'a Item>,
    field: Option<SortField>,
    reverse: bool,
    limit: Option<usize>,
) -> (Vec<&'a Item>, Vec<&'a Item>) {
    let (mut todos, mut notes): (Vec<&Item>, Vec<&Item>) =
        items.partition(|i| i.kind() == Kind::Todo);
    sort(&mut todos, field, reverse);
    sort(&mut notes, field, reverse);
    if let Some(n) = limit {
        todos.truncate(n);
        notes.truncate(n - todos.len());
    }
    (todos, notes)
}
