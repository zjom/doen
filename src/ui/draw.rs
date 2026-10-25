//! Rendering of the interactive mode.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::keys::{ACTIONS, Action, Section, spec_of};
use super::{Focus, Popup, Row, Ui, VIEWS, View};
use crate::dates::display;
use crate::item::{Item, Kind, Status};
use crate::output::{is_overdue, summary};

/// Below this width, the preview is not shown.
const PREVIEW_MIN_WIDTH: u16 = 90;

pub fn draw(f: &mut Frame, ui: &mut Ui) {
    let [main, bar] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(f.area());
    let side_w = if ui.side_open() {
        22.min(main.width / 4)
    } else {
        0
    };
    let [side, rest] =
        Layout::horizontal([Constraint::Length(side_w), Constraint::Fill(1)]).areas(main);
    let [items, preview] = if main.width >= PREVIEW_MIN_WIDTH {
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(rest)
    } else {
        [rest, Rect::default()]
    };
    let views_h = VIEWS.len() as u16 + 2;
    let [views, tags] =
        Layout::vertical([Constraint::Length(views_h), Constraint::Fill(1)]).areas(side);

    if ui.side_open() {
        draw_views(f, ui, views);
        draw_tags(f, ui, tags);
    }
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

/// A line with `left` at the start and `right` at the end of `width` columns.
fn spread<'a>(left: Span<'a>, right: Span<'a>, width: u16) -> Line<'a> {
    let gap = (width as usize).saturating_sub(left.width() + right.width());
    Line::from(vec![left, " ".repeat(gap.max(1)).into(), right])
}

fn draw_views(f: &mut Frame, ui: &mut Ui, area: Rect) {
    let active = ui.focus == Focus::Views;
    let w = area.width.saturating_sub(2);
    let lines: Vec<ListItem> = VIEWS
        .iter()
        .zip(&ui.view_counts)
        .map(|(v, n)| spread(format!(" {}", v.name()).into(), format!("{n} ").dim(), w).into())
        .collect();
    let list = List::new(lines)
        .block(panel("[1] Views", active))
        .highlight_style(highlight(active));
    f.render_stateful_widget(list, area, &mut ui.views);
}

fn draw_tags(f: &mut Frame, ui: &mut Ui, area: Rect) {
    let active = ui.focus == Focus::Tags;
    let w = area.width.saturating_sub(2);
    let lines: Vec<ListItem> = ui
        .tags
        .iter()
        .map(|(t, n)| {
            let name = match t {
                Some(t) => Span::styled(format!(" #{t}"), Style::new().fg(Color::Cyan)),
                None => " all".into(),
            };
            spread(name, format!("{n} ").dim(), w).into()
        })
        .collect();
    let list = List::new(lines)
        .block(panel("[2] Tags", active))
        .highlight_style(highlight(active));
    f.render_stateful_widget(list, area, &mut ui.tag_state);
}

fn draw_items(f: &mut Frame, ui: &mut Ui, area: Rect) {
    let active = ui.focus == Focus::Items;
    let mut title = format!("[3] {}", ui.view().name());
    if let Some(t) = ui.tag() {
        title.push_str(&format!(" #{t}"));
    }
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
    let block = panel("[0] Preview", active);
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

/// `key: short` for the actions, from the short descriptions of the actions.
fn short_hints(ui: &Ui, actions: &[Action]) -> String {
    let pairs: Vec<(Action, &str)> = actions.iter().map(|a| (*a, spec_of(*a).short)).collect();
    hints(ui, &pairs)
}

fn draw_bar(f: &mut Frame, ui: &Ui, area: Rect) {
    use Action as A;
    let hints = if ui.searching {
        hints(
            ui,
            &[
                (A::Submit, "keep filter"),
                (A::Back, "clear"),
                (A::Down, "down"),
                (A::Up, "up"),
            ],
        )
    } else {
        match ui.focus {
            Focus::Views | Focus::Tags => short_hints(
                ui,
                &[
                    A::Down,
                    A::Up,
                    A::NextPanel,
                    A::Submit,
                    A::TogglePanels,
                    A::NewNote,
                    A::NewTodo,
                    A::Help,
                    A::Quit,
                ],
            ),
            Focus::Preview => short_hints(ui, &[A::Down, A::Up, A::Back, A::Help, A::Quit]),
            Focus::Items if ui.view() == View::Trash => short_hints(
                ui,
                &[
                    A::Restore,
                    A::Delete,
                    A::EmptyTrash,
                    A::TogglePanels,
                    A::Help,
                    A::Quit,
                ],
            ),
            Focus::Items => short_hints(
                ui,
                &[
                    A::TogglePanels,
                    A::ToggleDone,
                    A::ToggleDrop,
                    A::Edit,
                    A::Rename,
                    A::Schedule,
                    A::Due,
                    A::PriorityUp,
                    A::PriorityDown,
                    A::Tags,
                    A::Archive,
                    A::Delete,
                    A::NewNote,
                    A::NewTodo,
                    A::Search,
                    A::Help,
                ],
            ),
        }
    };
    let msg = ui.message.as_ref().map(|(m, err)| {
        let style = if *err {
            Style::new().fg(Color::Red).bold()
        } else {
            Style::new().fg(Color::Green)
        };
        Span::styled(format!("{m} "), style)
    });
    let msg_w = msg.as_ref().map_or(0, |m| m.width() as u16).min(area.width);
    let [left, right] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(msg_w)]).areas(area);
    f.render_widget(Paragraph::new(hints).fg(Color::Blue), left);
    if let Some(m) = msg {
        f.render_widget(Paragraph::new(m), right);
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
    Text::from(lines)
}
