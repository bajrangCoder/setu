use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    div, px, size, AnyElement, App, Context, ElementId, Entity, FocusHandle, Focusable,
    IntoElement, Pixels, Render, SharedString, Size, Styled, Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::Scrollbar;
use gpui_component::spinner::Spinner;
use gpui_component::v_virtual_list;
use gpui_component::Sizable;
use gpui_component::VirtualListScrollHandle;

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
    /// Virtual list scroll handle for headers tab
    headers_scroll_handle: VirtualListScrollHandle,
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
            headers_scroll_handle: VirtualListScrollHandle::new(),
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
                    .code_editor("json")
                    .line_number(true)
                    .searchable(true)
                    .default_value(&content)
            });
            self.body_display = Some(body_display);
            self.last_body_content = current_content;
        } else if current_content != self.last_body_content {
            // Content has changed, use set_value to completely replace and reset
            if let Some(ref body_display) = self.body_display {
                let content = current_content.clone();
                body_display.update(cx, |state, cx| {
                    // Use set_value instead of replace - it properly clears and resets scroll
                    state.set_value(content, window, cx);
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
                    .child(self.render_tab_content(theme, data, cx)),
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

    fn render_tab_content(
        &self,
        theme: &Theme,
        data: &ResponseData,
        cx: &Context<Self>,
    ) -> AnyElement {
        match self.active_tab {
            ResponseTab::Body => self.render_body_tab(theme).into_any_element(),
            ResponseTab::Headers => self.render_headers_tab(theme, data, cx).into_any_element(),
        }
    }

    fn render_body_tab(&self, theme: &Theme) -> impl IntoElement {
        // Clean minimal container
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

    fn render_headers_tab(
        &self,
        theme: &Theme,
        data: &ResponseData,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        // Convert HashMap to Vec for indexing
        let headers: Vec<(String, String)> = data
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let header_count = headers.len();

        if header_count == 0 {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .flex_1()
                .w_full()
                .text_color(theme.colors.text_muted)
                .text_size(px(12.0))
                .child("No headers")
                .into_any_element();
        }

        // Fixed row height for consistent virtual list
        let row_height = px(40.0);
        let item_sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            (0..header_count)
                .map(|_| size(px(600.0), row_height))
                .collect(),
        );

        let bg_primary = theme.colors.bg_secondary;
        let bg_alternate = theme.colors.bg_tertiary;
        let border_color = theme.colors.border_primary.opacity(0.3);
        let key_color = theme.colors.text_secondary;
        let value_color = theme.colors.text_primary;

        div()
            .id("headers-virtual-container")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .bg(theme.colors.bg_tertiary)
            .child(
                v_virtual_list(
                    cx.entity().clone(),
                    "response-headers-list",
                    item_sizes.clone(),
                    move |_view, visible_range, _window, _cx| {
                        let headers = headers.clone();
                        visible_range
                            .map(|idx| {
                                let (key, value) = &headers[idx];
                                let bg_color = if idx % 2 == 0 {
                                    bg_primary
                                } else {
                                    bg_alternate
                                };

                                div()
                                    .id(ElementId::from(SharedString::from(format!(
                                        "header-row-{}",
                                        idx
                                    ))))
                                    .w_full()
                                    .h(row_height)
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .px(px(16.0))
                                    .bg(bg_color)
                                    .border_b_1()
                                    .border_color(border_color)
                                    // Key column
                                    .child(
                                        div()
                                            .w(px(180.0))
                                            .min_w(px(180.0))
                                            .pr(px(12.0))
                                            .text_color(key_color)
                                            .text_size(px(12.0))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(key.clone()),
                                    )
                                    // Value column
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_color(value_color)
                                            .text_size(px(12.0))
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(value.clone()),
                                    )
                            })
                            .collect()
                    },
                )
                .flex_1()
                .track_scroll(&self.headers_scroll_handle),
            )
            // Scrollbar overlay
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(px(8.0))
                    .child(Scrollbar::vertical(&self.headers_scroll_handle)),
            )
            .into_any_element()
    }
}
