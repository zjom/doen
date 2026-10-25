//! A one-line text field.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub struct Input {
    text: String,
    /// A byte index on a char boundary.
    cursor: usize,
}

impl Input {
    pub fn new(text: &str) -> Input {
        Input {
            text: text.to_string(),
            cursor: text.len(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The display width of the text before the cursor.
    pub fn cursor_x(&self) -> u16 {
        self.text[..self.cursor].width() as u16
    }

    fn prev(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |c| self.cursor + c.len_utf8())
    }

    /// The start of the word before the cursor.
    fn word_start(&self) -> usize {
        let before = self.text[..self.cursor].trim_end();
        before.rfind(char::is_whitespace).map_or(0, |i| {
            i + before[i..].chars().next().map_or(1, char::len_utf8)
        })
    }

    /// Applies an editing key. Returns `false` if the key is not an editing key.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.text.len(),
            KeyCode::Char('u') if ctrl => {
                self.text.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char('k') if ctrl => self.text.truncate(self.cursor),
            KeyCode::Char('w') if ctrl => {
                let start = self.word_start();
                self.text.drain(start..self.cursor);
                self.cursor = start;
            }
            KeyCode::Char(_) if ctrl => return false,
            KeyCode::Char(c) => {
                self.text.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let p = self.prev();
                self.text.drain(p..self.cursor);
                self.cursor = p;
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                let n = self.next();
                self.text.drain(self.cursor..n);
            }
            KeyCode::Left => self.cursor = self.prev(),
            KeyCode::Right => self.cursor = self.next(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            KeyCode::Backspace | KeyCode::Delete => {}
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
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
        i.handle(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(i.text(), "call the ");
    }
}
