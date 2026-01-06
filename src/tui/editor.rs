use std::cmp::min;

use crate::card::CardType;

pub struct Editor {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
    scroll_top: usize,
    pub card_type: CardType,
}

impl Editor {
    pub fn new() -> Self {
        let card_type = CardType::Basic;
        let lines = Self::init_lines(&card_type);
        Self {
            lines,
            cursor_row: 0,
            cursor_col: 3,
            scroll_top: 0,
            card_type,
        }
    }
    fn init_lines(card_type: &CardType) -> Vec<String> {
        match card_type {
            CardType::Basic => vec!["Q: ".to_string(), "A: ".to_string()],
            CardType::Cloze => vec!["C: ".to_string()],
        }
    }

    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn clear(&mut self) {
        self.lines = Self::init_lines(&self.card_type);
        self.cursor_row = 0;
        self.cursor_col = 3;
        self.scroll_top = 0;
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    pub fn scroll_top(&self) -> usize {
        self.scroll_top
    }

    pub fn ensure_cursor_visible(&mut self, view_height: usize) {
        if view_height == 0 {
            self.scroll_top = 0;
            return;
        }

        if self.cursor_row < self.scroll_top {
            self.scroll_top = self.cursor_row;
        } else {
            let bottom = self.scroll_top + view_height - 1;
            if self.cursor_row > bottom {
                self.scroll_top = self.cursor_row + 1 - view_height;
            }
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        let column = self.cursor_col;
        let line = self.current_line_mut();
        let idx = Self::char_to_byte_index(line, column);
        line.insert(idx, ch);
        self.cursor_col += 1;
    }

    pub fn insert_newline(&mut self) {
        let column = self.cursor_col;
        let line = self.current_line_mut();
        let idx = Self::char_to_byte_index(line, column);
        let remainder = line.split_off(idx);
        self.lines.insert(self.cursor_row + 1, remainder);
        self.cursor_row += 1;
        self.cursor_col = 0;
    }

    pub fn insert_tab(&mut self) {
        self.insert_char('\t');
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let column = self.cursor_col;
            let line = self.current_line_mut();
            let end = Self::char_to_byte_index(line, column);
            let start = Self::char_to_byte_index(line, column - 1);
            line.drain(start..end);
            self.cursor_col -= 1;
            return;
        }

        if self.cursor_row == 0 {
            return;
        }

        let current_line = self.lines.remove(self.cursor_row);
        self.cursor_row -= 1;
        let new_col = self.line_len(self.cursor_row);
        self.cursor_col = new_col;
        let prev_line = self.current_line_mut();
        prev_line.push_str(&current_line);
    }

    pub fn delete(&mut self) {
        let line_len = self.line_len(self.cursor_row);
        if self.cursor_col < line_len {
            let column = self.cursor_col;
            let line = self.current_line_mut();
            let start = Self::char_to_byte_index(line, column);
            let end = Self::char_to_byte_index(line, column + 1);
            line.drain(start..end);
            return;
        }

        if self.cursor_row + 1 >= self.lines.len() {
            return;
        }

        let next_line = self.lines.remove(self.cursor_row + 1);
        let line = self.current_line_mut();
        line.push_str(&next_line);
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.line_len(self.cursor_row);
        }
    }

    pub fn move_right(&mut self) {
        let line_len = self.line_len(self.cursor_row);
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row == 0 {
            return;
        }
        self.cursor_row -= 1;
        self.cursor_col = min(self.cursor_col, self.line_len(self.cursor_row));
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 >= self.lines.len() {
            return;
        }
        self.cursor_row += 1;
        self.cursor_col = min(self.cursor_col, self.line_len(self.cursor_row));
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.line_len(self.cursor_row);
    }

    fn current_line_mut(&mut self) -> &mut String {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        &mut self.lines[self.cursor_row]
    }

    fn line_len(&self, row: usize) -> usize {
        self.lines
            .get(row)
            .map(|line| line.chars().count())
            .unwrap_or(0)
    }

    fn char_to_byte_index(line: &str, column: usize) -> usize {
        line.char_indices()
            .nth(column)
            .map(|(idx, _)| idx)
            .unwrap_or_else(|| line.len())
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
