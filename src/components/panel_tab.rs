use gpui::prelude::*;
use gpui::{div, px, App, ClickEvent, IntoElement, Styled, Window};

use crate::theme::Theme;

/// Callback type for tab click
pub type OnTabClickCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct PanelTab {
    label: &'static str,
    is_active: bool,
    on_click: Option<OnTabClickCallback>,
}

impl PanelTab {
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            is_active: false,
            on_click: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.is_active = active;
        self
    }

    pub fn on_click(
        mut self,
        callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(callback));
        self
    }
}

impl RenderOnce for PanelTab {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();
        let is_active = self.is_active;

        div()
            .id(self.label)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(28.0))
            .px(px(10.0))
            .mx(px(2.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .text_size(px(11.0))
            .font_weight(if is_active {
                gpui::FontWeight::MEDIUM
            } else {
                gpui::FontWeight::NORMAL
            })
            .when(is_active, |s| {
                s.bg(theme.colors.bg_tertiary)
                    .text_color(theme.colors.text_primary)
            })
            .when(!is_active, |s| {
                s.text_color(theme.colors.text_muted)
                    .hover(|h| h.bg(theme.colors.bg_tertiary.opacity(0.5)))
            })
            .when_some(self.on_click, |el, callback| {
                el.on_click(move |event, window, cx| {
                    callback(event, window, cx);
                })
            })
            .child(self.label)
    }
}

/// Container for panel tabs - horizontal scrollable row
#[derive(IntoElement)]
pub struct PanelTabBar {
    children: Vec<PanelTab>,
}

impl PanelTabBar {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    pub fn child(mut self, tab: PanelTab) -> Self {
        self.children.push(tab);
        self
    }
}

impl RenderOnce for PanelTabBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();

        div()
            .id("panel-tab-bar")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(36.0))
            .px(px(12.0))
            .bg(theme.colors.bg_secondary)
            .border_b_1()
            .border_color(theme.colors.border_primary)
            .overflow_scroll()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .children(self.children),
            )
    }
}
