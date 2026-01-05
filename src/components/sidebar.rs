use gpui::prelude::*;
use gpui::{div, px, App, IntoElement, Styled, Window};

use crate::entities::{HistoryEntry, HttpMethod};
use crate::theme::Theme;

/// Sidebar component
#[derive(IntoElement)]
pub struct Sidebar {
    history: Vec<HistoryEntry>,
    is_visible: bool,
}

impl Sidebar {
    pub fn new(history: Vec<HistoryEntry>) -> Self {
        Self {
            history,
            is_visible: true,
        }
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.is_visible = visible;
        self
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();

        div()
            .when(!self.is_visible, |s| s.hidden())
            .flex()
            .flex_col()
            .w(px(240.0))
            .h_full()
            .bg(theme.colors.bg_secondary)
            .border_r_1()
            .border_color(theme.colors.border_primary)
            // Header - minimal
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(44.0))
                    .px(px(12.0))
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("HISTORY"),
                    ),
            )
            // History list
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .when(self.history.is_empty(), |el| {
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .flex_1()
                                .text_color(theme.colors.text_placeholder)
                                .text_size(px(11.0))
                                .child("No history yet"),
                        )
                    })
                    .children(
                        self.history
                            .into_iter()
                            .map(|entry| HistoryItem::new(entry)),
                    ),
            )
    }
}

/// Single history item - compact
#[derive(IntoElement)]
pub struct HistoryItem {
    entry: HistoryEntry,
}

impl HistoryItem {
    pub fn new(entry: HistoryEntry) -> Self {
        Self { entry }
    }

    fn method_color(&self, theme: &Theme) -> gpui::Hsla {
        match self.entry.request.method {
            HttpMethod::Get => theme.colors.method_get,
            HttpMethod::Post => theme.colors.method_post,
            HttpMethod::Put => theme.colors.method_put,
            HttpMethod::Delete => theme.colors.method_delete,
            HttpMethod::Patch => theme.colors.method_patch,
            HttpMethod::Head => theme.colors.method_head,
            HttpMethod::Options => theme.colors.method_options,
        }
    }
}

impl RenderOnce for HistoryItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();
        let method_color = self.method_color(&theme);
        let display_name = self.entry.display_name();
        let method = self.entry.request.method.as_str();

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .px(px(12.0))
            .py(px(6.0))
            .cursor_pointer()
            .hover(|s| s.bg(theme.colors.bg_tertiary))
            // Method - compact, colored
            .child(
                div()
                    .text_color(method_color)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(9.0))
                    .w(px(32.0))
                    .child(method),
            )
            // URL path
            .child(
                div()
                    .flex_1()
                    .text_color(theme.colors.text_secondary)
                    .text_size(px(11.0))
                    .overflow_hidden()
                    .child(display_name),
            )
    }
}
