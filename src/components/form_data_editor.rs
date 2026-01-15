use gpui::prelude::*;
use gpui::{
    div, px, App, Context, ElementId, Entity, FocusHandle, Focusable, IntoElement, Render,
    SharedString, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::Sizable;

use crate::icons::IconName;
use gpui_component::ActiveTheme;

pub struct FormDataRow {
    pub key_input: Entity<InputState>,
    pub value_input: Entity<InputState>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct FormDataField {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Clone)]
struct DraggedRow {
    index: usize,
    key: String,
    value: String,
}

pub struct FormDataEditor {
    rows: Vec<FormDataRow>,
    focus_handle: FocusHandle,
}

#[allow(dead_code)]
impl FormDataEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            rows: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key_input = cx.new(|cx| InputState::new(window, cx).placeholder("Key"));
        let value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Value"));

        self.rows.push(FormDataRow {
            key_input,
            value_input,
            enabled: true,
        });

        cx.notify();
    }

    pub fn remove_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.rows.len() {
            self.rows.remove(index);
            cx.notify();
        }
    }

    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        self.rows.clear();
        cx.notify();
    }

    pub fn toggle_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get_mut(index) {
            row.enabled = !row.enabled;
            cx.notify();
        }
    }

    pub fn move_row(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from == to || from >= self.rows.len() || to >= self.rows.len() {
            return;
        }
        let row = self.rows.remove(from);
        self.rows.insert(to, row);
        cx.notify();
    }

    pub fn get_fields(&self, cx: &App) -> Vec<FormDataField> {
        self.rows
            .iter()
            .map(|row| FormDataField {
                key: row.key_input.read(cx).text().to_string(),
                value: row.value_input.read(cx).text().to_string(),
                enabled: row.enabled,
            })
            .collect()
    }

    pub fn build_encoded_string(&self, cx: &App) -> String {
        self.get_fields(cx)
            .into_iter()
            .filter(|f| f.enabled && !f.key.is_empty())
            .map(|f| {
                format!(
                    "{}={}",
                    urlencoding::encode(&f.key),
                    urlencoding::encode(&f.value)
                )
            })
            .collect::<Vec<_>>()
            .join("&")
    }

    pub fn get_form_data(&self, cx: &App) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        for field in self.get_fields(cx) {
            if field.enabled && !field.key.is_empty() {
                map.insert(field.key, field.value);
            }
        }
        map
    }

    pub fn set_from_string(&mut self, content: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.rows.clear();

        for pair in content.split('&') {
            if pair.is_empty() {
                continue;
            }

            let (key, value) = if let Some((k, v)) = pair.split_once('=') {
                (
                    urlencoding::decode(k).unwrap_or_default().to_string(),
                    urlencoding::decode(v).unwrap_or_default().to_string(),
                )
            } else {
                (
                    urlencoding::decode(pair).unwrap_or_default().to_string(),
                    String::new(),
                )
            };

            let key_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Key")
                    .default_value(&key)
            });
            let value_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Value")
                    .default_value(&value)
            });

            self.rows.push(FormDataRow {
                key_input,
                value_input,
                enabled: true,
            });
        }

        cx.notify();
    }
}

impl Focusable for FormDataEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FormDataEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let this = cx.entity().clone();

        div()
            .id("form-data-editor-container")
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
                            .child("Form URL Encoded"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                Button::new("clear-all-form-btn")
                                    .icon(IconName::Trash)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Clear All")
                                    .on_click({
                                        let this = this.clone();
                                        move |_, _, cx| {
                                            this.update(cx, |editor, cx| {
                                                editor.clear_all(cx);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("add-form-row-btn")
                                    .icon(IconName::Plus)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Add Field")
                                    .on_click({
                                        let this = this.clone();
                                        move |_, window, cx| {
                                            this.update(cx, |editor, cx| {
                                                editor.add_row(window, cx);
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
                            .flex_1()
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
                    .child(div().w(px(40.0))),
            )
            .child(
                div()
                    .id("form-data-rows-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .children(self.rows.iter().enumerate().map(|(idx, row)| {
                        let this_toggle = this.clone();
                        let this_remove = this.clone();
                        let this_drop = this.clone();
                        let enabled = row.enabled;

                        div()
                            .id(ElementId::from(SharedString::from(format!(
                                "form-row-{}",
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
                            .on_drop(move |dragged: &DraggedRow, _, cx| {
                                this_drop.update(cx, |editor, cx| {
                                    editor.move_row(dragged.index, idx, cx);
                                });
                            })
                            .drag_over::<DraggedRow>(|style, _, _, cx| {
                                let theme = cx.theme();
                                style.bg(theme.primary.opacity(0.15))
                            })
                            .child(
                                div()
                                    .id(ElementId::from(SharedString::from(format!(
                                        "drag-handle-{}",
                                        idx
                                    ))))
                                    .w(px(24.0))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_grab()
                                    .on_drag(
                                        DraggedRow {
                                            index: idx,
                                            key: row.key_input.read(cx).text().to_string(),
                                            value: row.value_input.read(cx).text().to_string(),
                                        },
                                        |dragged, _, _, cx| {
                                            cx.new(|_| DragPreview {
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
                                            "form-checkbox-{}",
                                            idx
                                        )))
                                        .checked(enabled)
                                        .on_click(
                                            move |_checked, _, cx| {
                                                this_toggle.update(cx, |editor, cx| {
                                                    editor.toggle_row(idx, cx);
                                                });
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
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
                                div()
                                    .w(px(40.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "remove-form-row-{}",
                                            idx
                                        )))
                                        .icon(IconName::Trash)
                                        .ghost()
                                        .xsmall()
                                        .tooltip("Remove")
                                        .on_click(
                                            move |_, _, cx| {
                                                this_remove.update(cx, |editor, cx| {
                                                    editor.remove_row(idx, cx);
                                                });
                                            },
                                        ),
                                    ),
                            )
                    }))
                    .when(self.rows.is_empty(), |el| {
                        el.child(
                            div()
                                .flex()
                                .flex_1()
                                .items_center()
                                .justify_center()
                                .py(px(40.0))
                                .text_color(theme.muted_foreground.opacity(0.5))
                                .text_size(px(12.0))
                                .child("No form fields. Click + to add one."),
                        )
                    }),
            )
    }
}

struct DragPreview {
    key: String,
    value: String,
}

impl Render for DragPreview {
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
                            .child("Key")
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
