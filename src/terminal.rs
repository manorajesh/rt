use std::collections::VecDeque;
use anstyle_parse::{ DefaultCharAccumulator, Parser, Perform };

#[derive(Default, Clone)]
struct Cell {
    char: char,
    // Optional: Add fields for colors and attributes if needed
    fg: Option<u8>, // Foreground color
    bg: Option<u8>, // Background color
    bold: bool, // Bold text attribute
    underline: bool, // Underline text attribute
}

struct TextBuffer {
    rows: VecDeque<Vec<Cell>>, // The entire buffer, including off-screen lines
    width: usize, // The width of the viewport in characters
    height: usize, // The height of the viewport in lines
    viewport_top: usize, // The index of the first visible line in the buffer
    cursor_x: usize, // Cursor X position (column)
    cursor_y: usize, // Cursor Y position (row relative to the viewport)
}

impl TextBuffer {
    fn new(width: usize, height: usize) -> Self {
        Self {
            rows: VecDeque::new(),
            width,
            height,
            viewport_top: 0,
            cursor_x: 0,
            cursor_y: 0,
        }
    }

    fn insert_char(&mut self, c: char) {
        if self.cursor_y >= self.height {
            self.scroll_down(1);
            self.cursor_y = self.height - 1;
        }

        // Ensure the current row exists
        if self.rows.len() <= self.viewport_top + self.cursor_y {
            self.rows.push_back(Vec::with_capacity(self.width));
        }

        // Insert the character at the current cursor position
        // let row = &mut self.rows[self.viewport_top + self.cursor_y];

        if let Some(row) = self.rows.get_mut(self.viewport_top + self.cursor_y) {
            if self.cursor_x < row.len() {
                row[self.cursor_x] = Cell { char: c, ..Default::default() };
            } else {
                row.push(Cell { char: c, ..Default::default() });
            }
        } else {
            // self.rows.push_back(vec![Cell { char: c, ..Default::default() }]);
        }

        self.cursor_x += 1;
        if self.cursor_x >= self.width {
            self.cursor_x = 0;
            self.cursor_y += 1;
            if self.cursor_y >= self.height {
                self.scroll_down(1);
                self.cursor_y = self.height - 1;
            }
        }
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        self.cursor_y += 1;
        if self.cursor_y >= self.height {
            self.scroll_down(1);
            self.cursor_y = self.height - 1;
        }
    }

    fn scroll_up(&mut self, lines: usize) {
        if self.viewport_top >= lines {
            self.viewport_top -= lines;
        } else {
            self.viewport_top = 0;
        }
    }

    fn scroll_down(&mut self, lines: usize) {
        if self.viewport_top + self.height + lines < self.rows.len() {
            self.viewport_top += lines;
        } else {
            self.viewport_top = self.rows.len().saturating_sub(self.height);
        }
    }

    fn render_viewport(&self) -> String {
        // let end = (self.viewport_top + self.height).min(self.rows.len());
        let mut result = String::new();
        for row in self.rows.iter().skip(self.viewport_top).take(self.height) {
            for cell in row {
                result.push(cell.char);
            }
            result.push('\n');
        }
        result
    }

    fn move_cursor(&mut self, x: usize, y: usize) {
        self.cursor_x = x.min(self.width - 1);
        self.cursor_y = y;

        if self.cursor_y >= self.height {
            self.scroll_down(self.cursor_y - self.height + 1);
            self.cursor_y = self.height - 1;
        }
    }
}

impl Perform for TextBuffer {
    fn print(&mut self, c: char) {
        self.insert_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => {
                self.cursor_x = 0;
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &anstyle_parse::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: u8
    ) {
        match action {
            b'A' => {
                let lines = *params
                    .iter()
                    .next()
                    .map(|p| p.get(0).unwrap_or(&1))
                    .unwrap_or(&1) as usize;
                self.cursor_y = self.cursor_y.saturating_sub(lines);
            }
            b'B' => {
                let lines = *params
                    .iter()
                    .next()
                    .map(|p| p.get(0).unwrap_or(&1))
                    .unwrap_or(&1) as usize;
                self.cursor_y = (self.cursor_y + lines).min(self.height - 1);
            }
            b'C' => {
                let cols = *params
                    .iter()
                    .next()
                    .map(|p| p.get(0).unwrap_or(&1))
                    .unwrap_or(&1) as usize;
                self.cursor_x = (self.cursor_x + cols).min(self.width - 1);
            }
            b'D' => {
                let cols = *params
                    .iter()
                    .next()
                    .map(|p| p.get(0).unwrap_or(&1))
                    .unwrap_or(&1) as usize;
                self.cursor_x = self.cursor_x.saturating_sub(cols);
            }
            _ => {}
        }
    }

    // Implement other methods as needed
}

pub struct Terminal {
    buffer: TextBuffer,
    parser: Parser,
}

impl Terminal {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            buffer: TextBuffer::new(width, height),
            parser: Parser::<DefaultCharAccumulator>::new(),
        }
    }

    pub fn process_input(&mut self, input: &[u8]) {
        for byte in input {
            self.parser.advance(&mut self.buffer, *byte);
        }
    }

    pub fn render_as_str(&self) -> String {
        self.buffer.render_viewport()
    }
}
