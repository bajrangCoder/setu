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

    /// Toggle param enabled state
    pub fn toggle_param(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.param_rows.get_mut(index) {
            row.enabled = !row.enabled;
            cx.notify();
        }
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
                            .child("Query Parameters"),
                    )
                    .child(
                        div().flex().flex_row().items_center().gap(px(4.0)).child(
                            Button::new("add-param-btn")
                                .icon(IconName::Plus)
                                .ghost()
                                .xsmall()
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
                    .child(div().w(px(32.0)))
                    .child(
                        div()
                            .w(px(180.0))
                            .min_w(px(180.0))
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
                            .w(px(180.0))
                            .min_w(px(180.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Description"),
                    )
                    .child(div().w(px(60.0))),
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
                                    .w(px(180.0))
                                    .min_w(px(180.0))
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
                                div().w(px(180.0)).min_w(px(180.0)).pr(px(8.0)).child(
                                    Input::new(&row.description_input)
                                        .appearance(false)
                                        .xsmall(),
                                ),
                            )
                            .child(
                                div()
                                    .w(px(60.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    .gap(px(2.0))
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "remove-param-{}",
                                            idx
                                        )))
                                        .icon(IconName::Trash)
                                        .ghost()
                                        .xsmall()
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
