//! The keybindings of the interactive mode. The `ui_keys` config key replaces the keys of an
//! action.

use std::fmt;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    Help,
    TogglePanels,
    FocusViews,
    FocusTags,
    FocusItems,
    FocusPreview,
    NextPanel,
    FocusLeft,
    FocusRight,
    Back,
    Submit,
    Confirm,
    Search,
    Command,
    Reload,
    NewNote,
    NewTodo,
    Down,
    Up,
    Top,
    Bottom,
    PageDown,
    PageUp,
    Edit,
    ToggleDone,
    ToggleDrop,
    Rename,
    Schedule,
    Due,
    Date,
    PriorityUp,
    PriorityDown,
    Tags,
    Archive,
    Delete,
    Restore,
    EmptyTrash,
    PreviewDown,
    PreviewUp,
    PreviewPageDown,
    PreviewPageUp,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Section {
    Global,
    Movement,
    Items,
}

impl Section {
    pub const ALL: [Section; 3] = [Section::Global, Section::Movement, Section::Items];

    pub fn name(self) -> &'static str {
        match self {
            Section::Global => "Global",
            Section::Movement => "Movement",
            Section::Items => "Items",
        }
    }
}

pub struct Spec {
    pub action: Action,
    /// The name in the `ui_keys` config key.
    pub name: &'static str,
    pub keys: &'static [&'static str],
    pub section: Section,
    /// The description in the help.
    pub help: &'static str,
}

const fn spec(
    action: Action,
    name: &'static str,
    keys: &'static [&'static str],
    section: Section,
    help: &'static str,
) -> Spec {
    Spec {
        action,
        name,
        keys,
        section,
        help,
    }
}

use Action as A;
use Section::{Global as G, Items as I, Movement as M};

/// Every action, in the order of `Action`.
#[rustfmt::skip]
pub const ACTIONS: &[Spec] = &[
    spec(A::Quit, "quit", &["q"], G, "quit"),
    spec(A::Help, "help", &["?"], G, "show the keybindings"),
    spec(A::TogglePanels, "toggle_panels", &["v"], G, "show or hide the views and tags panels"),
    spec(A::FocusViews, "focus_views", &["1"], G, "show the panels, focus the views"),
    spec(A::FocusTags, "focus_tags", &["2"], G, "show the panels, focus the tags"),
    spec(A::FocusItems, "focus_items", &["3"], G, "focus the items"),
    spec(A::FocusPreview, "focus_preview", &["0"], G, "focus the preview"),
    spec(A::NextPanel, "next_panel", &["tab", "backtab"], G, "switch views ↔ tags, items ↔ preview"),
    spec(A::FocusLeft, "focus_left", &["h", "left"], G, "focus the items"),
    spec(A::FocusRight, "focus_right", &["l", "right"], G, "focus the preview; in the panels, close them"),
    spec(A::Back, "back", &["esc"], G, "close the panels, leave the preview, clear the search, cancel a prompt"),
    spec(A::Submit, "submit", &["enter"], G, "accept a prompt or the search; in the panels, close them"),
    spec(A::Confirm, "confirm", &["y", "enter"], G, "answer yes to a question"),
    spec(A::Search, "search", &["/"], G, "search the titles and bodies"),
    spec(A::Command, "command", &[":"], G, "run a doen command, as on the command line"),
    spec(A::Reload, "reload", &["ctrl-r"], G, "reload the store"),
    spec(A::NewNote, "new_note", &["n"], G, "new note, with the selected tag"),
    spec(A::NewTodo, "new_todo", &["t"], G, "new todo, with the selected tag"),
    spec(A::Down, "down", &["j", "down"], M, "move down"),
    spec(A::Up, "up", &["k", "up"], M, "move up"),
    spec(A::Top, "top", &["g", "home"], M, "go to the first entry"),
    spec(A::Bottom, "bottom", &["G", "end"], M, "go to the last entry"),
    spec(A::PageDown, "page_down", &["pagedown"], M, "move down a page"),
    spec(A::PageUp, "page_up", &["pageup"], M, "move up a page"),
    spec(A::Edit, "edit", &["e", "enter"], I, "open in the editor"),
    spec(A::ToggleDone, "toggle_done", &["space"], I, "toggle done"),
    spec(A::ToggleDrop, "toggle_drop", &["x"], I, "toggle dropped"),
    spec(A::Rename, "rename", &["r"], I, "change the title"),
    spec(A::Schedule, "schedule", &["s"], I, "set the scheduled date"),
    spec(A::Due, "due", &["u"], I, "set the due date"),
    spec(A::Date, "date", &["w"], I, "set the date"),
    spec(A::PriorityUp, "priority_up", &["+", "="], I, "raise the priority"),
    spec(A::PriorityDown, "priority_down", &["-"], I, "lower the priority"),
    spec(A::Tags, "tags", &["#"], I, "edit the tags"),
    spec(A::Archive, "archive", &["a"], I, "archive or unarchive"),
    spec(A::Delete, "delete", &["d"], I, "move to the trash; in the trash, delete permanently"),
    spec(A::Restore, "restore", &["R"], I, "restore from the trash"),
    spec(A::EmptyTrash, "empty_trash", &["E"], I, "empty the trash"),
    spec(A::PreviewDown, "preview_down", &["J"], I, "scroll the preview down"),
    spec(A::PreviewUp, "preview_up", &["K"], I, "scroll the preview up"),
    spec(A::PreviewPageDown, "preview_page_down", &["ctrl-d"], I, "scroll the preview down a page"),
    spec(A::PreviewPageUp, "preview_page_up", &["ctrl-u"], I, "scroll the preview up a page"),
];

pub fn spec_of(a: Action) -> &'static Spec {
    &ACTIONS[a as usize]
}

/// One key with its modifiers. Only Ctrl and Alt count: Shift is in the character.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    code: KeyCode,
    mods: KeyModifiers,
}

const NAMED: &[(&str, KeyCode)] = &[
    ("space", KeyCode::Char(' ')),
    ("enter", KeyCode::Enter),
    ("esc", KeyCode::Esc),
    ("tab", KeyCode::Tab),
    ("backtab", KeyCode::BackTab),
    ("backspace", KeyCode::Backspace),
    ("delete", KeyCode::Delete),
    ("insert", KeyCode::Insert),
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("left", KeyCode::Left),
    ("right", KeyCode::Right),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("pageup", KeyCode::PageUp),
    ("pagedown", KeyCode::PageDown),
];

impl Key {
    /// Parses `x`, `ctrl-x`, `alt-x`, `ctrl-alt-x`, a key name (`enter`, `pagedown`, …) or
    /// `f1` to `f12`.
    pub fn parse(s: &str) -> Result<Key, String> {
        let bad = || format!("invalid key '{s}'");
        let mut mods = KeyModifiers::NONE;
        let mut rest = s;
        loop {
            let (m, r) = if let Some(r) = rest.strip_prefix("ctrl-") {
                (KeyModifiers::CONTROL, r)
            } else if let Some(r) = rest.strip_prefix("alt-") {
                (KeyModifiers::ALT, r)
            } else {
                break;
            };
            if r.is_empty() {
                break;
            }
            mods |= m;
            rest = r;
        }
        let mut chars = rest.chars();
        let code = match (chars.next(), chars.next()) {
            (Some(c), None) if mods.contains(KeyModifiers::CONTROL) => {
                KeyCode::Char(c.to_ascii_lowercase())
            }
            (Some(c), None) => KeyCode::Char(c),
            _ if rest == "shift-tab" => KeyCode::BackTab,
            _ => match NAMED.iter().find(|(n, _)| *n == rest) {
                Some((_, c)) => *c,
                None => match rest.strip_prefix('f').and_then(|n| n.parse().ok()) {
                    Some(n @ 1..=12) => KeyCode::F(n),
                    _ => return Err(bad()),
                },
            },
        };
        Ok(Key { code, mods })
    }

    pub fn matches(&self, e: &KeyEvent) -> bool {
        let mods = e.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
        let code = match e.code {
            KeyCode::Char(c) if mods.contains(KeyModifiers::CONTROL) => {
                KeyCode::Char(c.to_ascii_lowercase())
            }
            KeyCode::Tab if e.modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
            c => c,
        };
        self.code == code && self.mods == mods
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mods.contains(KeyModifiers::CONTROL) {
            f.write_str("ctrl-")?;
        }
        if self.mods.contains(KeyModifiers::ALT) {
            f.write_str("alt-")?;
        }
        match self.code {
            KeyCode::F(n) => write!(f, "f{n}"),
            code => match NAMED.iter().find(|(_, c)| *c == code) {
                Some((n, _)) => f.write_str(n),
                None => match code {
                    KeyCode::Char(c) => write!(f, "{c}"),
                    _ => f.write_str("?"),
                },
            },
        }
    }
}

/// The keys of each action, indexed by `Action`.
#[derive(Clone, Debug)]
pub struct Keymap(Vec<Vec<Key>>);

impl Default for Keymap {
    fn default() -> Keymap {
        Keymap(
            ACTIONS
                .iter()
                .map(|s| {
                    let keys = s.keys.iter().map(|k| Key::parse(k));
                    keys.collect::<Result<_, _>>().expect("valid default keys")
                })
                .collect(),
        )
    }
}

impl Keymap {
    /// Replaces the keys of the action `name`. An empty list unbinds the action. Returns
    /// `Ok(false)` if there is no such action.
    pub fn set(&mut self, name: &str, keys: &[&str]) -> Result<bool, String> {
        let Some(n) = ACTIONS.iter().position(|s| s.name == name) else {
            return Ok(false);
        };
        self.0[n] = keys
            .iter()
            .map(|k| Key::parse(k))
            .collect::<Result<_, _>>()?;
        Ok(true)
    }

    pub fn is(&self, a: Action, e: &KeyEvent) -> bool {
        self.0[a as usize].iter().any(|k| k.matches(e))
    }

    /// The first key of the action, for the status bar.
    pub fn first(&self, a: Action) -> Option<String> {
        self.0[a as usize].first().map(Key::to_string)
    }

    /// Every key of the action, for the help.
    pub fn all(&self, a: Action) -> String {
        let keys: Vec<String> = self.0[a as usize].iter().map(Key::to_string).collect();
        if keys.is_empty() {
            "(none)".into()
        } else {
            keys.join(" ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_order() {
        for (n, s) in ACTIONS.iter().enumerate() {
            assert_eq!(s.action as usize, n, "{}", s.name);
        }
    }

    #[test]
    fn parse_and_match() {
        let ev = |c, m| KeyEvent::new(c, m);
        let k = Key::parse("ctrl-R").unwrap();
        assert!(k.matches(&ev(KeyCode::Char('r'), KeyModifiers::CONTROL)));
        assert_eq!(k.to_string(), "ctrl-r");
        let k = Key::parse("G").unwrap();
        assert!(k.matches(&ev(KeyCode::Char('G'), KeyModifiers::SHIFT)));
        assert!(!k.matches(&ev(KeyCode::Char('g'), KeyModifiers::NONE)));
        assert!(
            Key::parse("-")
                .unwrap()
                .matches(&ev(KeyCode::Char('-'), KeyModifiers::NONE))
        );
        assert!(Key::parse("ctrl--").is_ok());
        assert!(
            Key::parse("shift-tab")
                .unwrap()
                .matches(&ev(KeyCode::Tab, KeyModifiers::SHIFT))
        );
        assert_eq!(Key::parse("f5").unwrap().to_string(), "f5");
        assert!(Key::parse("hyper-x").is_err() && Key::parse("f13").is_err());
        let mut m = Keymap::default();
        assert_eq!(m.set("quit", &["Q", "ctrl-q"]), Ok(true));
        assert_eq!(m.set("nope", &[]), Ok(false));
        assert_eq!(m.all(Action::Quit), "Q ctrl-q");
        assert!(m.set("quit", &[]).unwrap() && m.first(Action::Quit).is_none());
    }
}
