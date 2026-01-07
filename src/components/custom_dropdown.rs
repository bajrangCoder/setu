use gpui::prelude::*;
use gpui::{
    anchored, deferred, div, px, App, Corner, Entity, IntoElement, SharedString, Styled, Window,
};
use gpui_component::ActiveTheme;

use crate::entities::HttpMethod;
use crate::icons::IconName;
use crate::theme::method_color;

/// State for the method dropdown
pub struct MethodDropdownState {
    pub selected: HttpMethod,
    pub is_open: bool,
    /// Tracks when the dropdown was last closed (to prevent immediate reopen on trigger click)
    close_time: Option<std::time::Instant>,
}

impl MethodDropdownState {
    pub fn new(method: HttpMethod) -> Self {
        Self {
            selected: method,
            is_open: false,
            close_time: None,
        }
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        // If we just closed (within 100ms), don't reopen
        if let Some(close_time) = self.close_time {
            if close_time.elapsed() < std::time::Duration::from_millis(100) {
                self.close_time = None;
                return;
            }
        }
        self.is_open = !self.is_open;
        self.close_time = None;
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.is_open = false;
        self.close_time = Some(std::time::Instant::now());
        cx.notify();
    }

    pub fn select(&mut self, method: HttpMethod, cx: &mut Context<Self>) {
        self.selected = method;
        self.is_open = false;
        self.close_time = None;
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn selected(&self) -> HttpMethod {
        self.selected
    }

    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        self.is_open
    }
}

/// Unified Method Dropdown component (Trigger + Menu)
#[derive(IntoElement)]
pub struct MethodDropdown {
    state: Entity<MethodDropdownState>,
    request: Entity<crate::entities::RequestEntity>,
}

impl MethodDropdown {
    pub fn new(
        state: Entity<MethodDropdownState>,
        request: Entity<crate::entities::RequestEntity>,
    ) -> Self {
        Self { state, request }
    }
}

impl RenderOnce for MethodDropdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let selected = state.selected;
        let is_open = state.is_open;
        let color = method_color(&selected, cx);

        // Clones for trigger and menu
        let state_ref = self.state.clone();
        let state_for_trigger = state_ref.clone();
        let state_for_menu = state_ref.clone();
        let request_ref = self.request.clone();

        let trigger = div()
            .id("method-dropdown-trigger")
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(10.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(theme.muted))
            .on_click(move |_event, _window, cx| {
                state_for_trigger.update(cx, |s, cx| s.toggle(cx));
            })
            .child(
                div()
                    .text_color(color)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(11.0))
                    .child(selected.as_str()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(10.0))
                    .child(if is_open {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    }),
            );

        if !is_open {
            return trigger.into_any_element();
        }

        let state_for_close = state_for_menu.clone();

        div()
            .child(trigger)
            .child(deferred(
                anchored()
                    .snap_to_window_with_margin(px(8.0))
                    .anchor(Corner::TopLeft)
                    .child(
                        div()
                            .mt(px(4.0))
                            .min_w(px(120.0))
                            .bg(theme.popover)
                            .border_1()
                            .border_color(theme.border)
                            .rounded(px(6.0))
                            .shadow_lg()
                            .overflow_hidden()
                            .id("method-dropdown-menu")
                            .on_mouse_down_out(move |_event, _window, cx| {
                                state_for_close.update(cx, |s, cx| s.close(cx));
                            })
                            .children(
                                [
                                    HttpMethod::Get,
                                    HttpMethod::Post,
                                    HttpMethod::Put,
                                    HttpMethod::Delete,
                                    HttpMethod::Patch,
                                    HttpMethod::Head,
                                    HttpMethod::Options,
                                ]
                                .into_iter()
                                .map(|method| {
                                    let color = method_color(&method, cx);
                                    let is_selected = method == selected;
                                    let state = state_for_menu.clone();
                                    let request = request_ref.clone();

                                    div()
                                        .id(SharedString::from(format!(
                                            "method-{}",
                                            method.as_str()
                                        )))
                                        .flex()
                                        .items_center()
                                        .px(px(14.0))
                                        .py(px(10.0))
                                        .cursor_pointer()
                                        .text_color(color)
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_size(px(12.0))
                                        .when(is_selected, |s| s.bg(theme.muted))
                                        .hover(|s| s.bg(theme.muted))
                                        .on_click(move |_event, _window, cx| {
                                            request.update(cx, |req, cx| {
                                                req.data.method = method;
                                                cx.notify();
                                            });
                                            state.update(cx, |s, cx| s.select(method, cx));
                                        })
                                        .child(method.as_str())
                                }),
                            ),
                    ),
            ))
            .into_any_element()
    }
}
