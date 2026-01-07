use gpui::prelude::*;
use gpui::{div, px, App, ClickEvent, Entity, IntoElement, Styled, Window};
use gpui_component::input::{Input, InputState};

use crate::components::{MethodDropdown, MethodDropdownState};
use crate::entities::RequestEntity;
use gpui_component::ActiveTheme;

/// Callback type for Send button
pub type OnSendCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// URL Bar component
#[derive(IntoElement)]
pub struct UrlBar {
    input_state: Entity<InputState>,
    method_dropdown: Option<Entity<MethodDropdownState>>,
    request: Option<Entity<RequestEntity>>,
    is_loading: bool,
    on_send: Option<OnSendCallback>,
}

impl UrlBar {
    pub fn new(input_state: Entity<InputState>) -> Self {
        Self {
            input_state,
            method_dropdown: None,
            request: None,
            is_loading: false,
            on_send: None,
        }
    }

    pub fn method_dropdown(
        mut self,
        dropdown_state: Entity<MethodDropdownState>,
        request: Entity<RequestEntity>,
    ) -> Self {
        self.method_dropdown = Some(dropdown_state);
        self.request = Some(request);
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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let is_loading = self.is_loading;

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .w_full()
            .h(px(40.0))
            .bg(theme.muted)
            .rounded(px(6.0))
            // Method dropdown trigger
            .when_some(
                self.method_dropdown.zip(self.request),
                |el, (dropdown_state, request)| {
                    el.child(
                        div()
                            .ml(px(4.0))
                            .child(MethodDropdown::new(dropdown_state, request)),
                    )
                },
            )
            // Divider
            .child(div().w(px(1.0)).h(px(20.0)).bg(theme.border))
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
                    .bg(theme.primary)
                    .text_color(theme.background)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_size(px(12.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.primary_hover))
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
