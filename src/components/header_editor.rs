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

#[derive(Clone)]
struct DraggedHeader {
    index: usize,
    key: String,
    value: String,
}

/// Header editor
pub struct HeaderEditor {
    request: Entity<RequestEntity>,
    header_rows: Vec<HeaderRow>,
    pending_initial_headers: Option<Vec<(String, String, bool)>>,
    focus_handle: FocusHandle,
}

impl HeaderEditor {
    pub fn new(request: Entity<RequestEntity>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&request, |this, _request, _event, cx| {
            this.sync_headers_from_request(cx);
            cx.notify();
        })
        .detach();

        let mut editor = Self {
            request,
            header_rows: Vec::new(),
            pending_initial_headers: None,
            focus_handle: cx.focus_handle(),
        };

        editor.sync_headers_from_request(cx);

        editor
    }

    fn sync_headers_from_request(&mut self, cx: &mut Context<Self>) {
        if !self.header_rows.is_empty() {
            return;
        }

        let headers: Vec<(String, String, bool)> = self
            .request
            .read(cx)
            .headers()
            .iter()
            .map(|h| (h.key.clone(), h.value.clone(), h.enabled))
            .collect();

        if headers.is_empty() {
            return;
        }

        self.pending_initial_headers = Some(headers);
    }

    pub fn populate_pending_headers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(headers) = self.pending_initial_headers.take() else {
            return;
        };

        for (key, value, enabled) in headers {
            let key_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Header name")
                    .default_value(&key)
            });
            let value_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Value")
                    .default_value(&value)
            });
            let description_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Description"));

            self.header_rows.push(HeaderRow {
                key_input,
                value_input,
                description_input,
                enabled,
            });
        }

        cx.notify();
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

    /// Clear all header rows
    pub fn clear_all_headers(&mut self, cx: &mut Context<Self>) {
        self.header_rows.clear();

        // Also clear from request entity
        self.request.update(cx, |req, cx| {
            req.clear_headers(cx);
        });

        cx.notify();
    }

    /// Toggle header enabled state
    pub fn toggle_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.header_rows.get_mut(index) {
            row.enabled = !row.enabled;
            cx.notify();
        }
    }

    pub fn move_header(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from == to || from >= self.header_rows.len() || to >= self.header_rows.len() {
            return;
        }
        let row = self.header_rows.remove(from);
        self.header_rows.insert(to, row);
        cx.notify();
    }

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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.populate_pending_headers(window, cx);

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
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                Button::new("clear-all-headers-btn")
                                    .icon(IconName::Trash)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Clear All")
                                    .on_click({
                                        let this = this.clone();
                                        move |_, _, cx| {
                                            this.update(cx, |editor, cx| {
                                                editor.clear_all_headers(cx);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("add-header-btn")
                                    .icon(IconName::Plus)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Add New")
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
                    .child(div().w(px(24.0)))
                    .child(div().w(px(32.0)))
                    .child(
                        div()
                            .w(px(150.0))
                            .min_w(px(150.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Key"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Value"),
                    )
                    .child(
                        div()
                            .w(px(150.0))
                            .min_w(px(150.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Description"),
                    )
                    .child(div().w(px(40.0))),
            )
            .child(
                div()
                    .id("header-rows-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .children(self.header_rows.iter().enumerate().map(|(idx, row)| {
                        let this_toggle = this.clone();
                        let this_remove = this.clone();
                        let this_drop = this.clone();
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
                            .on_drop(move |dragged: &DraggedHeader, _, cx| {
                                this_drop.update(cx, |editor, cx| {
                                    editor.move_header(dragged.index, idx, cx);
                                });
                            })
                            .drag_over::<DraggedHeader>(|style, _, _, cx| {
                                let theme = cx.theme();
                                style.bg(theme.primary.opacity(0.15))
                            })
                            .child(
                                div()
                                    .id(ElementId::from(SharedString::from(format!(
                                        "header-drag-handle-{}",
                                        idx
                                    ))))
                                    .w(px(24.0))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_grab()
                                    .on_drag(
                                        DraggedHeader {
                                            index: idx,
                                            key: row.key_input.read(cx).text().to_string(),
                                            value: row.value_input.read(cx).text().to_string(),
                                        },
                                        |dragged, _, _, cx| {
                                            cx.new(|_| HeaderDragPreview {
                                                key: dragged.key.clone(),
                                                value: dragged.value.clone(),
                                            })
                                        },
                                    )
                                    .child(
                                        gpui_component::Icon::new(IconName::GripVertical)
                                            .size(px(14.0))
                                            .text_color(theme.muted_foreground.opacity(0.5)),
                                    ),
                            )
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
                            .child(
                                div()
                                    .w(px(150.0))
                                    .min_w(px(150.0))
                                    .pr(px(8.0))
                                    .child(Input::new(&row.key_input).appearance(false).xsmall()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .pr(px(8.0))
                                    .child(Input::new(&row.value_input).appearance(false).xsmall()),
                            )
                            .child(
                                div().w(px(150.0)).min_w(px(150.0)).pr(px(8.0)).child(
                                    Input::new(&row.description_input)
                                        .appearance(false)
                                        .xsmall(),
                                ),
                            )
                            .child(
                                div()
                                    .w(px(40.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "remove-header-{}",
                                            idx
                                        )))
                                        .icon(IconName::Trash)
                                        .ghost()
                                        .xsmall()
                                        .tooltip("Remove")
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

struct HeaderDragPreview {
    key: String,
    value: String,
}

impl Render for HeaderDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w(px(400.0))
            .flex()
            .flex_row()
            .items_center()
            .h(px(36.0))
            .px(px(16.0))
            .bg(theme.background.opacity(0.95))
            .border_1()
            .border_color(theme.primary.opacity(0.5))
            .rounded(px(6.0))
            .shadow_lg()
            .opacity(0.9)
            .child(
                div()
                    .w(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        gpui_component::Icon::new(IconName::GripVertical)
                            .size(px(14.0))
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(
                div()
                    .w(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .rounded(px(3.0))
                            .border_1()
                            .border_color(theme.primary)
                            .bg(theme.primary.opacity(0.2)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .px(px(8.0))
                    .text_color(theme.foreground)
                    .text_size(px(12.0))
                    .overflow_hidden()
                    .child(if self.key.is_empty() {
                        div()
                            .text_color(theme.muted_foreground.opacity(0.5))
                            .child("Header name")
                    } else {
                        div().child(self.key.clone())
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .px(px(8.0))
                    .text_color(theme.foreground)
                    .text_size(px(12.0))
                    .overflow_hidden()
                    .child(if self.value.is_empty() {
                        div()
                            .text_color(theme.muted_foreground.opacity(0.5))
                            .child("Value")
                    } else {
                        div().child(self.value.clone())
                    }),
            )
            .child(
                div()
                    .w(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        gpui_component::Icon::new(IconName::Trash)
                            .size(px(14.0))
                            .text_color(theme.muted_foreground.opacity(0.5)),
                    ),
            )
    }
}
