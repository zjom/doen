use std::cmp::{Ordering, Reverse};

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
    Scheduled,
    Date,
    Created,
    Closed,
    Title,
    Status,
    Priority,
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
    pub scheduled: Option<Range>,
    pub due: Option<Range>,
    /// The accepted priorities. `None` in the list matches todos without a priority.
    pub priority: Option<Vec<Option<u8>>>,
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
                .scheduled
                .as_ref()
                .is_some_and(|r| m.kind == Kind::Note || !r.contains(m.scheduled))
            || self
                .due
                .as_ref()
                .is_some_and(|r| m.kind == Kind::Note || !r.contains(m.due))
            || self
                .priority
                .as_ref()
                .is_some_and(|ps| m.kind == Kind::Note || !ps.contains(&m.priority))
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

fn cmp_key<K: Ord>(a: K, b: K, reverse: bool) -> Ordering {
    if reverse { b.cmp(&a) } else { a.cmp(&b) }
}

/// Compares two optional keys. Items without the key come last, also when reversed.
fn cmp_opt<K: Ord>(a: Option<K>, b: Option<K>, reverse: bool) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => cmp_key(a, b, reverse),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn cmp_field(a: &Item, b: &Item, field: SortField, reverse: bool) -> Ordering {
    let (ma, mb) = (&a.meta, &b.meta);
    match field {
        SortField::Due => cmp_opt(ma.due, mb.due, reverse),
        SortField::Scheduled => cmp_opt(ma.scheduled, mb.scheduled, reverse),
        SortField::Date => cmp_key(ma.date, mb.date, reverse),
        SortField::Created => cmp_key(ma.created, mb.created, reverse),
        SortField::Closed => cmp_opt(ma.closed_on(), mb.closed_on(), reverse),
        SortField::Title => cmp_key(a.title().to_lowercase(), b.title().to_lowercase(), reverse),
        SortField::Status => cmp_opt(ma.current_status(), mb.current_status(), reverse),
        // The highest priority comes first.
        SortField::Priority => cmp_opt(ma.priority.map(Reverse), mb.priority.map(Reverse), reverse),
    }
}

/// Sorts a group of items of one kind. Notes have no scheduled date, due date, closed date, status
/// or priority and sort by date instead; by default, the newest notes come first.
fn sort(items: &mut [&Item], field: Option<SortField>, reverse: bool) {
    let Some(kind) = items.first().map(|i| i.kind()) else {
        return;
    };
    let (field, reverse) = match (field, kind) {
        (None, Kind::Todo) => (SortField::Due, reverse),
        (None, Kind::Note) => (SortField::Date, !reverse),
        (
            Some(
                SortField::Scheduled
                | SortField::Due
                | SortField::Closed
                | SortField::Status
                | SortField::Priority,
            ),
            Kind::Note,
        ) => (SortField::Date, reverse),
        (Some(f), _) => (f, reverse),
    };
    items.sort_by(|a, b| {
        cmp_field(a, b, field, reverse)
            .then_with(|| match field {
                // Todos due on the same day: the highest priority comes first.
                SortField::Due => cmp_field(a, b, SortField::Priority, reverse),
                _ => Ordering::Equal,
            })
            .then_with(|| cmp_key(a.meta.created, b.meta.created, reverse))
    });
}

/// Splits the items into sorted todos and notes.
pub fn select<'a>(
    items: impl Iterator<Item = &'a Item>,
    field: Option<SortField>,
    reverse: bool,
) -> (Vec<&'a Item>, Vec<&'a Item>) {
    let (mut todos, mut notes): (Vec<&Item>, Vec<&Item>) =
        items.partition(|i| i.kind() == Kind::Todo);
    sort(&mut todos, field, reverse);
    sort(&mut notes, field, reverse);
    (todos, notes)
}
