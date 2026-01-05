use gpui::prelude::*;
use gpui::{div, px, App, IntoElement, Styled, Window};

use crate::theme::Theme;

/// Compact status badge
#[derive(IntoElement)]
pub struct StatusBadge {
    status_code: u16,
}

impl StatusBadge {
    pub fn new(status_code: u16) -> Self {
        Self { status_code }
    }

    pub fn with_text(self, _text: &str) -> Self {
        // Keeping method for API compatibility, but not using text for minimal design
        self
    }

    fn status_color(&self, theme: &Theme) -> gpui::Hsla {
        match self.status_code / 100 {
            1 => theme.colors.status_1xx,
            2 => theme.colors.status_2xx,
            3 => theme.colors.status_3xx,
            4 => theme.colors.status_4xx,
            5 => theme.colors.status_5xx,
            _ => theme.colors.text_muted,
        }
    }
}

impl RenderOnce for StatusBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();
        let color = self.status_color(&theme);

        div()
            .text_color(color)
            .font_weight(gpui::FontWeight::BOLD)
            .text_size(px(12.0))
            .child(format!("{}", self.status_code))
    }
}
