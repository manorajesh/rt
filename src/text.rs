use log::error;

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub character: char,
    pub fg_color: Color,
    pub bg_color: Color,
    pub style: Style,
}

#[derive(Clone, Copy, Debug)]
pub enum Color {
    Default,
    RGB(u8, u8, u8),
}

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            character: '\0',
            fg_color: Color::Default,
            bg_color: Color::Default,
            style: Style::default(),
        }
    }
}

impl Default for Style {
    fn default() -> Self {
        Style {
            bold: false,
            italic: false,
        }
    }
}

#[derive(Debug)]
pub struct Text {
    pub buffer: Vec<Cell>, // Flat Vec for text buffer
    width: usize,
    height: usize,
    viewport: Viewport,
}

impl Text {
    pub fn new(width: usize, height: usize) -> Self {
        let buffer = Vec::with_capacity(width * height);
        let viewport = Viewport::new(height, width);

        Text { buffer, width, height, viewport }
    }

    pub fn push_str(&mut self, s: &str) {
        for c in s.chars() {
            self.push(c);
        }
    }

    pub fn push(&mut self, c: char) {
        if c == '\n' {
            self.push_newline();
        } else {
            let row = self.viewport.top_row;
            let col = self.buffer.len() % self.width;
            self.insert_char(row, col, c);
        }
    }

    pub fn push_newline(&mut self) {
        self.viewport.scroll_down(1, self.height);
    }

    // Insert a character at the given row and column
    pub fn insert_char(&mut self, row: usize, col: usize, c: char) {
        let index = row * self.width + col;

        if index < self.buffer.len() {
            if let Some(cell) = self.buffer.get_mut(index) {
                cell.character = c;
            }
        } else {
            // Dynamically push new cells if we're beyond the current buffer size
            while self.buffer.len() <= index {
                self.buffer.push(Cell::default());
            }
            self.buffer[index].character = c;
        }
        println!("{:?}", self.buffer);
    }

    // Get a reference to a cell at a specific row and column
    pub fn get_cell(&self, row: usize, col: usize) -> Option<&Cell> {
        let index = row * self.width + col;
        self.buffer.get(index)
    }

    // Get a mutable reference to a cell at a specific row and column
    pub fn get_cell_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        let index = row * self.width + col;
        self.buffer.get_mut(index)
    }

    // Resize the text buffer
    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        let mut new_buffer = Vec::with_capacity(new_width * new_height);

        let min_height = self.height.min(new_height);
        let min_width = self.width.min(new_width);

        for row in 0..min_height {
            for col in 0..min_width {
                if let Some(&cell) = self.get_cell(row, col) {
                    new_buffer.push(cell);
                } else {
                    new_buffer.push(Cell::default());
                }
            }
        }

        self.width = new_width;
        self.height = new_height;
        self.buffer = new_buffer;
    }

    // Scroll the viewport up by a given number of lines
    pub fn scroll_up(&mut self, amount: usize) {
        self.viewport.scroll_up(amount, self.height);
    }

    // Scroll the viewport down by a given number of lines
    pub fn scroll_down(&mut self, amount: usize) {
        self.viewport.scroll_down(amount, self.height);
    }

    // Render the current viewport
    pub fn render(&self) {
        // Example rendering function
        for (row, col, cell) in self {
            render_cell(cell, row, col);
        }
    }

    pub fn as_str(&self) -> String {
        self.buffer
            .iter()
            .map(|cell| cell.character)
            .collect()
    }
}

impl<'a> IntoIterator for &'a Text {
    type Item = (usize, usize, &'a Cell);
    type IntoIter = TextIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        TextIterator {
            text_display: self,
            row: 0,
            col: 0,
        }
    }
}

#[derive(Debug)]
pub struct TextIterator<'a> {
    text_display: &'a Text,
    row: usize,
    col: usize,
}

impl<'a> Iterator for TextIterator<'a> {
    type Item = (usize, usize, &'a Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.text_display.height {
            return None;
        }

        let cell = self.text_display.get_cell(self.row, self.col)?;

        let item = (self.row, self.col, cell);

        // Increment the column, and if it exceeds the width, reset it and move to the next row.
        self.col += 1;
        if self.col >= self.text_display.width {
            self.col = 0;
            self.row += 1;
        }

        Some(item)
    }
}

#[derive(Debug)]
pub struct Viewport {
    pub top_row: usize,
    pub height: usize,
    pub width: usize,
}

impl Viewport {
    pub fn new(height: usize, width: usize) -> Self {
        Viewport {
            top_row: 0,
            height,
            width,
        }
    }

    pub fn scroll_up(&mut self, amount: usize, buffer_height: usize) {
        self.top_row = self.top_row.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize, buffer_height: usize) {
        self.top_row = (self.top_row + amount).min(buffer_height.saturating_sub(self.height));
    }
}

fn render_cell(cell: &Cell, row: usize, col: usize) {
    println!("Rendering '{}' at ({}, {})", cell.character, row, col);
}
