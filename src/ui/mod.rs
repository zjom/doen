//! `doen ui`: an interactive mode in the style of lazygit. Panels on the left select a view and
//! a tag, the item list is in the middle and the preview of the selected item is on the right.

mod draw;
mod input;
pub mod keys;

use std::collections::HashMap;
use std::io::stdout;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::widgets::ListState;

use crate::app::App;
use crate::cli::args::CreateCmd;
use crate::commands::{create, edit, list, meta, status};
use crate::dates::{format_iso, parse_date};
use crate::error::{Error, Result, general, usage};
use crate::filter::{SortField, select};
use crate::ids;
use crate::item::{Item, Kind, Place, Status};
use crate::store::{self, Change, Store};
use crate::tags;
use crate::term;
use input::Input;
use keys::Action;

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Today,
    Todos,
    Notes,
    Closed,
    Archive,
    Trash,
}

pub const VIEWS: [View; 6] = [
    View::Today,
    View::Todos,
    View::Notes,
    View::Closed,
    View::Archive,
    View::Trash,
];

impl View {
    pub fn name(self) -> &'static str {
        match self {
            View::Today => "today",
            View::Todos => "todos",
            View::Notes => "notes",
            View::Closed => "closed",
            View::Archive => "archive",
            View::Trash => "trash",
        }
    }

    /// The groups of the view, sorted.
    fn groups<'a>(self, items: Vec<&'a Item>, app: &App) -> Vec<(&'static str, Vec<&'a Item>)> {
        let sort = app.cfg.default_sort;
        let closed = |i: &&Item| i.meta.current_status() != Some(Status::Open);
        let at = |places: &'static [Place]| move |i: &&Item| places.contains(&i.place);
        let items = items.into_iter();
        match self {
            View::Today => list::agenda(
                items.filter(at(&[Place::Root])),
                app.clock.today,
                sort,
                false,
            ),
            View::Todos => {
                let todos = items.filter(at(&[Place::Root])).filter(|i| !closed(i));
                vec![("TODOS", select(todos, sort, false).0)]
            }
            View::Notes => vec![(
                "NOTES",
                select(items.filter(at(&[Place::Root])), sort, false).1,
            )],
            View::Closed => {
                let closed = items
                    .filter(at(&[Place::Root, Place::Archive]))
                    .filter(closed);
                vec![("CLOSED", select(closed, Some(SortField::Closed), true).0)]
            }
            View::Archive | View::Trash => {
                let place = if self == View::Archive {
                    Place::Archive
                } else {
                    Place::Trash
                };
                let (todos, notes) = select(items.filter(|i| i.place == place), sort, false);
                vec![("TODOS", todos), ("NOTES", notes)]
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Views,
    Tags,
    Items,
    Preview,
}

/// A line of the item list.
pub enum Row {
    Header(&'static str, usize),
    /// An index into `Store::items`.
    Item(usize),
}

pub enum InputAction {
    Create(Kind),
    Title(String),
    Schedule(String),
    Due(String),
    Date(String),
    Tags(String),
}

pub enum ConfirmAction {
    Trash(String),
    Delete(String),
    EmptyTrash,
}

pub enum Popup {
    Input {
        title: String,
        input: Input,
        action: InputAction,
    },
    Confirm {
        title: String,
        message: String,
        action: ConfirmAction,
    },
    Help {
        scroll: u16,
    },
}

pub struct Ui {
    pub app: App,
    pub store: Store,
    pub focus: Focus,
    pub views: ListState,
    /// The item count of each view, without the tag filter and the search.
    pub view_counts: Vec<usize>,
    /// `None` is the entry for all tags.
    pub tags: Vec<(Option<String>, usize)>,
    pub tag_state: ListState,
    pub rows: Vec<Row>,
    /// The rows that are items, as indexes into `rows`.
    pub item_rows: Vec<usize>,
    pub cursor: usize,
    pub list_state: ListState,
    pub search: Input,
    pub searching: bool,
    pub preview_scroll: u16,
    pub popup: Option<Popup>,
    /// A message for the status bar, and whether it is an error.
    pub message: Option<(String, bool)>,
    /// Set when a command may have written to stderr, which ratatui does not know about.
    pub redraw: bool,
    quit: bool,
}

pub fn run(app: App) -> Result<()> {
    if !term::stdout_is_terminal() {
        return Err(general("the interactive mode requires a terminal"));
    }
    // Purges old trash before the screen is taken, so that its messages stay readable.
    app.begin()?.finish(&app)?;
    let store = Store::open(&app.root, &Place::ALL, false)?;
    let mut ui = Ui {
        app,
        store,
        focus: Focus::Items,
        views: ListState::default().with_selected(Some(1)),
        view_counts: vec![],
        tags: vec![],
        tag_state: ListState::default().with_selected(Some(0)),
        rows: vec![],
        item_rows: vec![],
        cursor: 0,
        list_state: ListState::default(),
        search: Input::default(),
        searching: false,
        preview_scroll: 0,
        popup: None,
        message: None,
        redraw: false,
        quit: false,
    };
    ui.rebuild(None);
    let mut terminal = ratatui::init();
    let result = ui.event_loop(&mut terminal);
    ratatui::restore();
    result
}

fn suspend() -> Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn resume(terminal: &mut DefaultTerminal) -> Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}

impl Ui {
    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.quit {
            if std::mem::take(&mut self.redraw) {
                terminal.clear()?;
            }
            terminal.draw(|f| draw::draw(f, self))?;
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if let Some(id) = self.key(key) {
                        self.edit(terminal, &id)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn view(&self) -> View {
        VIEWS[self.views.selected().unwrap_or(0)]
    }

    pub fn tag(&self) -> Option<&str> {
        self.tag_state
            .selected()
            .and_then(|n| self.tags.get(n))
            .and_then(|(t, _)| t.as_deref())
    }

    pub fn selected(&self) -> Option<&Item> {
        match self.rows.get(*self.item_rows.get(self.cursor)?)? {
            Row::Item(n) => Some(&self.store.items[*n]),
            Row::Header(..) => None,
        }
    }

    fn selected_id(&self) -> Option<String> {
        self.selected().map(|i| i.id().to_string())
    }

    /// Rebuilds the counts, the tag list and the rows. Selects the item `keep` if it is listed.
    fn rebuild(&mut self, keep: Option<String>) {
        let keep = keep.or_else(|| self.selected_id());
        let keep_tag = self.tag().map(String::from);
        let all: Vec<&Item> = self.store.items.iter().collect();
        self.view_counts = VIEWS
            .iter()
            .map(|v| {
                let groups = v.groups(all.clone(), &self.app);
                groups.iter().map(|(_, g)| g.len()).sum()
            })
            .collect();

        let needle = self.search.text().to_lowercase();
        let found: Vec<&Item> = all
            .into_iter()
            .filter(|i| needle.is_empty() || i.body.to_lowercase().contains(&needle))
            .collect();
        let groups = self.view().groups(found, &self.app);

        let mut counts: Vec<(String, usize)> = vec![];
        for t in groups
            .iter()
            .flat_map(|(_, g)| g)
            .flat_map(|i| &i.meta.tags)
        {
            match counts.iter_mut().find(|(c, _)| c == t) {
                Some((_, n)) => *n += 1,
                None => counts.push((t.clone(), 1)),
            }
        }
        counts.sort();
        let total = groups.iter().map(|(_, g)| g.len()).sum();
        self.tags = std::iter::once((None, total))
            .chain(counts.into_iter().map(|(t, n)| (Some(t), n)))
            .collect();
        let tag_n = keep_tag
            .and_then(|k| self.tags.iter().position(|(t, _)| t.as_deref() == Some(&k)))
            .unwrap_or(0);
        self.tag_state.select(Some(tag_n));

        let tag = self.tag().map(String::from);
        let index: HashMap<*const Item, usize> = (self.store.items.iter())
            .enumerate()
            .map(|(n, i)| (i as *const Item, n))
            .collect();
        let mut rows = vec![];
        for (header, items) in groups {
            let items: Vec<&Item> = items
                .into_iter()
                .filter(|i| {
                    tag.as_ref()
                        .is_none_or(|f| i.meta.tags.iter().any(|t| tags::matches(t, f)))
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            rows.push(Row::Header(header, items.len()));
            rows.extend(
                items
                    .into_iter()
                    .map(|i| Row::Item(index[&(i as *const Item)])),
            );
        }
        self.item_rows = (0..rows.len())
            .filter(|&n| matches!(rows[n], Row::Item(_)))
            .collect();
        self.rows = rows;
        let found = keep.and_then(|id| {
            self.item_rows.iter().position(|&r| match self.rows[r] {
                Row::Item(n) => self.store.items[n].id() == id,
                Row::Header(..) => false,
            })
        });
        let cursor = found.unwrap_or(self.cursor);
        self.set_cursor(cursor);
    }

    fn set_cursor(&mut self, n: usize) {
        let n = n.min(self.item_rows.len().saturating_sub(1));
        if n != self.cursor {
            self.preview_scroll = 0;
        }
        self.cursor = n;
        self.list_state.select(self.item_rows.get(n).copied());
        // The header above the first item stays visible.
        if n == 0 {
            *self.list_state.offset_mut() = 0;
        }
    }

    fn reload(&mut self, keep: Option<String>) {
        match Store::open(&self.app.root, &Place::ALL, false) {
            Ok(s) => self.store = s,
            Err(e) => self.error(e),
        }
        self.rebuild(keep);
    }

    fn info(&mut self, msg: impl Into<String>) {
        self.message = Some((msg.into(), false));
    }

    fn error(&mut self, e: Error) {
        self.message = Some((e.to_string(), true));
    }

    fn report(&mut self, r: Result<String>) {
        match r {
            Ok(msg) => self.info(msg),
            Err(e) => self.error(e),
        }
    }

    fn is(&self, a: Action, key: &KeyEvent) -> bool {
        self.app.cfg.ui_keys.is(a, key)
    }

    /// The first key of an action, for messages.
    fn key_name(&self, a: Action) -> String {
        self.app
            .cfg
            .ui_keys
            .first(a)
            .unwrap_or_else(|| "(none)".into())
    }

    /// Handles a key. Returns the ID of an item to open in the editor.
    fn key(&mut self, key: KeyEvent) -> Option<String> {
        self.message = None;
        // Always quits, whatever the keymap.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return None;
        }
        if let Some(popup) = self.popup.take() {
            self.popup_key(popup, key);
            return None;
        }
        if self.searching {
            self.search_key(key);
            return None;
        }
        let side = self.side_open();
        let is = |a| self.is(a, &key);
        if is(Action::Quit) {
            self.quit = true;
        } else if is(Action::Help) {
            self.popup = Some(Popup::Help { scroll: 0 });
        } else if is(Action::FocusViews) {
            self.focus = Focus::Views;
        } else if is(Action::FocusTags) {
            self.focus = Focus::Tags;
        } else if is(Action::FocusItems) {
            self.focus = Focus::Items;
        } else if is(Action::FocusPreview) {
            self.focus = Focus::Preview;
        } else if is(Action::TogglePanels) {
            self.focus = if side { Focus::Items } else { Focus::Views };
        } else if is(Action::NextPanel) {
            self.cycle_focus();
        } else if is(Action::FocusRight) {
            self.focus = if side { Focus::Items } else { Focus::Preview };
        } else if is(Action::FocusLeft)
            || is(Action::Back) && (side || self.focus == Focus::Preview)
        {
            self.focus = Focus::Items;
        } else if is(Action::Back) && !self.search.text().is_empty() {
            self.search = Input::default();
            self.rebuild(None);
        } else if is(Action::Reload) {
            self.reload(None);
            self.info("reloaded");
        } else if is(Action::Search) {
            self.focus = Focus::Items;
            self.searching = true;
        } else if is(Action::NewNote) {
            self.prompt("New note", "", InputAction::Create(Kind::Note));
        } else if is(Action::NewTodo) {
            self.prompt("New todo", "", InputAction::Create(Kind::Todo));
        } else {
            match self.focus {
                Focus::Views => self.views_key(key),
                Focus::Tags => self.tags_key(key),
                Focus::Items => return self.items_key(key),
                Focus::Preview => self.preview_key(key),
            }
        }
        None
    }

    /// Whether the views and tags panels are shown. They are shown only while one has the focus.
    pub fn side_open(&self) -> bool {
        matches!(self.focus, Focus::Views | Focus::Tags)
    }

    /// Switches between the two panels of the side column, or between the items and the preview.
    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Views => Focus::Tags,
            Focus::Tags => Focus::Views,
            Focus::Items => Focus::Preview,
            Focus::Preview => Focus::Items,
        };
    }

    /// The new position after a movement key in a list of `len` entries, or `None`.
    fn movement(&self, key: &KeyEvent, pos: usize, len: usize) -> Option<usize> {
        let last = len.saturating_sub(1);
        let page = 10;
        let is = |a| self.is(a, key);
        Some(if is(Action::Down) {
            (pos + 1).min(last)
        } else if is(Action::Up) {
            pos.saturating_sub(1)
        } else if is(Action::Top) {
            0
        } else if is(Action::Bottom) {
            last
        } else if is(Action::PageDown) {
            (pos + page).min(last)
        } else if is(Action::PageUp) {
            pos.saturating_sub(page)
        } else {
            return None;
        })
    }

    fn views_key(&mut self, key: KeyEvent) {
        let pos = self.views.selected().unwrap_or(0);
        if let Some(n) = self.movement(&key, pos, VIEWS.len()) {
            if n != pos {
                self.views.select(Some(n));
                self.cursor = 0;
                self.rebuild(None);
            }
        } else if self.is(Action::Submit, &key) {
            self.focus = Focus::Items;
        }
    }

    fn tags_key(&mut self, key: KeyEvent) {
        let pos = self.tag_state.selected().unwrap_or(0);
        if let Some(n) = self.movement(&key, pos, self.tags.len()) {
            if n != pos {
                self.tag_state.select(Some(n));
                self.cursor = 0;
                self.rebuild(None);
            }
        } else if self.is(Action::Submit, &key) {
            self.focus = Focus::Items;
        }
    }

    fn preview_key(&mut self, key: KeyEvent) {
        let is = |a| self.is(a, &key);
        if is(Action::Down) || is(Action::PreviewDown) {
            self.scroll_preview(1);
        } else if is(Action::Up) || is(Action::PreviewUp) {
            self.scroll_preview(-1);
        } else if is(Action::PageDown) || is(Action::PreviewPageDown) {
            self.scroll_preview(10);
        } else if is(Action::PageUp) || is(Action::PreviewPageUp) {
            self.scroll_preview(-10);
        } else if is(Action::Top) {
            self.preview_scroll = 0;
        }
    }

    fn scroll_preview(&mut self, by: i32) {
        self.preview_scroll = (self.preview_scroll as i32 + by).max(0) as u16;
    }

    /// Whether the key types a character into a text field. Such keys never run an action.
    fn types(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char(_))
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    }

    fn search_key(&mut self, key: KeyEvent) {
        if self.is(Action::Submit, &key) {
            self.searching = false;
        } else if self.is(Action::Back, &key) {
            self.searching = false;
            self.search = Input::default();
            self.rebuild(None);
        } else if let Some(n) = (!Self::types(&key))
            .then(|| self.movement(&key, self.cursor, self.item_rows.len()))
            .flatten()
        {
            self.set_cursor(n);
        } else if self.search.handle(key) {
            self.cursor = 0;
            self.rebuild(None);
        }
    }

    fn items_key(&mut self, key: KeyEvent) -> Option<String> {
        if let Some(n) = self.movement(&key, self.cursor, self.item_rows.len()) {
            self.set_cursor(n);
            return None;
        }
        let is = |a| self.is(a, &key);
        if is(Action::PreviewDown) {
            self.scroll_preview(1);
        } else if is(Action::PreviewUp) {
            self.scroll_preview(-1);
        } else if is(Action::PreviewPageDown) {
            self.scroll_preview(10);
        } else if is(Action::PreviewPageUp) {
            self.scroll_preview(-10);
        } else if is(Action::EmptyTrash) && self.view() == View::Trash {
            self.confirm(
                "Empty the trash",
                "Delete every item in the trash permanently?".into(),
                ConfirmAction::EmptyTrash,
            );
        } else {
            return self.item_action(key);
        }
        None
    }

    /// Runs an action on the selected item.
    fn item_action(&mut self, key: KeyEvent) -> Option<String> {
        let item = self.selected()?;
        let (id, title) = (item.id().to_string(), item.title().to_string());
        let trashed = item.place == Place::Trash;
        let action = keys::ACTIONS
            .iter()
            .map(|s| s.action)
            .filter(|a| keys::spec_of(*a).section == keys::Section::Items)
            .find(|a| self.is(*a, &key))?;
        match action {
            Action::Delete if trashed => self.confirm(
                "Delete",
                format!("Delete '{title}' permanently?"),
                ConfirmAction::Delete(id),
            ),
            Action::Delete => self.confirm(
                "Trash",
                format!("Move '{title}' to the trash?"),
                ConfirmAction::Trash(id),
            ),
            Action::Restore if trashed => {
                let r = self.mutate(&id, "restore", |_, i| {
                    i.set_date("trashed", None);
                    Ok(())
                });
                self.after(r, &id);
            }
            Action::Restore => self.error(usage("the item is not in the trash")),
            _ if trashed => {
                let msg = format!(
                    "the item is in the trash: restore it with {} or delete it with {}",
                    self.key_name(Action::Restore),
                    self.key_name(Action::Delete)
                );
                self.error(usage(msg));
            }
            Action::Edit => return Some(id),
            Action::ToggleDone => self.toggle_status(&id, Status::Done),
            Action::ToggleDrop => self.toggle_status(&id, Status::Drop),
            Action::Archive => {
                let (verb, set) = match self.selected()?.meta.archived {
                    Some(_) => ("unarchive", None),
                    None => ("archive", Some(self.app.clock.today)),
                };
                let r = self.mutate(&id, verb, |_, i| {
                    i.set_date("archived", set);
                    Ok(())
                });
                self.after(r, &id);
            }
            Action::PriorityUp => self.bump_priority(&id, 1),
            Action::PriorityDown => self.bump_priority(&id, -1),
            Action::Rename => self.prompt("Title", &title, InputAction::Title(id)),
            Action::Schedule => {
                let cur = self.selected()?.meta.scheduled.map(format_iso);
                self.date_prompt("Scheduled", cur, InputAction::Schedule(id));
            }
            Action::Due => {
                let cur = self.selected()?.meta.due.map(format_iso);
                self.date_prompt("Due", cur, InputAction::Due(id));
            }
            Action::Date => {
                let cur = format_iso(self.selected()?.meta.date);
                self.prompt("Date", &cur, InputAction::Date(id));
            }
            Action::Tags => {
                let cur = self.selected()?.meta.tags.join(", ");
                self.prompt("Tags (comma-separated)", &cur, InputAction::Tags(id));
            }
            _ => {}
        }
        None
    }

    fn prompt(&mut self, title: &str, value: &str, action: InputAction) {
        self.popup = Some(Popup::Input {
            title: title.into(),
            input: Input::new(value),
            action,
        });
    }

    fn date_prompt(&mut self, title: &str, value: Option<String>, action: InputAction) {
        if self.selected().is_some_and(|i| i.kind() == Kind::Note) {
            self.error(usage(format!(
                "only todos have a {} date",
                title.to_lowercase()
            )));
            return;
        }
        let title = format!("{title} (date, or empty to clear)");
        self.prompt(&title, &value.unwrap_or_default(), action);
    }

    fn confirm(&mut self, title: &str, message: String, action: ConfirmAction) {
        self.popup = Some(Popup::Confirm {
            title: title.into(),
            message,
            action,
        });
    }

    fn popup_key(&mut self, popup: Popup, key: KeyEvent) {
        match popup {
            Popup::Help { scroll } => {
                let scroll = if self.is(Action::Down, &key) {
                    scroll + 1
                } else if self.is(Action::Up, &key) {
                    scroll.saturating_sub(1)
                } else {
                    return;
                };
                self.popup = Some(Popup::Help { scroll });
            }
            Popup::Confirm { action, .. } if self.is(Action::Confirm, &key) => {
                self.run_confirm(action);
            }
            Popup::Confirm { .. } => {}
            Popup::Input { action, input, .. } if self.is(Action::Submit, &key) => {
                self.run_input(action, input.text().trim());
            }
            Popup::Input { .. } if self.is(Action::Back, &key) => {}
            Popup::Input {
                title,
                mut input,
                action,
            } => {
                input.handle(key);
                self.popup = Some(Popup::Input {
                    title,
                    input,
                    action,
                });
            }
        }
    }

    fn run_confirm(&mut self, action: ConfirmAction) {
        let today = self.app.clock.today;
        match action {
            ConfirmAction::Trash(id) => {
                let r = self.mutate(&id, "rm", |_, i| {
                    i.set_date("trashed", Some(today));
                    Ok(())
                });
                self.after(r, &id);
            }
            ConfirmAction::Delete(id) => {
                let r = self.delete(Some(&id));
                self.after(r, &id);
            }
            ConfirmAction::EmptyTrash => {
                let r = self.delete(None);
                self.after(r, "");
            }
        }
    }

    fn run_input(&mut self, action: InputAction, text: &str) {
        let id = match action {
            InputAction::Create(kind) => {
                let r = self.create(kind, text);
                let id = r.as_ref().map_or(String::new(), Clone::clone);
                return self.after(r.map(|id| format!("created  {id}")), &id);
            }
            InputAction::Title(ref id)
            | InputAction::Schedule(ref id)
            | InputAction::Due(ref id)
            | InputAction::Date(ref id)
            | InputAction::Tags(ref id) => id.clone(),
        };
        let clock = &self.app.clock;
        let date = |s: &str| match s {
            "" | "none" => Ok(None),
            s => parse_date(s, clock).map(Some).map_err(usage),
        };
        let r = match action {
            InputAction::Create(_) => unreachable!(),
            InputAction::Title(_) if text.is_empty() => Err(usage("the title cannot be empty")),
            InputAction::Title(_) => self.mutate(&id, "title", |_, i| {
                meta::retitle(i, text);
                Ok(())
            }),
            InputAction::Schedule(_) => date(text).and_then(|d| {
                self.mutate(&id, "scheduled", |_, i| {
                    i.set_date("scheduled", d);
                    Ok(())
                })
            }),
            InputAction::Due(_) => date(text).and_then(|d| {
                self.mutate(&id, "due", |_, i| {
                    i.set_date("due", d);
                    Ok(())
                })
            }),
            InputAction::Date(_) => parse_date(text, clock).map_err(usage).and_then(|d| {
                self.mutate(&id, "date", |_, i| {
                    i.set_date("date", Some(d));
                    Ok(())
                })
            }),
            InputAction::Tags(_) => tags::parse_list(text).map_err(usage).and_then(|t| {
                self.mutate(&id, "tag", |_, i| {
                    i.set_tags(&t);
                    Ok(())
                })
            }),
        };
        self.after(r, &id);
    }

    /// Reports the result of an action and reloads the store, keeping the item `id` selected.
    fn after(&mut self, r: Result<String>, id: &str) {
        self.redraw = true;
        self.reload((!id.is_empty()).then(|| id.to_string()));
        self.report(r);
    }

    /// Changes one item in a transaction and returns the output line of the change.
    fn mutate(
        &self,
        id: &str,
        verb: &str,
        f: impl FnOnce(&App, &mut Item) -> Result<()>,
    ) -> Result<String> {
        let mut tx = self.app.begin()?;
        let n = ids::resolve(&mut tx.store, id, true)?;
        f(&self.app, &mut tx.store.items[n])?;
        tx.save(n, verb)?;
        let line = tx
            .changes
            .shown
            .last()
            .map(Change::line)
            .unwrap_or_default();
        tx.commit(&self.app);
        Ok(line)
    }

    /// Deletes the item `id` permanently, or with `None`, every item in the trash.
    fn delete(&self, id: Option<&str>) -> Result<String> {
        let mut tx = self.app.begin_with(&[Place::Trash])?;
        let idx = match id {
            Some(id) => vec![ids::resolve(&mut tx.store, id, true)?],
            None => (0..tx.store.items.len())
                .filter(|&n| tx.store.items[n].place == Place::Trash)
                .collect(),
        };
        for &n in &idx {
            let item = &tx.store.items[n];
            store::delete(item, &mut tx.changes)?;
            tx.changes.shown.push(Change::new("deleted", item));
        }
        let msg = match tx.changes.shown.as_slice() {
            [] => "the trash is empty".to_string(),
            [c] => c.line(),
            all => format!("deleted {} items", all.len()),
        };
        tx.commit(&self.app);
        Ok(msg)
    }

    fn toggle_status(&mut self, id: &str, status: Status) {
        let today = self.app.clock.today;
        let r = self
            .selected()
            .filter(|i| i.kind() == Kind::Todo)
            .ok_or_else(|| usage("only todos have a status"))
            .and_then(|i| {
                let target = if i.meta.current_status() == Some(status) {
                    Status::Open
                } else {
                    status
                };
                let verb = match target {
                    Status::Open => "reopen",
                    s => s.as_str(),
                };
                self.mutate(id, verb, |app, i| {
                    status::apply(app, i, target, today);
                    Ok(())
                })
            });
        self.after(r, id);
    }

    fn bump_priority(&mut self, id: &str, by: i8) {
        let r = match self.selected() {
            Some(i) if i.kind() == Kind::Note => Err(usage("only todos have a priority")),
            _ => self.mutate(id, "priority", |_, i| {
                let p = (i.meta.priority.unwrap_or(0) as i8 + by).clamp(0, 3) as u8;
                i.set_priority((p > 0).then_some(p));
                Ok(())
            }),
        };
        self.after(r, id);
    }

    /// Creates an item with the selected tag. Returns the ID.
    fn create(&self, kind: Kind, title: &str) -> Result<String> {
        if title.is_empty() {
            return Err(usage("the title cannot be empty"));
        }
        let c = CreateCmd {
            title: vec![title.to_string()],
            content: None,
            tag: self.tag().map(String::from).into_iter().collect(),
            template: None,
            date: None,
            scheduled: None,
            due: None,
            priority: None,
            edit: false,
        };
        create::create(&self.app, kind, c)
    }

    /// Opens the item in the editor, with the terminal in its normal state.
    fn edit(&mut self, terminal: &mut DefaultTerminal, id: &str) -> Result<()> {
        suspend()?;
        let r = edit::edit_item(&self.app, id, true);
        resume(terminal)?;
        self.reload(Some(id.to_string()));
        match r {
            Ok(()) => {}
            Err(e) => self.error(e),
        }
        Ok(())
    }
}
