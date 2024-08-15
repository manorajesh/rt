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
        if self.viewport_top + lines < self.rows.len() {
            self.viewport_top += lines;
        } else {
            self.viewport_top = self.rows.len().saturating_sub(1);
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

    // Helper methods for handling attributes
    fn reset_attributes(&mut self) {
        if let Some(row) = self.rows.get_mut(self.viewport_top + self.cursor_y) {
            for cell in row {
                cell.bold = false;
                cell.underline = false;
                cell.fg = None;
                cell.bg = None;
            }
        }
    }

    fn set_bold(&mut self, bold: bool) {
        if let Some(row) = self.rows.get_mut(self.viewport_top + self.cursor_y) {
            if let Some(cell) = row.get_mut(self.cursor_x) {
                cell.bold = bold;
            }
        }
    }

    fn set_underline(&mut self, underline: bool) {
        if let Some(row) = self.rows.get_mut(self.viewport_top + self.cursor_y) {
            if let Some(cell) = row.get_mut(self.cursor_x) {
                cell.underline = underline;
            }
        }
    }

    fn set_foreground_color(&mut self, color: u8) {
        if let Some(row) = self.rows.get_mut(self.viewport_top + self.cursor_y) {
            if let Some(cell) = row.get_mut(self.cursor_x) {
                cell.fg = Some(color);
            }
        }
    }

    fn set_background_color(&mut self, color: u8) {
        if let Some(row) = self.rows.get_mut(self.viewport_top + self.cursor_y) {
            if let Some(cell) = row.get_mut(self.cursor_x) {
                cell.bg = Some(color);
            }
        }
    }

    fn erase_screen_from_cursor(&mut self) {
        // Erase from cursor to the end of the screen
        for row in self.rows.iter_mut().skip(self.viewport_top + self.cursor_y) {
            for cell in row.iter_mut().skip(self.cursor_x) {
                *cell = Cell::default();
            }
        }
    }

    fn erase_screen_to_cursor(&mut self) {
        // Erase from start of the screen to the cursor
        for row in self.rows.iter_mut().take(self.viewport_top + self.cursor_y + 1) {
            for cell in row.iter_mut().take(self.cursor_x + 1) {
                *cell = Cell::default();
            }
        }
    }

    fn erase_screen(&mut self) {
        // Erase the entire screen
        for row in self.rows.iter_mut() {
            for cell in row.iter_mut() {
                *cell = Cell::default();
            }
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
            0x07 => {
                // Bell
                println!("Bell");
            }
            0x08 => {
                // Backspace
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
            }
            0x09 => {
                // Tab
                self.cursor_x += 8 - (self.cursor_x % 8);
                if self.cursor_x >= self.width {
                    self.newline();
                }
            }
            0x0f => {
                // Shift In (SI)
                // Character set changes can be handled here if needed
                println!("Shift In (SI) - Unhandled control character");
            }
            _ => {
                println!("Unhandled control character: {}", byte);
            }
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
            b'K' => {
                let mode = *params
                    .iter()
                    .next()
                    .map(|p| p.get(0).unwrap_or(&0))
                    .unwrap_or(&0);

                if let Some(row) = self.rows.get_mut(self.viewport_top + self.cursor_y) {
                    match mode {
                        0 => {
                            // Erase from cursor to end of line
                            for cell in row.iter_mut().skip(self.cursor_x) {
                                *cell = Cell::default();
                            }
                        }
                        1 => {
                            // Erase from start of line to cursor
                            for cell in row.iter_mut().take(self.cursor_x + 1) {
                                *cell = Cell::default();
                            }
                        }
                        2 => {
                            // Erase entire line
                            for cell in row.iter_mut() {
                                *cell = Cell::default();
                            }
                        }
                        _ => {
                            println!("Unhandled CSI K mode: {}", mode);
                        }
                    }
                }
            }
            b'H' | b'f' => {
                let y = *params
                    .iter()
                    .next()
                    .map(|p| p.get(0).unwrap_or(&1))
                    .unwrap_or(&1) as usize;
                let x = *params
                    .iter()
                    .next()
                    .map(|p| p.get(0).unwrap_or(&1))
                    .unwrap_or(&1) as usize;
                self.move_cursor(x.saturating_sub(1), y.saturating_sub(1));
            }
            b'r' => {
                // Set Scrolling Region (CSI r)
                let top = *params
                    .iter()
                    .next()
                    .and_then(|p| p.get(0))
                    .unwrap_or(&1) as usize;
                let bottom = *params
                    .iter()
                    .next()
                    .and_then(|p| p.get(0))
                    .unwrap_or(&(self.height as u16)) as usize;

                // Normally, this would set up the scrolling region, but here we just print it
                println!("Set scrolling region: top = {}, bottom = {}", top, bottom);
            }
            b'm' => {
                // SGR (Set Graphics Rendition)
                for param in params {
                    match param.get(0).unwrap_or(&0) {
                        0 => {
                            // Reset all attributes
                            self.reset_attributes();
                        }
                        1 => {
                            // Bold on
                            self.set_bold(true);
                        }
                        4 => {
                            // Underline on
                            self.set_underline(true);
                        }
                        30..=37 => {
                            // Set foreground color (30-37)
                            self.set_foreground_color((*param.get(0).unwrap_or(&30) - 30) as u8);
                        }
                        40..=47 => {
                            // Set background color (40-47)
                            self.set_background_color((*param.get(0).unwrap_or(&40) - 40) as u8);
                        }
                        _ => {
                            println!("Unhandled SGR parameter: {}", param.get(0).unwrap_or(&0));
                        }
                    }
                }
            }
            b'l' => {
                // Reset Mode (CSI l)
                // This could handle reset modes, such as disabling line wrap, etc.
                println!("Reset Mode - Unhandled CSI action: l");
            }
            b'h' => {
                // Set Mode (CSI h)
                // This could handle set modes, such as enabling line wrap, etc.
                println!("Set Mode - Unhandled CSI action: h");
            }
            b'J' => {
                // Erase in Display (CSI J)
                let mode = *params
                    .iter()
                    .next()
                    .and_then(|p| p.get(0))
                    .unwrap_or(&0);

                match mode {
                    0 => {
                        // Erase from cursor to end of screen
                        self.erase_screen_from_cursor();
                    }
                    1 => {
                        // Erase from start of screen to cursor
                        self.erase_screen_to_cursor();
                    }
                    2 => {
                        // Erase entire screen
                        self.erase_screen();
                    }
                    _ => {
                        println!("Unhandled CSI J mode: {}", mode);
                    }
                }
            }
            b'P' => {
                // Delete Character (CSI P)
                let count = *params
                    .iter()
                    .next()
                    .map(|p| p.get(0).unwrap_or(&1))
                    .unwrap_or(&1) as usize;
                if let Some(row) = self.rows.get_mut(self.viewport_top + self.cursor_y) {
                    for _ in 0..count {
                        if self.cursor_x < row.len() {
                            row.remove(self.cursor_x);
                        }
                    }
                }
            }
            _ => {
                println!("Unhandled CSI action: {}", action);
            }
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            66 => {
                // Escape sequence for 'B'
                println!("Handled escape sequence: B");
            }
            48 => {
                // Escape sequence for '0'
                println!("Handled escape sequence: 0");
            }
            _ => {
                println!("Unhandled escape sequence: {}", byte);
            }
        }
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _ignore: bool) {
        println!("Unhandled OSC sequence");
    }

    fn hook(
        &mut self,
        _params: &anstyle_parse::Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: u8
    ) {
        println!("Unhandled DCS hook");
    }

    fn put(&mut self, _byte: u8) {
        println!("Unhandled DCS put");
    }

    fn unhook(&mut self) {
        println!("Unhandled DCS unhook");
    }
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

    pub fn show_buffer_stats(&self) {
        println!("Buffer stats:");
        println!("  Rows: {}", self.buffer.rows.len());
        println!("  Viewport top: {}", self.buffer.viewport_top);
        println!("  Cursor: ({}, {})", self.buffer.cursor_x, self.buffer.cursor_y);
    }

    pub fn scroll_buffer(&mut self, delta: f32) {
        let delta = delta.clamp(-5.0, 5.0);
        if delta > 0.0 {
            self.buffer.scroll_up(delta as usize);
        } else {
            self.buffer.scroll_down(-delta as usize);
        }
    }
}
