mod colors;
mod gpui_theme;
mod spacing;
mod typography;

pub use colors::*;
pub use gpui_theme::*;
pub use spacing::*;
pub use typography::*;

/// Main theme struct containing all design tokens
#[derive(Clone)]
pub struct Theme {
    pub colors: Colors,
    pub typography: Typography,
    pub spacing: Spacing,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            colors: Colors::dark(),
            typography: Typography::default(),
            spacing: Spacing::default(),
        }
    }
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            colors: Colors::dark(),
            ..Default::default()
        }
    }

    pub fn light() -> Self {
        Self {
            colors: Colors::light(),
            ..Default::default()
        }
    }
}
