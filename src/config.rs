#[derive(Debug, Clone)]
pub struct Config {
    pub font_size: u32,
    pub font_path: String,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub glyph_cache_size: usize,
    pub num_rows: usize,
    pub num_cols: usize,
}

const FONT_SIZE: u32 = 16;

impl Config {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            font_size: FONT_SIZE,
            font_path: "../Inter-Bold.ttf".to_string(),
            atlas_width: 1024,
            atlas_height: 1024,
            glyph_cache_size: 1024,
            num_rows: (screen_height as usize) / (FONT_SIZE as usize),
            num_cols: (screen_width as usize) / (FONT_SIZE as usize),
        }
    }
}
