use gpui::prelude::*;
use gpui::{
    div, px, App, Context, ElementId, Entity, FocusHandle, Focusable, IntoElement,
    PathPromptOptions, Render, SharedString, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::Sizable;
use std::path::PathBuf;

use crate::icons::IconName;
use gpui_component::ActiveTheme;

pub struct MultipartFormRow {
    pub key_input: Entity<InputState>,
    pub value_input: Entity<InputState>,
    pub file_path: Option<PathBuf>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct MultipartFormField {
    pub key: String,
    pub value: String,
    pub file_path: Option<PathBuf>,
    pub enabled: bool,
}

#[derive(Clone)]
struct DraggedMultipartRow {
    index: usize,
    key: String,
    value: String,
    is_file: bool,
}

pub struct MultipartFormDataEditor {
    rows: Vec<MultipartFormRow>,
    focus_handle: FocusHandle,
}

#[allow(dead_code)]
impl MultipartFormDataEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            rows: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key_input = cx.new(|cx| InputState::new(window, cx).placeholder("Key"));
        let value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Value"));

        self.rows.push(MultipartFormRow {
            key_input,
            value_input,
            file_path: None,
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

    pub fn set_file_for_row(
        &mut self,
        index: usize,
        path: PathBuf,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(row) = self.rows.get_mut(index) {
            row.file_path = Some(path);
            cx.notify();
        }
    }

    pub fn clear_file_for_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get_mut(index) {
            row.file_path = None;
            cx.notify();
        }
    }

    pub fn get_fields(&self, cx: &App) -> Vec<MultipartFormField> {
        self.rows
            .iter()
            .map(|row| MultipartFormField {
                key: row.key_input.read(cx).text().to_string(),
                value: row.value_input.read(cx).text().to_string(),
                file_path: row.file_path.clone(),
                enabled: row.enabled,
            })
            .collect()
    }

    pub fn get_form_data(&self, cx: &App) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        for field in self.get_fields(cx) {
            if field.enabled && !field.key.is_empty() {
                if let Some(path) = field.file_path {
                    map.insert(field.key, format!("@{}", path.display()));
                } else {
                    map.insert(field.key, field.value);
                }
            }
        }
        map
    }

    pub fn get_multipart_fields(&self, cx: &App) -> Vec<crate::entities::MultipartField> {
        self.get_fields(cx)
            .into_iter()
            .filter(|f| f.enabled && !f.key.is_empty())
            .map(|f| {
                if let Some(path) = f.file_path {
                    crate::entities::MultipartField {
                        key: f.key,
                        value: String::new(),
                        file_path: Some(path.to_string_lossy().to_string()),
                    }
                } else {
                    crate::entities::MultipartField {
                        key: f.key,
                        value: f.value,
                        file_path: None,
                    }
                }
            })
            .collect()
    }

    pub fn set_from_multipart_fields(
        &mut self,
        fields: &[crate::entities::MultipartField],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rows.clear();

        for field in fields {
            let key_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Key")
                    .default_value(&field.key)
            });
            let value_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Value")
                    .default_value(&field.value)
            });

            let file_path = field.file_path.as_ref().map(PathBuf::from);

            self.rows.push(MultipartFormRow {
                key_input,
                value_input,
                file_path,
                enabled: true,
            });
        }

        cx.notify();
    }

    fn pick_file_for_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().clone();

        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select file to upload".into()),
        };

        let paths_receiver = cx.prompt_for_paths(options);

        cx.spawn_in(window, async move |_weak_this, cx| {
            let channel_result = paths_receiver.await;

            let Ok(platform_result) = channel_result else {
                log::error!("File picker channel closed unexpectedly");
                return;
            };

            let Ok(paths_opt) = platform_result else {
                log::error!("File picker failed");
                return;
            };

            let Some(paths) = paths_opt else {
                return;
            };

            let Some(path) = paths.first().cloned() else {
                return;
            };

            let _ = cx.update(|window, app| {
                this.update(app, |editor, cx| {
                    editor.set_file_for_row(index, path, window, cx);
                });
            });
        })
        .detach();
    }
}

impl Focusable for MultipartFormDataEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MultipartFormDataEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let this = cx.entity().clone();

        div()
            .id("multipart-form-data-editor-container")
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
                            .child("Form Data (Multipart)"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                Button::new("clear-all-multipart-btn")
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
                                Button::new("add-multipart-row-btn")
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
                    .child(div().w(px(24.0)).flex_shrink_0())
                    .child(div().w(px(32.0)).flex_shrink_0())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(80.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Key"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(100.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Value"),
                    )
                    .child(div().w(px(70.0)).flex_shrink_0())
                    .child(div().w(px(28.0)).flex_shrink_0()),
            )
            .child(
                div()
                    .id("multipart-form-rows-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .children(self.rows.iter().enumerate().map(|(idx, row)| {
                        let this_toggle = this.clone();
                        let this_remove = this.clone();
                        let this_drop = this.clone();
                        let this_pick_file = this.clone();
                        let this_clear_file = this.clone();
                        let enabled = row.enabled;
                        let has_file = row.file_path.is_some();
                        let file_name = row
                            .file_path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string());

                        div()
                            .id(ElementId::from(SharedString::from(format!(
                                "multipart-row-{}",
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
                            .on_drop(move |dragged: &DraggedMultipartRow, _, cx| {
                                this_drop.update(cx, |editor, cx| {
                                    editor.move_row(dragged.index, idx, cx);
                                });
                            })
                            .drag_over::<DraggedMultipartRow>(|style, _, _, cx| {
                                let theme = cx.theme();
                                style.bg(theme.primary.opacity(0.15))
                            })
                            .child(
                                div()
                                    .id(ElementId::from(SharedString::from(format!(
                                        "multipart-drag-handle-{}",
                                        idx
                                    ))))
                                    .w(px(24.0))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_grab()
                                    .on_drag(
                                        DraggedMultipartRow {
                                            index: idx,
                                            key: row.key_input.read(cx).text().to_string(),
                                            value: row.value_input.read(cx).text().to_string(),
                                            is_file: has_file,
                                        },
                                        |dragged, _, _, cx| {
                                            cx.new(|_| MultipartDragPreview {
                                                key: dragged.key.clone(),
                                                value: dragged.value.clone(),
                                                is_file: dragged.is_file,
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
                                            "multipart-checkbox-{}",
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
                                    .min_w(px(80.0))
                                    .pr(px(8.0))
                                    .child(Input::new(&row.key_input).appearance(false).xsmall()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(100.0))
                                    .pr(px(8.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .when(!has_file, |el| {
                                        el.child(
                                            Input::new(&row.value_input).appearance(false).xsmall(),
                                        )
                                    })
                                    .when(has_file, |el| {
                                        el.child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(6.0))
                                                .px(px(6.0))
                                                .py(px(4.0))
                                                .rounded(px(4.0))
                                                .bg(theme.secondary.opacity(0.5))
                                                .overflow_hidden()
                                                .child(
                                                    gpui_component::Icon::new(IconName::FilePen)
                                                        .size(px(12.0))
                                                        .text_color(theme.muted_foreground),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_size(px(12.0))
                                                        .text_color(theme.foreground)
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(
                                                            file_name.clone().unwrap_or_default(),
                                                        ),
                                                )
                                                .child(
                                                    Button::new(SharedString::from(format!(
                                                        "clear-file-{}",
                                                        idx
                                                    )))
                                                    .icon(IconName::CircleX)
                                                    .ghost()
                                                    .xsmall()
                                                    .tooltip("Remove file")
                                                    .on_click(move |_, _, cx| {
                                                        this_clear_file.update(cx, |editor, cx| {
                                                            editor.clear_file_for_row(idx, cx);
                                                        });
                                                    }),
                                                ),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .w(px(70.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "browse-file-{}",
                                            idx
                                        )))
                                        .label(if has_file { "Change" } else { "Browse" })
                                        .ghost()
                                        .xsmall()
                                        .on_click(
                                            move |_, window, cx| {
                                                this_pick_file.update(cx, |editor, cx| {
                                                    editor.pick_file_for_row(idx, window, cx);
                                                });
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(28.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "remove-multipart-row-{}",
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

struct MultipartDragPreview {
    key: String,
    value: String,
    is_file: bool,
}

impl Render for MultipartDragPreview {
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
                            .child(if self.is_file { "File" } else { "Value" })
                    } else {
                        div().child(self.value.clone())
                    }),
            )
            .child(
                div()
                    .w(px(60.0))
                    .px(px(8.0))
                    .text_color(theme.muted_foreground)
                    .text_size(px(10.0))
                    .child(if self.is_file { "File" } else { "Text" }),
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
