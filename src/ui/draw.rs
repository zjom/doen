//! Rendering of the interactive mode.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::keys::{ACTIONS, Action, Section};
use super::{Focus, Popup, Row, Ui};
use crate::dates::display;
use crate::item::{Item, Kind, Status};
use crate::output::{is_overdue, summary};

/// Below this width, the preview is below the items.
const PREVIEW_MIN_WIDTH: u16 = 90;
/// Below this height, the preview is not shown below the items.
const PREVIEW_MIN_HEIGHT: u16 = 16;

pub fn draw(f: &mut Frame, ui: &mut Ui) {
    let [bar, main] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(f.area());
    let [items, preview] = if !ui.show_preview {
        [main, Rect::default()]
    } else if main.width >= PREVIEW_MIN_WIDTH {
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(main)
    } else if main.height >= PREVIEW_MIN_HEIGHT {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(main)
    } else {
        [main, Rect::default()]
    };

    draw_items(f, ui, items);
    if !preview.is_empty() {
        draw_preview(f, ui, preview);
    }
    draw_bar(f, ui, bar);
    match &ui.popup {
        Some(Popup::Input { title, input, .. }) => {
            let area = centered(f.area(), 60, 3);
            f.render_widget(Clear, area);
            let block = panel(title, true).title_bottom(hints(
                ui,
                &[(Action::Submit, "ok"), (Action::Back, "cancel")],
            ));
            let inner = block.inner(area);
            f.render_widget(Paragraph::new(input.text()).block(block), area);
            f.set_cursor_position((inner.x + input.cursor_x(), inner.y));
        }
        Some(Popup::Confirm { title, message, .. }) => {
            let area = centered(f.area(), 50, 4);
            f.render_widget(Clear, area);
            let block = panel(title, true)
                .title_bottom(hints(ui, &[(Action::Confirm, "yes")]) + "· other: cancel ");
            f.render_widget(
                Paragraph::new(message.as_str())
                    .wrap(Wrap { trim: true })
                    .block(block),
                area,
            );
        }
        Some(Popup::Help { scroll }) => {
            let area = centered(f.area(), 100, f.area().height.saturating_sub(4));
            f.render_widget(Clear, area);
            let block = panel("Keybindings", true).title_bottom(
                hints(ui, &[(Action::Down, "scroll"), (Action::Up, "scroll")]) + "· other: close ",
            );
            f.render_widget(
                Paragraph::new(help_text(ui))
                    .block(block)
                    .scroll((*scroll, 0)),
                area,
            );
        }
        None => {}
    }
}

fn panel<'a>(title: &str, active: bool) -> Block<'a> {
    let style = if active {
        Style::new().fg(Color::Green).bold()
    } else {
        Style::new()
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(style)
        .title(format!(" {title} "))
}

fn highlight(active: bool) -> Style {
    if active {
        Style::new().bg(Color::Blue).add_modifier(Modifier::BOLD)
    } else {
        Style::new().bg(Color::DarkGray)
    }
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(4));
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 3,
        width: w,
        height: h,
    }
}

fn draw_items(f: &mut Frame, ui: &mut Ui, area: Rect) {
    let active = ui.focus == Focus::Items;
    let title = match &ui.listing {
        Some(l) => format!("[1] :{}", l.line),
        None => "[1] todos".to_string(),
    };
    let mut block = panel(&title, active);
    if !ui.item_rows.is_empty() {
        block = block.title_bottom(
            Line::from(format!(" {} of {} ", ui.cursor + 1, ui.item_rows.len())).right_aligned(),
        );
    }
    if ui.searching || !ui.search.text().is_empty() {
        let style = if ui.searching {
            Style::new().fg(Color::Yellow)
        } else {
            Style::new()
        };
        block = block.title_bottom(Span::styled(format!(" /{} ", ui.search.text()), style));
    }
    let inner = block.inner(area);
    if ui.searching {
        f.set_cursor_position((inner.x + 2 + ui.search.cursor_x(), area.bottom() - 1));
    }
    if ui.rows.is_empty() {
        let empty = Paragraph::new(" nothing here".dim()).block(block);
        f.render_widget(empty, area);
        return;
    }
    let w = inner.width as usize;
    let lines: Vec<ListItem> = ui
        .rows
        .iter()
        .map(|r| match r {
            Row::Header(h, n) => Line::from(vec![
                Span::styled(format!(" {h}"), Style::new().bold().fg(Color::Magenta)),
                format!(" {n}").dim(),
            ])
            .into(),
            Row::Item(n) => item_line(&ui.store.items[*n], ui, w).into(),
        })
        .collect();
    let list = List::new(lines)
        .block(block)
        .highlight_style(highlight(active));
    f.render_stateful_widget(list, area, &mut ui.list_state);
}

/// Cuts `s` to at most `w` columns, ending it with `…` if it is cut.
fn truncate(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    let mut out = String::new();
    let mut n = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if n + cw + 1 > w {
            break;
        }
        out.push(c);
        n += cw;
    }
    if w > 0 {
        out.push('…');
    }
    out
}

fn item_line<'a>(item: &Item, ui: &Ui, width: usize) -> Line<'a> {
    let app = &ui.app;
    let m = &item.meta;
    let show = |d, overdue| {
        display(
            d,
            &app.clock,
            app.cfg.relative_days,
            &app.cfg.date_format,
            overdue,
        )
    };
    let status = m.current_status();
    let closed = status.is_some_and(|s| s != Status::Open);

    let mut left = vec![
        " ".into(),
        Span::styled(item.id().to_string(), Style::new().fg(Color::Yellow)),
        " ".into(),
    ];
    if let Some(s) = status {
        let style = match s {
            Status::Open => Style::new(),
            Status::Done => Style::new().fg(Color::Green),
            Status::Drop => Style::new().dim(),
        };
        left.push(Span::styled(format!("[{}] ", s.checkbox()), style));
    }
    if let Some(p) = m.priority {
        left.push(Span::styled(
            format!("{} ", "!".repeat(p as usize)),
            Style::new().fg(Color::Red).bold(),
        ));
    }

    let date = match item.kind() {
        Kind::Todo if closed => m.closed_on().map(|d| (show(d, false), Style::new().dim())),
        Kind::Todo => match (m.due, m.scheduled) {
            (Some(d), _) if is_overdue(item, app) => {
                Some((show(d, true), Style::new().fg(Color::Red)))
            }
            (Some(d), _) => Some((show(d, false), Style::new())),
            (None, Some(d)) if d > app.clock.today => Some((
                format!("from {}", show(d, false)),
                Style::new().fg(Color::Blue),
            )),
            _ => None,
        },
        Kind::Note => Some((show(m.date, false), Style::new().dim())),
    };
    let tags = m
        .tags
        .iter()
        .map(|t| format!("#{t}"))
        .collect::<Vec<_>>()
        .join(" ");

    let left_w: usize = left.iter().map(Span::width).sum();
    let date_w = date.as_ref().map_or(0, |(d, _)| d.width() + 2);
    let mut tags_w = if tags.is_empty() { 0 } else { tags.width() + 2 };
    const MIN_TITLE: usize = 12;
    if left_w + MIN_TITLE + date_w + tags_w + 1 > width {
        tags_w = 0;
    }
    let title_w = width.saturating_sub(left_w + date_w + tags_w + 1);
    let title = truncate(item.title(), title_w);
    let title_style = if closed {
        Style::new().dim().add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::new()
    };
    let gap = title_w.saturating_sub(title.width());
    left.push(Span::styled(title, title_style));
    left.push(" ".repeat(gap).into());
    if tags_w > 0 {
        left.push(Span::styled(
            format!("  {tags}"),
            Style::new().fg(Color::Cyan),
        ));
    }
    if let Some((d, style)) = date {
        left.push(Span::styled(format!("  {d}"), style));
    }
    Line::from(left)
}

fn draw_preview(f: &mut Frame, ui: &mut Ui, area: Rect) {
    let active = ui.focus == Focus::Preview;
    let block = panel("[2] Preview", active);
    let Some(item) = ui.selected() else {
        f.render_widget(block, area);
        return;
    };
    let mut lines = vec![
        Line::from(item.title().to_string()).bold(),
        Line::from(summary(item, &ui.app)).fg(Color::Yellow),
    ];
    if item.kind() == Kind::Todo && !item.meta.status.is_empty() {
        let history = item
            .meta
            .status
            .iter()
            .map(|(d, s)| format!("{} {}", s.as_str(), d.format(&ui.app.cfg.date_format)))
            .collect::<Vec<_>>()
            .join(" → ");
        lines.push(Line::from(history).dim());
    }
    let path = item.path.strip_prefix(&ui.app.root).unwrap_or(&item.path);
    lines.push(Line::from(path.display().to_string()).dim());
    lines.push(Line::default());
    let mut fenced = false;
    for l in item.content().lines() {
        if l.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let line = Line::from(l.to_string());
        lines.push(if fenced || l.trim_start().starts_with("```") {
            line.fg(Color::Green)
        } else if l.starts_with('#') {
            line.bold().fg(Color::Magenta)
        } else if l.trim_start().starts_with("- [x]") {
            line.dim()
        } else if l.starts_with('>') {
            line.italic().dim()
        } else {
            line
        });
    }
    let p = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((ui.preview_scroll, 0))
        .block(block);
    f.render_widget(p, area);
}

/// `key: what` for each bound action, separated by ` · `, with a space at each end.
fn hints(ui: &Ui, actions: &[(Action, &str)]) -> String {
    let keys = &ui.app.cfg.ui_keys;
    let parts: Vec<String> = actions
        .iter()
        .filter_map(|(a, what)| keys.first(*a).map(|k| format!("{k}: {what}")))
        .collect();
    format!(" {} ", parts.join(" · "))
}

fn draw_bar(f: &mut Frame, ui: &Ui, area: Rect) {
    if let Some((m, err)) = &ui.message {
        let style = if *err {
            Style::new().fg(Color::Red).bold()
        } else {
            Style::new().fg(Color::Green)
        };
        f.render_widget(
            Paragraph::new(m.as_str()).style(style).right_aligned(),
            area,
        );
    }
}

fn help_text(ui: &Ui) -> Text<'static> {
    let keys = &ui.app.cfg.ui_keys;
    let mut lines = vec![];
    for section in Section::ALL {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::from(section.name()).bold().fg(Color::Magenta));
        for s in ACTIONS.iter().filter(|s| s.section == section) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<16}", keys.all(s.action)),
                    Style::new().fg(Color::Yellow),
                ),
                s.help.into(),
                format!("  ({})", s.name).dim(),
            ]));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from("ctrl-c always quits.").dim());
    lines.push(
        Line::from("Change the keys with 'ui_keys' in the config, by the names in parentheses.")
            .dim(),
    );
    lines.push(Line::from("Dates take the same forms as on the command line.").dim());
    lines.push(Line::default());
    lines.push(Line::from("Commands").bold().fg(Color::Magenta));
    for (example, meaning) in [
        (
            ":n -t work call bob",
            "create a note, as 'doen n -t work call bob'",
        ),
        (":today", "the agenda of 'doen today'"),
        (
            ":closed 1w",
            "the todos closed in the last 7 days, newest first",
        ),
        (
            ":--date yesterday",
            "list as 'doen ls --date yesterday'; esc returns to the todos",
        ),
        (
            ":due fri",
            "without IDs, a command acts on the selected item",
        ),
    ] {
        lines.push(Line::from(vec![
            Span::styled(format!("  {example:<22}"), Style::new().fg(Color::Yellow)),
            meaning.into(),
        ]));
    }
    Text::from(lines)
}
