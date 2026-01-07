use gpui::prelude::*;
use gpui::{
    div, px, App, Context, ElementId, Entity, FocusHandle, Focusable, IntoElement, Render,
    SharedString, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::Sizable;

use crate::entities::{Header, RequestEntity};
use crate::icons::IconName;
use gpui_component::ActiveTheme;

pub struct HeaderRow {
    pub key_input: Entity<InputState>,
    pub value_input: Entity<InputState>,
    pub description_input: Entity<InputState>,
    pub enabled: bool,
}

/// Header editor
pub struct HeaderEditor {
    request: Entity<RequestEntity>,
    header_rows: Vec<HeaderRow>,
    focus_handle: FocusHandle,
}

impl HeaderEditor {
    pub fn new(request: Entity<RequestEntity>, cx: &mut Context<Self>) -> Self {
        // Subscribe to request changes
        cx.subscribe(&request, |this, _request, _event, cx| {
            this.sync_headers_from_request(cx);
            cx.notify();
        })
        .detach();

        let mut editor = Self {
            request,
            header_rows: Vec::new(),
            focus_handle: cx.focus_handle(),
        };

        // Initial sync
        editor.sync_headers_from_request(cx);

        editor
    }

    /// Sync header rows from request entity
    fn sync_headers_from_request(&mut self, cx: &mut Context<Self>) {
        // Only sync if the number of headers changed
        let request_headers = self.request.read(cx).headers();
        if request_headers.len() == self.header_rows.len() {
            return;
        }
    }

    /// Add a new empty header row
    pub fn add_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key_input = cx.new(|cx| InputState::new(window, cx).placeholder("Header name"));
        let value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Value"));
        let description_input = cx.new(|cx| InputState::new(window, cx).placeholder("Description"));

        self.header_rows.push(HeaderRow {
            key_input,
            value_input,
            description_input,
            enabled: true,
        });

        cx.notify();
    }

    /// Add a header with pre-filled key and value
    pub fn add_header_with_value(
        &mut self,
        key: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key_owned = key.to_string();
        let value_owned = value.to_string();

        let key_for_input = key_owned.clone();
        let value_for_input = value_owned.clone();

        let key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Header name")
                .default_value(&key_for_input)
        });
        let value_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Value")
                .default_value(&value_for_input)
        });
        let description_input = cx.new(|cx| InputState::new(window, cx).placeholder("Description"));

        self.header_rows.push(HeaderRow {
            key_input,
            value_input,
            description_input,
            enabled: true,
        });

        // Also add to request entity
        let key_for_req = key_owned.clone();
        let value_for_req = value_owned.clone();
        self.request.update(cx, |req, cx| {
            req.add_header(Header::new(&key_for_req, &value_for_req), cx);
        });

        cx.notify();
    }

    /// Set or update a header - if key exists, update value; otherwise add new
    pub fn set_or_update_header(
        &mut self,
        key: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Check if header with this key already exists
        for row in &self.header_rows {
            let existing_key = row.key_input.read(cx).text().to_string();
            if existing_key.to_lowercase() == key.to_lowercase() {
                // Update existing header value
                let val = value.to_string();
                row.value_input.update(cx, |state, cx| {
                    state.set_value(val, window, cx);
                });
                cx.notify();
                return;
            }
        }

        // Key doesn't exist, add new header
        self.add_header_with_value(key, value, window, cx);
    }

    /// Remove a header row by index
    pub fn remove_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.header_rows.len() {
            self.header_rows.remove(index);

            // Also remove from request entity
            self.request.update(cx, |req, cx| {
                req.remove_header(index, cx);
            });

            cx.notify();
        }
    }

    /// Toggle header enabled state
    pub fn toggle_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.header_rows.get_mut(index) {
            row.enabled = !row.enabled;
            cx.notify();
        }
    }

    /// Get all headers as Header structs
    pub fn get_headers(&self, cx: &App) -> Vec<Header> {
        self.header_rows
            .iter()
            .map(|row| Header {
                key: row.key_input.read(cx).text().to_string(),
                value: row.value_input.read(cx).text().to_string(),
                enabled: row.enabled,
            })
            .filter(|h| !h.key.is_empty())
            .collect()
    }
}

impl Focusable for HeaderEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HeaderEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let this = cx.entity().clone();

        div()
            .id("header-editor-container")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .bg(theme.muted)
            // Header with title and actions
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(36.0))
                    .px(px(16.0))
                    .bg(theme.secondary)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Header List"),
                    )
                    .child(
                        div().flex().flex_row().items_center().gap(px(4.0)).child(
                            Button::new("add-header-btn")
                                .icon(IconName::Plus)
                                .ghost()
                                .xsmall()
                                .on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        this.update(cx, |editor, cx| {
                                            editor.add_header(window, cx);
                                        });
                                    }
                                }),
                        ),
                    ),
            )
            // Table header
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(28.0))
                    .px(px(16.0))
                    .border_b_1()
                    .border_color(theme.border.opacity(0.5))
                    .bg(theme.secondary.opacity(0.5))
                    // Checkbox column
                    .child(div().w(px(32.0)))
                    // Key column header
                    .child(
                        div()
                            .w(px(180.0))
                            .min_w(px(180.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Key"),
                    )
                    // Value column header
                    .child(
                        div()
                            .flex_1()
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Value"),
                    )
                    // Description column header
                    .child(
                        div()
                            .w(px(180.0))
                            .min_w(px(180.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Description"),
                    )
                    // Actions column
                    .child(div().w(px(60.0))),
            )
            // Scrollable content
            .child(
                div()
                    .id("header-rows-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    // Header rows
                    .children(self.header_rows.iter().enumerate().map(|(idx, row)| {
                        let this_toggle = this.clone();
                        let this_remove = this.clone();
                        let enabled = row.enabled;

                        div()
                            .id(ElementId::from(SharedString::from(format!(
                                "header-row-{}",
                                idx
                            ))))
                            .flex()
                            .flex_row()
                            .items_center()
                            .min_h(px(36.0))
                            .px(px(16.0))
                            .border_b_1()
                            .border_color(theme.border.opacity(0.3))
                            .hover(|s| s.bg(theme.secondary.opacity(0.3)))
                            .when(!enabled, |el| el.opacity(0.5))
                            .child(
                                div()
                                    .w(px(32.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Checkbox::new(SharedString::from(format!(
                                            "header-checkbox-{}",
                                            idx
                                        )))
                                        .checked(enabled)
                                        .on_click(
                                            move |_checked, _, cx| {
                                                this_toggle.update(cx, |editor, cx| {
                                                    editor.toggle_header(idx, cx);
                                                });
                                            },
                                        ),
                                    ),
                            )
                            // Key input
                            .child(
                                div()
                                    .w(px(180.0))
                                    .min_w(px(180.0))
                                    .pr(px(8.0))
                                    .child(Input::new(&row.key_input).appearance(false).xsmall()),
                            )
                            // Value input
                            .child(
                                div()
                                    .flex_1()
                                    .pr(px(8.0))
                                    .child(Input::new(&row.value_input).appearance(false).xsmall()),
                            )
                            // Description input
                            .child(
                                div().w(px(180.0)).min_w(px(180.0)).pr(px(8.0)).child(
                                    Input::new(&row.description_input)
                                        .appearance(false)
                                        .xsmall(),
                                ),
                            )
                            // Actions
                            .child(
                                div()
                                    .w(px(60.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    .gap(px(2.0))
                                    // Delete button
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "remove-header-{}",
                                            idx
                                        )))
                                        .icon(IconName::Trash)
                                        .ghost()
                                        .xsmall()
                                        .on_click(
                                            move |_, _, cx| {
                                                this_remove.update(cx, |editor, cx| {
                                                    editor.remove_header(idx, cx);
                                                });
                                            },
                                        ),
                                    ),
                            )
                    }))
                    // Empty state
                    .when(self.header_rows.is_empty(), |el| {
                        el.child(
                            div()
                                .flex()
                                .flex_1()
                                .items_center()
                                .justify_center()
                                .py(px(40.0))
                                .text_color(theme.muted_foreground.opacity(0.5))
                                .text_size(px(12.0))
                                .child("No headers. Click + to add one."),
                        )
                    }),
            )
    }
}
