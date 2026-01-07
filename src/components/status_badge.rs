use gpui::prelude::*;
use gpui::{div, px, App, IntoElement, Styled, Window};

use crate::theme::status_color;

/// Compact status badge
#[derive(IntoElement)]
pub struct StatusBadge {
    status_code: u16,
}

impl StatusBadge {
    pub fn new(status_code: u16) -> Self {
        Self { status_code }
    }

    #[allow(dead_code)]
    pub fn with_text(self, _text: &str) -> Self {
        // Keeping method for API compatibility, but not using text for minimal design
        self
    }
}

impl RenderOnce for StatusBadge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = status_color(self.status_code, cx);

        div()
            .text_color(color)
            .font_weight(gpui::FontWeight::BOLD)
            .text_size(px(12.0))
            .child(format!("{}", self.status_code))
    }
}
