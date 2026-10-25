//! The completion menu of the command line, with the candidates of `crate::complete`.

use crate::app::App;
use crate::complete::{Candidate, Context, candidates};
use crate::store::Store;

use super::command::split;
use super::input::Input;

/// The candidates for the word before the cursor.
pub struct Menu {
    /// The byte index in the line where the word starts.
    pub start: usize,
    pub candidates: Vec<Candidate>,
    /// The candidate that is in the line, if one is.
    pub selected: Option<usize>,
}

impl Menu {
    /// The menu for the word before the cursor, or `None` if nothing completes it.
    pub fn new(input: &Input, store: &Store, app: &App) -> Option<Menu> {
        let before = &input.text()[..input.cursor()];
        let start = before
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        let word = &before[start..];
        if word.contains(['\'', '"', '\\']) {
            return None;
        }
        let words = split(&before[..start]).ok()?;
        let candidates = candidates(Context::Ui, &words, word, store, app);
        (!candidates.is_empty()).then_some(Menu {
            start,
            candidates,
            selected: None,
        })
    }

    /// Puts the next candidate, or the previous one if `back`, in the line. An only candidate is
    /// accepted with a space after it, which ends the menu: the result is `None` then.
    pub fn cycle(mut self, input: &mut Input, back: bool) -> Option<Menu> {
        let n = self.candidates.len();
        if n == 1 {
            input.replace(self.start, &format!("{} ", self.candidates[0].value));
            return None;
        }
        let i = match (self.selected, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        self.selected = Some(i);
        input.replace(self.start, &self.candidates[i].value);
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(line: &str) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path(), &[], false).unwrap();
        let app = crate::testing::app(dir.path());
        Menu::new(&Input::new(line), &store, &app).map_or(vec![], |m| {
            m.candidates.into_iter().map(|c| c.value).collect()
        })
    }

    #[test]
    fn completing() {
        assert_eq!(values("to"), ["todo", "today"]);
        assert_eq!(values("cl"), ["closed"]);
        assert_eq!(values("u"), ["unarchive"]);
        assert_eq!(values("ls --so"), ["--sort"]);
        assert_eq!(values("ls --sort ti"), ["title"]);
        assert_eq!(values("tag "), ["add", "rm"]);
        assert_eq!(values("due tom"), ["tomorrow"]);
        assert_eq!(values("done --on y"), ["yesterday"]);
        assert_eq!(values("closed L"), ["LW", "LM", "LY"]);
        assert_eq!(values("ls t"), ["todo", "today", "tomorrow", "tue", "thu"]);
        assert_eq!(values("n -T no"), ["none"]);
        assert_eq!(values("priority "), ["1", "2", "3", "none"]);
        assert!(values("n buy mi").is_empty());
        assert!(values("n 'buy mi").is_empty());
    }
}
