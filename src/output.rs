//! Output formats: the default table, `--ids`, `--porcelain` and `--json`.

use crate::app::App;
use crate::dates::{display, format_iso};
use crate::item::{Item, Kind, Status};

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

fn date_cell(item: &Item, app: &App) -> String {
    let d = match item.kind() {
        Kind::Todo => item.meta.due,
        Kind::Note => Some(item.meta.date),
    };
    d.map(|d| {
        display(
            d,
            &app.clock,
            app.cfg.relative_days,
            &app.cfg.date_format,
            is_overdue(item, app),
        )
    })
    .unwrap_or_default()
}

fn history(item: &Item) -> String {
    item.meta
        .status
        .iter()
        .map(|(d, s)| format!("{} {}", s.as_str(), d.format("%m-%d")))
        .collect::<Vec<_>>()
        .join(" → ")
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    format!("{s}{}", " ".repeat(w.saturating_sub(n)))
}

/// Prints groups of items in the requested format.
pub fn print_groups(app: &App, groups: &[(&str, Vec<&Item>)], format: Format, long: bool) {
    let all: Vec<&Item> = groups.iter().flat_map(|(_, g)| g.iter().copied()).collect();
    if print_machine(&all, format) {
        return;
    }
    let cell = |i: &Item| match i.meta.heading.as_ref().and(i.meta.current_status()) {
        Some(s) => format!("[{}] {}", s.checkbox(), i.title()),
        None if i.kind() == Kind::Todo => format!("[{}] {}", Status::Open.checkbox(), i.title()),
        None => i.title().to_string(),
    };
    let id_w = all.iter().map(|i| i.id().len()).max().unwrap_or(4);
    let cell_w = all
        .iter()
        .map(|i| cell(i).chars().count())
        .max()
        .unwrap_or(0);
    let date_w = all
        .iter()
        .map(|i| date_cell(i, app).chars().count())
        .max()
        .unwrap_or(0);
    let mut first = true;
    for (header, items) in groups {
        if items.is_empty() {
            continue;
        }
        if !first {
            println!();
        }
        first = false;
        println!("{}", app.paint("1", header));
        for i in items {
            let mut cols = vec![
                app.paint("33", &pad(i.id(), id_w)),
                pad(&cell(i), cell_w),
                match pad(&date_cell(i, app), date_w) {
                    d if is_overdue(i, app) => app.paint("31", &d),
                    d => d,
                },
            ];
            if long {
                cols.push(i.meta.date.format(&app.cfg.date_format).to_string());
                let created = i.meta.created.with_timezone(app.clock.now.offset());
                cols.push(
                    created
                        .format(&format!("{} %H:%M", app.cfg.date_format))
                        .to_string(),
                );
                if !i.meta.status.is_empty() {
                    cols.push(history(i));
                }
            }
            let tags: Vec<String> = i.meta.tags.iter().map(|t| format!("#{t}")).collect();
            cols.push(app.paint("36", &tags.join(" ")));
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
        parts.push(
            m.tags
                .iter()
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    parts.join(" · ")
}
