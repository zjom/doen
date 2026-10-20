use chrono::NaiveDate;

use crate::app::App;
use crate::dates::{display, format_iso};
use crate::item::{Item, Kind, Status};
use crate::term;

pub fn porcelain_line(item: &Item) -> String {
    let m = &item.meta;
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    [
        m.id.clone(),
        m.kind.as_str().to_string(),
        m.current_status()
            .map(|s| s.as_str().to_string())
            .unwrap_or_default(),
        item.place.as_str().to_string(),
        format_iso(m.date),
        m.due.map(format_iso).unwrap_or_default(),
        clean(item.title()),
        m.tags.join(","),
        m.priority.map(|p| p.to_string()).unwrap_or_default(),
        m.scheduled.map(format_iso).unwrap_or_default(),
    ]
    .join("\t")
}

pub fn json_item(item: &Item) -> serde_json::Value {
    let mut map = item.doc.to_json();
    map.insert("title".into(), item.title().into());
    let path = std::path::absolute(&item.path).unwrap_or_else(|_| item.path.clone());
    map.insert("path".into(), path.to_string_lossy().into());
    map.insert("place".into(), item.place.as_str().into());
    map.insert(
        "current_status".into(),
        item.meta
            .current_status()
            .map_or(serde_json::Value::Null, |s| s.as_str().into()),
    );
    serde_json::Value::Object(map)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Format {
    Human,
    Ids,
    Porcelain,
    Json,
}

/// Prints items in a machine format. Returns `false` for `Format::Human`.
pub fn print_machine(items: &[&Item], format: Format) -> bool {
    match format {
        Format::Human => return false,
        Format::Ids => items.iter().for_each(|i| println!("{}", i.id())),
        Format::Porcelain => items.iter().for_each(|i| println!("{}", porcelain_line(i))),
        Format::Json => {
            let v: Vec<serde_json::Value> = items.iter().map(|i| json_item(i)).collect();
            println!("{}", serde_json::to_string_pretty(&v).expect("JSON"));
        }
    }
    true
}

pub fn is_overdue(item: &Item, app: &App) -> bool {
    item.meta.current_status() == Some(Status::Open)
        && item.meta.due.is_some_and(|d| d < app.clock.today)
}

/// The labels of the date columns, in the header line of a group of one kind.
fn date_labels(kind: Kind, long: bool) -> &'static [&'static str] {
    match (kind, long) {
        (Kind::Note, false) => &["", "", "date"],
        (Kind::Note, true) => &["", "", "created", "date"],
        (Kind::Todo, false) => &["scheduled", "due"],
        (Kind::Todo, true) => &["scheduled", "due", "created", "date", "status"],
    }
}

/// The date columns of a list row. Todos show the scheduled date and the due date. Notes show the
/// `date` field in the column after the due date. `long` adds the creation time, the date and the
/// status history of todos. With `long`, the creation time and the date of both kinds line up.
fn date_cells(item: &Item, app: &App, long: bool) -> Vec<String> {
    let show = |d: NaiveDate, overdue: bool| {
        display(
            d,
            &app.clock,
            app.cfg.relative_days,
            &app.cfg.date_format,
            overdue,
        )
    };
    let m = &item.meta;
    let created = m
        .created
        .with_timezone(app.clock.now.offset())
        .format(&format!("{} %H:%M", app.cfg.date_format))
        .to_string();
    match item.kind() {
        Kind::Note if long => vec![String::new(), String::new(), created, show(m.date, false)],
        Kind::Note => vec![String::new(), String::new(), show(m.date, false)],
        Kind::Todo => {
            let mut cells = vec![
                m.scheduled.map(|d| show(d, false)).unwrap_or_default(),
                m.due
                    .map(|d| show(d, is_overdue(item, app)))
                    .unwrap_or_default(),
            ];
            if long {
                cells.push(created);
                cells.push(show(m.date, false));
                cells.push(history(item));
            }
            cells
        }
    }
}

fn history(item: &Item) -> String {
    item.meta
        .status
        .iter()
        .map(|(d, s)| format!("{} {}", s.as_str(), d.format("%m-%d")))
        .collect::<Vec<_>>()
        .join(" → ")
}

/// The display width of `s`, without color codes.
fn width(s: &str) -> usize {
    let mut n = 0;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            chars.by_ref().find(|&c| c == 'm');
        } else {
            n += 1;
        }
    }
    n
}

/// Cuts `s` to at most `w` columns, ending it with `…` if it is cut. Color codes are kept.
fn truncate(s: &str, w: usize) -> String {
    if width(s) <= w {
        return s.to_string();
    }
    let keep = w.saturating_sub(1);
    let mut out = String::new();
    let mut n = 0;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            out.push(c);
            for c in chars.by_ref() {
                out.push(c);
                if c == 'm' {
                    break;
                }
            }
        } else if n < keep {
            out.push(c);
            n += 1;
        }
    }
    out.truncate(out.trim_end().len());
    out + if w > 0 { "…" } else { "" }
}

fn pad(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(width(s))))
}

fn hashtags(tags: &[String]) -> String {
    tags.iter()
        .map(|t| format!("#{t}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Keeps at most `limit` items in total, taken from the groups in order.
pub fn limit(groups: &mut [(&str, Vec<&Item>)], limit: Option<usize>) {
    if let Some(mut left) = limit {
        for (_, g) in groups {
            g.truncate(left);
            left -= g.len();
        }
    }
}

/// Prints groups of items in the requested format.
pub fn print_groups(app: &App, groups: &[(&str, Vec<&Item>)], format: Format, long: bool) {
    let all: Vec<&Item> = groups.iter().flat_map(|(_, g)| g.iter().copied()).collect();
    if print_machine(&all, format) {
        return;
    }
    // Priority marks (`!` to `!!!`) go between the checkbox and the title, in a column that
    // exists only if an item has a priority.
    let prio_w = all
        .iter()
        .filter_map(|i| i.meta.priority)
        .max()
        .unwrap_or(0) as usize;
    let title = |i: &Item| match prio_w {
        0 => i.title().to_string(),
        w => {
            let marks = "!".repeat(i.meta.priority.unwrap_or(0) as usize);
            format!("{} {}", pad(&app.paint("31", &marks), w), i.title())
        }
    };
    let cell = |i: &Item| match (i.meta.current_status(), &i.meta.heading) {
        (Some(s), Some(_)) => format!("[{}] {}", s.checkbox(), title(i)),
        (Some(_), None) => format!("[{}] {}", Status::Open.checkbox(), title(i)),
        (None, _) => i.title().to_string(),
    };
    struct Row<'a> {
        item: &'a Item,
        cell: String,
        dates: Vec<String>,
    }
    let groups: Vec<(&str, Vec<Row>)> = groups
        .iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(h, items)| {
            let rows = items.iter().map(|&item| Row {
                item,
                cell: cell(item),
                dates: date_cells(item, app, long),
            });
            (*h, rows.collect())
        })
        .collect();
    let width = |f: fn(&Row) -> usize| groups.iter().flat_map(|(_, r)| r).map(f).max().unwrap_or(0);
    let id_w = width(|r| r.item.id().len());
    let mut cell_w = width(|r| self::width(&r.cell));
    // The width of each date column, including its labels. A column that is blank in every row
    // takes no space.
    let n_dates = width(|r| r.dates.len());
    let date_ws: Vec<usize> = (0..n_dates)
        .map(|n| {
            let cells = groups
                .iter()
                .flat_map(|(_, r)| r)
                .filter_map(|r| r.dates.get(n));
            let w = cells.map(|d| d.chars().count()).max().unwrap_or(0);
            let labels = groups
                .iter()
                .filter_map(|(_, r)| date_labels(r[0].item.kind(), long).get(n));
            match w {
                0 => 0,
                w => labels.map(|l| l.len()).fold(w, usize::max),
            }
        })
        .collect();
    // On a terminal, the title column takes the width the other columns leave, but not less
    // than `MIN_CELL_W`. Longer titles end with `…`.
    const MIN_CELL_W: usize = 20;
    if let Some(term_w) = term::width() {
        let tags_w = width(|r| self::width(&hashtags(&r.item.meta.tags)));
        let others: usize = 2
            + id_w
            + 2
            + date_ws
                .iter()
                .filter(|&&w| w > 0)
                .map(|w| w + 2)
                .sum::<usize>()
            + if tags_w > 0 { tags_w + 2 } else { 0 };
        cell_w = cell_w.min(term_w.saturating_sub(others).max(MIN_CELL_W));
    }
    for (k, (header, rows)) in groups.iter().enumerate() {
        if k > 0 {
            println!();
        }
        // The group name, then the labels of the date columns above the columns.
        let mut cols = vec![pad(header, 2 + id_w + 2 + cell_w)];
        let labels = date_labels(rows[0].item.kind(), long);
        for (n, &w) in date_ws.iter().enumerate().filter(|&(_, &w)| w > 0) {
            cols.push(pad(labels.get(n).unwrap_or(&""), w));
        }
        println!("{}", app.paint("1", cols.join("  ").trim_end()));
        for Row {
            item: i,
            cell,
            dates,
        } in rows
        {
            let mut cols = vec![
                app.paint("33", &pad(i.id(), id_w)),
                pad(&truncate(cell, cell_w), cell_w),
            ];
            for (n, &w) in date_ws.iter().enumerate().filter(|&(_, &w)| w > 0) {
                let date = pad(dates.get(n).map_or("", |d| d), w);
                cols.push(if n == 1 && is_overdue(i, app) {
                    app.paint("31", &date)
                } else {
                    date
                });
            }
            cols.push(app.paint("36", &hashtags(&i.meta.tags)));
            println!("  {}", cols.join("  ").trim_end());
        }
    }
}

/// The one-line metadata summary of `doen show`.
pub fn summary(item: &Item, app: &App) -> String {
    let m = &item.meta;
    let mut parts = vec![m.id.clone(), m.kind.as_str().to_string()];
    if let Some(s) = m.current_status() {
        parts.push(s.as_str().to_string());
    }
    if let Some(p) = m.priority {
        parts.push(format!("priority {p}"));
    }
    if let Some(d) = m.scheduled {
        parts.push(format!("scheduled {}", d.format(&app.cfg.date_format)));
    }
    if let Some(d) = m.due {
        parts.push(format!("due {}", d.format(&app.cfg.date_format)));
    }
    if m.kind == Kind::Note || m.date != app.clock.local_day(&m.created) {
        parts.push(format!("date {}", m.date.format(&app.cfg.date_format)));
    }
    if let Some(d) = m.archived {
        parts.push(format!("archived {}", d.format(&app.cfg.date_format)));
    }
    if let Some(d) = m.trashed {
        parts.push(format!("trashed {}", d.format(&app.cfg.date_format)));
    }
    if !m.tags.is_empty() {
        parts.push(hashtags(&m.tags));
    }
    parts.join(" · ")
}
