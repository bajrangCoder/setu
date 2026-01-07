use gpui::prelude::*;
use gpui::{div, px, App, IntoElement, Styled, Window};
use gpui_component::sidebar::{
    Sidebar as GpuiSidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
};
use gpui_component::{ActiveTheme, Side};

use crate::entities::HistoryEntry;
use crate::theme::method_color;

/// Sidebar component using gpui-component's Sidebar
#[derive(IntoElement)]
pub struct Sidebar {
    history: Vec<HistoryEntry>,
}

impl Sidebar {
    pub fn new(history: Vec<HistoryEntry>) -> Self {
        Self { history }
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

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
                        .text_color(theme.muted_foreground)
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
            .w_full()
            .into_any_element()
    }
}

/// Single history item - kept for reference/alternate usage
#[derive(IntoElement)]
#[allow(dead_code)]
pub struct HistoryItem {
    entry: HistoryEntry,
}

#[allow(dead_code)]
impl HistoryItem {
    pub fn new(entry: HistoryEntry) -> Self {
        Self { entry }
    }
}

impl RenderOnce for HistoryItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let m_color = method_color(&self.entry.request.method, cx);
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
            .hover(|s| s.bg(theme.muted))
            // Method - compact, colored
            .child(
                div()
                    .text_color(m_color)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(9.0))
                    .w(px(32.0))
                    .child(method),
            )
            // URL path
            .child(
                div()
                    .flex_1()
                    .text_color(theme.secondary_foreground)
                    .text_size(px(11.0))
                    .overflow_hidden()
                    .child(display_name),
            )
    }
}
