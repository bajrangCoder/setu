use gpui::prelude::*;
use gpui::{App, ClickEvent, IntoElement, Styled, Window, div, px};
use gpui_component::ActiveTheme;

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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let is_active = self.is_active;

        div()
            .id(self.label)
            .flex()
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
            .when(is_active, |style| {
                style.bg(theme.muted).text_color(theme.foreground)
            })
            .when(!is_active, |style| {
                style
                    .text_color(theme.muted_foreground)
                    .hover(|hover| hover.bg(theme.muted.opacity(0.5)))
            })
            .when_some(self.on_click, |element, callback| {
                element.on_click(move |event, window, cx| callback(event, window, cx))
            })
            .child(self.label)
    }
}

#[derive(IntoElement)]
pub struct PanelTabBar {
    children: Vec<PanelTab>,
    bordered: bool,
    align_end: bool,
}

impl PanelTabBar {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            bordered: true,
            align_end: false,
        }
    }

    pub fn child(mut self, tab: PanelTab) -> Self {
        self.children.push(tab);
        self
    }

    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    pub fn align_end(mut self) -> Self {
        self.align_end = true;
        self
    }
}

impl RenderOnce for PanelTabBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("panel-tab-bar")
            .flex()
            .items_center()
            .w_full()
            .h(px(36.0))
            .px(px(12.0))
            .when(self.bordered, |style| {
                style
                    .bg(theme.secondary)
                    .border_b_1()
                    .border_color(theme.border)
            })
            .when(self.align_end, |style| style.justify_end())
            .overflow_x_scroll()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .children(self.children),
            )
    }
}
