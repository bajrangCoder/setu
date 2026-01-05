use gpui::{px, Pixels};

/// Typography settings
#[derive(Clone)]
pub struct Typography {
    // Font sizes
    pub size_xs: Pixels,
    pub size_sm: Pixels,
    pub size_base: Pixels,
    pub size_lg: Pixels,
    pub size_xl: Pixels,
    pub size_2xl: Pixels,
    pub size_3xl: Pixels,

    // Font families (using system fonts)
    pub font_sans: &'static str,
    pub font_mono: &'static str,

    // Line heights
    pub line_height_tight: f32,
    pub line_height_normal: f32,
    pub line_height_relaxed: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            // Size scale
            size_xs: px(11.0),
            size_sm: px(12.0),
            size_base: px(13.0),
            size_lg: px(14.0),
            size_xl: px(16.0),
            size_2xl: px(20.0),
            size_3xl: px(24.0),

            // Font families
            font_sans: "Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
            font_mono: "'JetBrains Mono', 'SF Mono', Menlo, Monaco, monospace",

            // Line heights
            line_height_tight: 1.25,
            line_height_normal: 1.5,
            line_height_relaxed: 1.75,
        }
    }
}
