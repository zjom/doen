//! A one-line text field with readline-style editing keys.

use std::ops::Range;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub struct Input {
    text: String,
    /// A byte index on a char boundary.
    cursor: usize,
    /// The last killed text, for `ctrl-y`.
    killed: String,
}

impl Input {
    pub fn new(text: &str) -> Input {
        Input {
            text: text.to_string(),
            cursor: text.len(),
            killed: String::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Replaces the text from `start` to the cursor, and puts the cursor after the new text.
    pub fn replace(&mut self, start: usize, s: &str) {
        self.text.replace_range(start..self.cursor, s);
        self.cursor = start + s.len();
    }

    /// The display width of the text before the cursor.
    pub fn cursor_x(&self) -> u16 {
        self.text[..self.cursor].width() as u16
    }

    /// The start of the char before byte `i`, or 0.
    fn prev_of(&self, i: usize) -> usize {
        self.text[..i].char_indices().next_back().map_or(0, |(j, _)| j)
    }

    /// The end of the char at byte `i`, or `i` at the end.
    fn next_of(&self, i: usize) -> usize {
        self.text[i..].chars().next().map_or(i, |c| i + c.len_utf8())
    }

    /// The start of the whitespace-separated word before the cursor.
    fn word_start(&self) -> usize {
        let before = self.text[..self.cursor].trim_end();
        before.rfind(char::is_whitespace).map_or(0, |i| {
            i + before[i..].chars().next().map_or(1, char::len_utf8)
        })
    }

    /// The start of the alphanumeric word before the cursor.
    fn word_left(&self) -> usize {
        let mut start = self.cursor;
        let mut seen = false;
        for (i, c) in self.text[..self.cursor].char_indices().rev() {
            if c.is_alphanumeric() {
                seen = true;
            } else if seen {
                break;
            }
            start = i;
        }
        start
    }

    /// The end of the alphanumeric word after the cursor.
    fn word_right(&self) -> usize {
        let mut seen = false;
        for (i, c) in self.text[self.cursor..].char_indices() {
            if c.is_alphanumeric() {
                seen = true;
            } else if seen {
                return self.cursor + i;
            }
        }
        self.text.len()
    }

    /// Removes `range` into the kill buffer and puts the cursor at its start.
    fn kill(&mut self, range: Range<usize>) {
        if !range.is_empty() {
            self.killed = self.text.drain(range.clone()).collect();
        }
        self.cursor = range.start;
    }

    fn insert(&mut self, s: &str) {
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    /// Swaps the chars around the cursor, or the last two at the end, and moves past them.
    fn transpose(&mut self) {
        let mid = if self.cursor == self.text.len() {
            self.prev_of(self.cursor)
        } else {
            self.cursor
        };
        let start = self.prev_of(mid);
        let end = self.next_of(mid);
        if start == mid || mid == end {
            return;
        }
        let swapped = format!("{}{}", &self.text[mid..end], &self.text[start..mid]);
        self.text.replace_range(start..end, &swapped);
        self.cursor = end;
    }

    /// Applies an editing key. Returns `false` if the key is not an editing key.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let (start, end) = (self.cursor, self.text.len());
        match key.code {
            KeyCode::Char(c) if ctrl => match c {
                'a' => self.cursor = 0,
                'e' => self.cursor = end,
                'b' => self.cursor = self.prev_of(start),
                'f' => self.cursor = self.next_of(start),
                'h' => self.kill_char(self.prev_of(start)..start),
                'd' => self.kill_char(start..self.next_of(start)),
                'u' => self.kill(0..start),
                'k' => self.kill(start..end),
                'w' => self.kill(self.word_start()..start),
                'y' => self.insert(&self.killed.clone()),
                't' => self.transpose(),
                _ => return false,
            },
            KeyCode::Char(c) if alt => match c {
                'b' => self.cursor = self.word_left(),
                'f' => self.cursor = self.word_right(),
                'd' => self.kill(start..self.word_right()),
                _ => return false,
            },
            KeyCode::Char(c) => {
                let mut buf = [0; 4];
                self.insert(c.encode_utf8(&mut buf));
            }
            KeyCode::Left if ctrl || alt => self.cursor = self.word_left(),
            KeyCode::Right if ctrl || alt => self.cursor = self.word_right(),
            KeyCode::Backspace if ctrl || alt => self.kill(self.word_left()..start),
            KeyCode::Delete if ctrl || alt => self.kill(start..self.word_right()),
            KeyCode::Left => self.cursor = self.prev_of(start),
            KeyCode::Right => self.cursor = self.next_of(start),
            KeyCode::Backspace => self.kill_char(self.prev_of(start)..start),
            KeyCode::Delete => self.kill_char(start..self.next_of(start)),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = end,
            _ => return false,
        }
        true
    }

    /// Deletes a single char without touching the kill buffer.
    fn kill_char(&mut self, range: Range<usize>) {
        self.text.drain(range.clone());
        self.cursor = range.start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn alt(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
    }

    fn with(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    #[test]
    fn editing() {
        let mut i = Input::new("café");
        i.handle(key(KeyCode::Backspace));
        i.handle(key(KeyCode::Left));
        i.handle(key(KeyCode::Char('ß')));
        assert_eq!(i.text(), "caßf");
        assert_eq!(i.cursor_x(), 3);
        let mut i = Input::new("call the bank ");
        i.handle(ctrl('w'));
        assert_eq!(i.text(), "call the ");
    }

    #[test]
    fn word_motion() {
        let mut i = Input::new("done tag:foo bar");
        let left = with(KeyCode::Left, KeyModifiers::CONTROL);
        let mut stops = vec![];
        for _ in 0..5 {
            i.handle(left);
            stops.push(i.cursor());
        }
        assert_eq!(stops, [13, 9, 5, 0, 0]);
        let mut stops = vec![];
        for _ in 0..5 {
            i.handle(alt('f'));
            stops.push(i.cursor());
        }
        assert_eq!(stops, [4, 8, 12, 16, 16]);
        let mut i = Input::new("café ok");
        i.handle(alt('b'));
        i.handle(alt('b'));
        assert_eq!(i.cursor(), 0);
        i.handle(with(KeyCode::Right, KeyModifiers::ALT));
        assert_eq!(&i.text()[..i.cursor()], "café");
    }

    #[test]
    fn word_kills() {
        let mut i = Input::new("done tag:foo bar");
        i.handle(alt('b'));
        i.handle(with(KeyCode::Backspace, KeyModifiers::ALT));
        assert_eq!(i.text(), "done tag:bar");
        i.handle(alt('d'));
        assert_eq!(i.text(), "done tag:");
        i.handle(ctrl('w'));
        assert_eq!(i.text(), "done ");
        i.handle(ctrl('y'));
        assert_eq!(i.text(), "done tag:");
    }

    #[test]
    fn kill_and_yank() {
        let mut i = Input::new("hello world");
        i.handle(ctrl('a'));
        i.handle(alt('f'));
        i.handle(ctrl('k'));
        assert_eq!(i.text(), "hello");
        i.handle(ctrl('a'));
        i.handle(ctrl('y'));
        assert_eq!(i.text(), " worldhello");
        i.handle(ctrl('u'));
        assert_eq!(i.text(), "hello");
        i.handle(ctrl('e'));
        i.handle(ctrl('y'));
        assert_eq!(i.text(), "hello world");
    }

    #[test]
    fn chars() {
        let mut i = Input::new("abcd");
        i.handle(ctrl('b'));
        i.handle(ctrl('b'));
        i.handle(ctrl('d'));
        assert_eq!(i.text(), "abd");
        i.handle(ctrl('h'));
        assert_eq!(i.text(), "ad");
        i.handle(ctrl('f'));
        assert_eq!(i.cursor(), 2);
        // Single-char deletes don't replace the kill buffer.
        i.handle(ctrl('y'));
        assert_eq!(i.text(), "ad");
    }

    #[test]
    fn transpose() {
        let mut i = Input::new("abcé");
        i.handle(ctrl('t'));
        assert_eq!(i.text(), "abéc");
        i.handle(ctrl('a'));
        i.handle(ctrl('t'));
        assert_eq!(i.text(), "abéc");
        i.handle(ctrl('f'));
        i.handle(ctrl('t'));
        assert_eq!(i.text(), "baéc");
        assert_eq!(i.cursor(), 2);
        let mut i = Input::new("a");
        i.handle(ctrl('t'));
        assert_eq!(i.text(), "a");
    }

    #[test]
    fn other_chords_ignored() {
        let mut i = Input::new("ab");
        assert!(!i.handle(alt('x')));
        assert!(!i.handle(ctrl('x')));
        assert_eq!(i.text(), "ab");
    }
}
