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

pub struct ParamRow {
    pub key_input: Entity<InputState>,
    pub value_input: Entity<InputState>,
    pub description_input: Entity<InputState>,
    pub enabled: bool,
}

/// Query parameter
#[derive(Debug, Clone)]
pub struct QueryParam {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Clone)]
struct DraggedParam {
    index: usize,
    key: String,
    value: String,
}

/// Params editor
pub struct ParamsEditor {
    param_rows: Vec<ParamRow>,
    focus_handle: FocusHandle,
}

impl ParamsEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            param_rows: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Add a new empty param row
    pub fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key_input = cx.new(|cx| InputState::new(window, cx).placeholder("Param name"));
        let value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Value"));
        let description_input = cx.new(|cx| InputState::new(window, cx).placeholder("Description"));

        self.param_rows.push(ParamRow {
            key_input,
            value_input,
            description_input,
            enabled: true,
        });

        cx.notify();
    }

    /// Remove a param row by index
    pub fn remove_param(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.param_rows.len() {
            self.param_rows.remove(index);
            cx.notify();
        }
    }

    /// Clear all param rows
    pub fn clear_all_params(&mut self, cx: &mut Context<Self>) {
        self.param_rows.clear();
        cx.notify();
    }

    /// Toggle param enabled state
    pub fn toggle_param(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.param_rows.get_mut(index) {
            row.enabled = !row.enabled;
            cx.notify();
        }
    }

    /// Move param from one index to another
    pub fn move_param(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from == to || from >= self.param_rows.len() || to >= self.param_rows.len() {
            return;
        }
        let row = self.param_rows.remove(from);
        self.param_rows.insert(to, row);
        cx.notify();
    }

    /// Get all params
    pub fn get_params(&self, cx: &App) -> Vec<QueryParam> {
        self.param_rows
            .iter()
            .map(|row| QueryParam {
                key: row.key_input.read(cx).text().to_string(),
                value: row.value_input.read(cx).text().to_string(),
                enabled: row.enabled,
            })
            .collect()
    }

    /// Build query string from params
    pub fn build_query_string(&self, cx: &App) -> String {
        let params: Vec<String> = self
            .get_params(cx)
            .into_iter()
            .filter(|p| p.enabled && !p.key.is_empty())
            .map(|p| {
                if p.value.is_empty() {
                    p.key
                } else {
                    format!("{}={}", p.key, p.value)
                }
            })
            .collect();

        if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        }
    }
}

impl Focusable for ParamsEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ParamsEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let this = cx.entity().clone();

        div()
            .id("params-editor-container")
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
                            .child("Query Parameters"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                Button::new("clear-all-params-btn")
                                    .icon(IconName::Trash)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Clear All")
                                    .on_click({
                                        let this = this.clone();
                                        move |_, _, cx| {
                                            this.update(cx, |editor, cx| {
                                                editor.clear_all_params(cx);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("add-param-btn")
                                    .icon(IconName::Plus)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Add New")
                                    .on_click({
                                        let this = this.clone();
                                        move |_, window, cx| {
                                            this.update(cx, |editor, cx| {
                                                editor.add_param(window, cx);
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
                    .id("param-rows-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .children(self.param_rows.iter().enumerate().map(|(idx, row)| {
                        let this_toggle = this.clone();
                        let this_remove = this.clone();
                        let this_drop = this.clone();
                        let enabled = row.enabled;

                        div()
                            .id(ElementId::from(SharedString::from(format!(
                                "param-row-{}",
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
                            .on_drop(move |dragged: &DraggedParam, _, cx| {
                                this_drop.update(cx, |editor, cx| {
                                    editor.move_param(dragged.index, idx, cx);
                                });
                            })
                            .drag_over::<DraggedParam>(|style, _, _, cx| {
                                let theme = cx.theme();
                                style.bg(theme.primary.opacity(0.15))
                            })
                            .child(
                                div()
                                    .id(ElementId::from(SharedString::from(format!(
                                        "param-drag-handle-{}",
                                        idx
                                    ))))
                                    .w(px(24.0))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_grab()
                                    .on_drag(
                                        DraggedParam {
                                            index: idx,
                                            key: row.key_input.read(cx).text().to_string(),
                                            value: row.value_input.read(cx).text().to_string(),
                                        },
                                        |dragged, _, _, cx| {
                                            cx.new(|_| ParamDragPreview {
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
                                            "param-checkbox-{}",
                                            idx
                                        )))
                                        .checked(enabled)
                                        .on_click(
                                            move |_checked, _, cx| {
                                                this_toggle.update(cx, |editor, cx| {
                                                    editor.toggle_param(idx, cx);
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
                                            "remove-param-{}",
                                            idx
                                        )))
                                        .icon(IconName::Trash)
                                        .ghost()
                                        .xsmall()
                                        .tooltip("Remove")
                                        .on_click(
                                            move |_, _, cx| {
                                                this_remove.update(cx, |editor, cx| {
                                                    editor.remove_param(idx, cx);
                                                });
                                            },
                                        ),
                                    ),
                            )
                    }))
                    .when(self.param_rows.is_empty(), |el| {
                        el.child(
                            div()
                                .flex()
                                .flex_1()
                                .items_center()
                                .justify_center()
                                .py(px(40.0))
                                .text_color(theme.muted_foreground.opacity(0.5))
                                .text_size(px(12.0))
                                .child("No parameters. Click + to add one."),
                        )
                    }),
            )
    }
}

struct ParamDragPreview {
    key: String,
    value: String,
}

impl Render for ParamDragPreview {
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
