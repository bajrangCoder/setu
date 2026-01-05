use gpui::prelude::*;
use gpui::{div, px, App, ClickEvent, Entity, IntoElement, Styled, Window};
use gpui_component::input::{Input, InputState};

use crate::components::{MethodDropdownState, MethodDropdownTrigger};
use crate::theme::Theme;

/// Callback type for Send button
pub type OnSendCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// URL Bar component
#[derive(IntoElement)]
pub struct UrlBar {
    input_state: Entity<InputState>,
    method_dropdown: Option<Entity<MethodDropdownState>>,
    is_loading: bool,
    on_send: Option<OnSendCallback>,
}

impl UrlBar {
    pub fn new(input_state: Entity<InputState>) -> Self {
        Self {
            input_state,
            method_dropdown: None,
            is_loading: false,
            on_send: None,
        }
    }

    pub fn method_dropdown(mut self, dropdown_state: Entity<MethodDropdownState>) -> Self {
        self.method_dropdown = Some(dropdown_state);
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.is_loading = loading;
        self
    }

    pub fn on_send(
        mut self,
        callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_send = Some(Box::new(callback));
        self
    }
}

impl RenderOnce for UrlBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();
        let is_loading = self.is_loading;

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .w_full()
            .h(px(40.0))
            .bg(theme.colors.bg_tertiary)
            .rounded(px(6.0))
            // Method dropdown trigger
            .when_some(self.method_dropdown, |el, dropdown_state| {
                el.child(
                    div()
                        .ml(px(4.0))
                        .child(MethodDropdownTrigger::new(dropdown_state)),
                )
            })
            // Divider
            .child(div().w(px(1.0)).h(px(20.0)).bg(theme.colors.border_primary))
            // URL input using gpui-component
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .h_full()
                    .px(px(8.0))
                    .child(
                        Input::new(&self.input_state)
                            .appearance(false) // Remove default styling
                            .size_full(),
                    ),
            )
            // Send button
            .child(
                div()
                    .id("send-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(px(16.0))
                    .h(px(32.0))
                    .mr(px(4.0))
                    .rounded(px(4.0))
                    .bg(theme.colors.accent)
                    .text_color(theme.colors.bg_primary)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_size(px(12.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.colors.accent_hover))
                    .when(is_loading, |s| s.opacity(0.7))
                    .when_some(self.on_send, |el, callback| {
                        el.on_click(move |event, window, cx| {
                            if !is_loading {
                                callback(event, window, cx);
                            }
                        })
                    })
                    .child(if is_loading { "Sending..." } else { "Send" }),
            )
    }
}
