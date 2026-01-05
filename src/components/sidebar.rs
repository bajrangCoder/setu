// Setu Sidebar - using gpui-component's Sidebar components
// Provides a collapsible history sidebar with consistent theming

use gpui::prelude::*;
use gpui::{div, px, App, IntoElement, Styled, Window};
use gpui_component::sidebar::{
    Sidebar as GpuiSidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
};
use gpui_component::Side;

use crate::entities::{HistoryEntry, HttpMethod};
use crate::theme::Theme;

/// Sidebar component using gpui-component's Sidebar
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

    fn method_color(method: &HttpMethod, theme: &Theme) -> gpui::Hsla {
        match method {
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

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();

        if !self.is_visible {
            return div().into_any_element();
        }

        // Build sidebar menu items from history
        let menu_items: Vec<SidebarMenuItem> = self
            .history
            .iter()
            .map(|entry| {
                let display_name = entry.display_name();
                let method_str = entry.request.method.as_str();
                // Combine method and path for display
                let label = format!("{} {}", method_str, display_name);

                SidebarMenuItem::new(label)
            })
            .collect();

        GpuiSidebar::new(Side::Left)
            .header(
                SidebarHeader::new().child(
                    div()
                        .text_color(theme.colors.text_muted)
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child("HISTORY"),
                ),
            )
            .child(if menu_items.is_empty() {
                // Show empty state
                SidebarGroup::new("Requests").child(
                    SidebarMenu::new().child(SidebarMenuItem::new("No history yet").disable(true)),
                )
            } else {
                // Show history items
                SidebarGroup::new("Requests").child(SidebarMenu::new().children(menu_items))
            })
            .into_any_element()
    }
}

/// Single history item - kept for reference/alternate usage
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
