// Setu Method Dropdown
// Provides HTTP method selection with consistent theming
// Uses gpui-component theme system for all colors

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Styled, Window};
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
        // This prevents the trigger click from reopening after on_mouse_down_out closes it
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

/// Just the trigger button for the dropdown (renders in UrlBar)
#[derive(IntoElement)]
pub struct MethodDropdownTrigger {
    state: Entity<MethodDropdownState>,
}

impl MethodDropdownTrigger {
    pub fn new(state: Entity<MethodDropdownState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for MethodDropdownTrigger {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let selected = state.selected;
        let is_open = state.is_open;
        let color = method_color(&selected, cx);

        let state_entity = self.state.clone();

        div()
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
                state_entity.update(cx, |s, cx| s.toggle(cx));
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
            )
    }
}

/// Overlay dropdown menu for method selection
#[derive(IntoElement)]
pub struct MethodDropdownOverlay {
    state: Entity<MethodDropdownState>,
    request: Entity<crate::entities::RequestEntity>,
}

impl MethodDropdownOverlay {
    pub fn new(
        state: Entity<MethodDropdownState>,
        request: Entity<crate::entities::RequestEntity>,
    ) -> Self {
        Self { state, request }
    }
}

impl RenderOnce for MethodDropdownOverlay {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let is_open = state.is_open;
        let selected = state.selected;

        if !is_open {
            return div().into_any_element();
        }

        let state_entity = self.state.clone();
        let state_for_close = self.state.clone();
        let request_entity = self.request.clone();

        div()
            .absolute()
            .top(px(134.0))
            .left(px(255.0))
            .min_w(px(120.0))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .shadow_lg()
            .overflow_hidden()
            .id("method-dropdown-menu")
            // Close dropdown when clicking outside the menu
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
                    let state = state_entity.clone();
                    let request = request_entity.clone();

                    div()
                        .id(SharedString::from(format!("method-{}", method.as_str())))
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
                            // Update request method
                            request.update(cx, |req, cx| {
                                req.data.method = method;
                                cx.notify();
                            });
                            // Update dropdown state
                            state.update(cx, |s, cx| s.select(method, cx));
                        })
                        .child(method.as_str())
                }),
            )
            .into_any_element()
    }
}
