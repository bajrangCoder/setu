use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Styled,
    Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::spinner::Spinner;
use gpui_component::Sizable;

use crate::components::StatusBadge;
use crate::entities::{ResponseData, ResponseEntity, ResponseEvent, ResponseState};
use crate::theme::Theme;

/// Active tab in the response panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseTab {
    #[default]
    Body,
    Headers,
}

/// Response view
pub struct ResponseView {
    pub response: Entity<ResponseEntity>,
    active_tab: ResponseTab,
    body_display: Option<Entity<InputState>>,
    /// Tracks the last displayed body content to know when to update
    last_body_content: String,
    focus_handle: FocusHandle,
}

impl ResponseView {
    pub fn new(response: Entity<ResponseEntity>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&response, |_this, _response, _event: &ResponseEvent, cx| {
            // Just notify to trigger re-render, which will update the display
            cx.notify();
        })
        .detach();

        Self {
            response,
            active_tab: ResponseTab::Body,
            body_display: None,
            last_body_content: String::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Initialize or update the body display with Window access (called from render)
    fn ensure_body_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Get current response body
        let current_content = if let Some(ref data) = self.response.read(cx).data {
            data.formatted_body()
        } else {
            String::new()
        };

        if self.body_display.is_none() {
            // Create the body display for the first time
            let content = current_content.clone();
            let body_display = cx.new(|cx| {
                InputState::new(window, cx)
                    //.multi_line(true)
                    .code_editor("json")
                    .line_number(true)
                    .default_value(&content)
            });
            self.body_display = Some(body_display);
            self.last_body_content = current_content;
        } else if current_content != self.last_body_content {
            // Content has changed, update the display
            if let Some(ref body_display) = self.body_display {
                let content = current_content.clone();
                body_display.update(cx, |state, cx| {
                    state.replace(&content, window, cx);
                });
                self.last_body_content = current_content;
            }
        }
    }

    pub fn set_tab(&mut self, tab: ResponseTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }
}

impl Focusable for ResponseView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ResponseView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure body display is initialized and up-to-date
        self.ensure_body_display(window, cx);

        let theme = Theme::dark();

        // Clone what we need before borrowing
        let state = self.response.read(cx).state.clone();
        let data = self.response.read(cx).data.clone();

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .child(self.render_content(&theme, &state, data.as_ref(), cx))
    }
}

impl ResponseView {
    fn render_content(
        &self,
        theme: &Theme,
        state: &ResponseState,
        data: Option<&ResponseData>,
        cx: &Context<Self>,
    ) -> AnyElement {
        match state {
            ResponseState::Idle => self.render_empty(theme).into_any_element(),
            ResponseState::Loading => self.render_loading(theme).into_any_element(),
            ResponseState::Error(msg) => self.render_error(theme, msg).into_any_element(),
            ResponseState::Success => {
                if let Some(data) = data {
                    self.render_success(theme, data, cx).into_any_element()
                } else {
                    self.render_empty(theme).into_any_element()
                }
            }
        }
    }

    fn render_empty(&self, theme: &Theme) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_color(theme.colors.text_muted)
                    .text_size(px(12.0))
                    .child("Response will appear here"),
            )
            .child(
                div()
                    .text_color(theme.colors.text_placeholder)
                    .text_size(px(11.0))
                    .child("Send a request to get started"),
            )
    }

    fn render_loading(&self, theme: &Theme) -> impl IntoElement {
        // Use gpui-component's animated Spinner
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .child(Spinner::new().large())
            .child(
                div()
                    .text_color(theme.colors.text_muted)
                    .text_size(px(12.0))
                    .child("Sending request..."),
            )
    }

    fn render_error(&self, theme: &Theme, message: &str) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .p(px(16.0))
            .child(
                div()
                    .text_color(theme.colors.error)
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Request failed"),
            )
            .child(
                div()
                    .text_color(theme.colors.text_muted)
                    .text_size(px(11.0))
                    .child(message.to_string()),
            )
    }

    fn render_success(
        &self,
        theme: &Theme,
        data: &ResponseData,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let this = cx.entity().clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            // Header with status + tabs
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(16.0))
                    .h(px(40.0))
                    .border_b_1()
                    .border_color(theme.colors.border_primary)
                    // Left: status + meta
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(StatusBadge::new(data.status_code))
                            .child(
                                div()
                                    .text_color(theme.colors.text_muted)
                                    .text_size(px(11.0))
                                    .child(data.formatted_duration()),
                            )
                            .child(
                                div()
                                    .text_color(theme.colors.text_muted)
                                    .text_size(px(11.0))
                                    .child(data.formatted_size()),
                            ),
                    )
                    .child(self.render_tabs(theme, this)),
            )
            // Content - fills remaining space
            .child(
                div()
                    .id("response-content-wrapper")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.render_tab_content(theme, data)),
            )
    }

    fn render_tabs(&self, _theme: &Theme, this: Entity<ResponseView>) -> impl IntoElement {
        use crate::components::PanelTab;

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .child(
                PanelTab::new("Body")
                    .active(self.active_tab == ResponseTab::Body)
                    .on_click({
                        let this = this.clone();
                        move |_event, _window, cx| {
                            this.update(cx, |view, cx| view.set_tab(ResponseTab::Body, cx));
                        }
                    }),
            )
            .child(
                PanelTab::new("Headers")
                    .active(self.active_tab == ResponseTab::Headers)
                    .on_click({
                        let this = this.clone();
                        move |_event, _window, cx| {
                            this.update(cx, |view, cx| view.set_tab(ResponseTab::Headers, cx));
                        }
                    }),
            )
    }

    fn render_tab_content(&self, theme: &Theme, data: &ResponseData) -> AnyElement {
        match self.active_tab {
            ResponseTab::Body => self.render_body_tab(theme).into_any_element(),
            ResponseTab::Headers => self.render_headers_tab(theme, data).into_any_element(),
        }
    }

    fn render_body_tab(&self, theme: &Theme) -> impl IntoElement {
        // Clean minimal container - no padding before line numbers
        div()
            .id("body-scroll-container")
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .bg(theme.colors.bg_tertiary)
            .when_some(self.body_display.as_ref(), |el, editor| {
                el.child(Input::new(editor).appearance(false).size_full())
            })
    }

    fn render_headers_tab(&self, theme: &Theme, data: &ResponseData) -> impl IntoElement {
        div()
            .id("headers-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .bg(theme.colors.bg_tertiary)
            .rounded(px(6.0))
            .children(data.headers.iter().enumerate().map(|(idx, (key, value))| {
                // Alternate row backgrounds for readability
                let bg_color = if idx % 2 == 0 {
                    theme.colors.bg_secondary
                } else {
                    theme.colors.bg_tertiary
                };

                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .py(px(10.0))
                    .px(px(16.0))
                    .bg(bg_color)
                    .border_b_1()
                    .border_color(theme.colors.border_primary.opacity(0.3))
                    // Header key column
                    .child(
                        div()
                            .w(px(220.0))
                            .min_w(px(220.0))
                            .pr(px(16.0))
                            .text_color(theme.colors.text_secondary)
                            .text_size(px(12.0))
                            .child(key.clone()),
                    )
                    // Header value column
                    .child(
                        div()
                            .flex_1()
                            .text_color(theme.colors.text_primary)
                            .text_size(px(12.0))
                            .text_ellipsis()
                            .child(value.clone()),
                    )
            }))
    }
}
