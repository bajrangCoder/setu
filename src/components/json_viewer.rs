use gpui::prelude::*;
use gpui::{div, px, App, IntoElement, SharedString, Styled, Window};

use crate::theme::Theme;

/// JSON Viewer component
#[derive(IntoElement)]
pub struct JsonViewer {
    content: SharedString,
}

impl JsonViewer {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

impl RenderOnce for JsonViewer {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .p(px(16.0))
            .bg(theme.colors.bg_tertiary)
            .rounded(px(8.0))
            .overflow_hidden()
            .child(
                div()
                    .font_family("monospace")
                    .text_size(px(13.0))
                    .text_color(theme.colors.text_primary)
                    .child(self.content),
            )
    }
}

/// Empty state for response viewer
#[derive(IntoElement)]
pub struct ResponseEmpty;

impl RenderOnce for ResponseEmpty {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .w_full()
            .h_full()
            .gap(px(12.0))
            .child(
                // TODO: Add proper SVG icon for empty state
                div().text_size(px(48.0)).child("📡"),
            )
            .child(
                div()
                    .text_color(theme.colors.text_secondary)
                    .text_size(px(14.0))
                    .child("Enter a URL and send a request to see the response"),
            )
            .child(
                div()
                    .text_color(theme.colors.text_muted)
                    .text_size(px(12.0))
                    .child("Press Cmd+Enter to send"),
            )
    }
}

/// Loading state for response viewer
#[derive(IntoElement)]
pub struct ResponseLoading;

impl RenderOnce for ResponseLoading {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .w_full()
            .h_full()
            .gap(px(12.0))
            .child(
                // TODO: Add proper SVG loading spinner
                div().text_size(px(32.0)).child("⏳"),
            )
            .child(
                div()
                    .text_color(theme.colors.text_secondary)
                    .text_size(px(14.0))
                    .child("Sending request..."),
            )
    }
}

/// Error state for response viewer
#[derive(IntoElement)]
pub struct ResponseError {
    message: SharedString,
}

impl ResponseError {
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl RenderOnce for ResponseError {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .w_full()
            .p(px(24.0))
            .gap(px(12.0))
            .bg(theme.colors.error.opacity(0.1))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.colors.error.opacity(0.3))
            .child(
                // TODO: Add proper SVG error icon
                div().text_size(px(32.0)).child("❌"),
            )
            .child(
                div()
                    .text_color(theme.colors.error)
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Request Failed"),
            )
            .child(
                div()
                    .text_color(theme.colors.text_secondary)
                    .text_size(px(13.0))
                    .child(self.message),
            )
    }
}
