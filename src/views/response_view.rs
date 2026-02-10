use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    div, img, px, size, AnyElement, App, Context, ElementId, Entity, FocusHandle, Focusable, Image,
    ImageFormat, IntoElement, PathPromptOptions, Pixels, Render, SharedString, Size, Styled,
    Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::notification::NotificationType;
use gpui_component::scroll::Scrollbar;
use gpui_component::spinner::Spinner;
use gpui_component::v_virtual_list;
use gpui_component::Selectable;
use gpui_component::Sizable;
use gpui_component::VirtualListScrollHandle;
use gpui_component::WindowExt;

use crate::components::StatusBadge;
use crate::entities::{
    ContentCategory, ResponseData, ResponseEntity, ResponseEvent, ResponseState,
};
use crate::icons::IconName;
use gpui_component::ActiveTheme;
use gpui_component::Icon;

/// Active tab in the response panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseTab {
    /// Formatted body based on content-type (JSON formatted, HTML, images, etc.)
    #[default]
    Body,
    /// Raw response body as-is
    Raw,
    /// Response headers
    Headers,
}

/// Response view
pub struct ResponseView {
    pub response: Entity<ResponseEntity>,
    active_tab: ResponseTab,
    /// Body display
    body_display: Option<Entity<InputState>>,
    /// Raw body display (plain text)
    raw_display: Option<Entity<InputState>>,
    /// Hash of last displayed body content for efficient change detection
    last_body_hash: u64,
    /// Last content category for pretty display
    last_content_category: ContentCategory,
    focus_handle: FocusHandle,
    /// Virtual list scroll handle for headers tab
    headers_scroll_handle: VirtualListScrollHandle,
    /// Whether to wrap lines in the editor
    wrap_lines: bool,
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
            raw_display: None,
            last_body_hash: 0,
            last_content_category: ContentCategory::Text,
            focus_handle: cx.focus_handle(),
            headers_scroll_handle: VirtualListScrollHandle::new(),
            wrap_lines: true,
        }
    }

    fn ensure_body_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (current_hash, content_category, formatted_content) = {
            self.response.update(cx, |resp, _cx| {
                if let Some(ref mut data) = resp.data {
                    let formatted = data.formatted_body();
                    (data.body_hash(), data.content_category(), Some(formatted))
                } else {
                    (0, ContentCategory::Text, None)
                }
            })
        };

        let needs_update =
            current_hash != self.last_body_hash || content_category != self.last_content_category;

        if self.body_display.is_none() || needs_update {
            let lang = content_category.language();
            let content = formatted_content
                .map(|s| s.as_ref().clone())
                .unwrap_or_default();

            let wrap_lines = self.wrap_lines;
            let body_display = cx.new(|cx| {
                InputState::new(window, cx)
                    .code_editor(lang)
                    .line_number(true)
                    .searchable(true)
                    .soft_wrap(wrap_lines)
                    .default_value(&content)
            });
            self.body_display = Some(body_display);
            self.last_body_hash = current_hash;
            self.last_content_category = content_category;
        }
    }

    fn ensure_raw_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (current_hash, raw_body) = {
            let resp = self.response.read(cx);
            if let Some(ref data) = resp.data {
                (data.body_hash(), Some(data.body.clone()))
            } else {
                (0, None)
            }
        };

        if self.raw_display.is_none() {
            let content = raw_body.unwrap_or_default();
            let wrap_lines = self.wrap_lines;
            let raw_display = cx.new(|cx| {
                InputState::new(window, cx)
                    .code_editor("text")
                    .line_number(true)
                    .searchable(true)
                    .soft_wrap(wrap_lines)
                    .default_value(&content)
            });
            self.raw_display = Some(raw_display);
            self.last_body_hash = current_hash;
        } else if current_hash != self.last_body_hash {
            if let Some(ref raw_display) = self.raw_display {
                let content = raw_body.unwrap_or_default();
                raw_display.update(cx, |state, cx| {
                    state.set_value(content, window, cx);
                });
                self.last_body_hash = current_hash;
            }
        }
    }

    pub fn set_tab(&mut self, tab: ResponseTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }

    pub fn toggle_wrap_lines(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.wrap_lines = !self.wrap_lines;
        if let Some(ref body_display) = self.body_display {
            body_display.update(cx, |state, cx| {
                state.set_soft_wrap(self.wrap_lines, window, cx);
            });
        }
        if let Some(ref raw_display) = self.raw_display {
            raw_display.update(cx, |state, cx| {
                state.set_soft_wrap(self.wrap_lines, window, cx);
            });
        }
        cx.notify();
    }

    pub fn trigger_search(&mut self, window: &mut Window, _cx: &mut Context<Self>) {
        let editor = match self.active_tab {
            ResponseTab::Body => self.body_display.clone(),
            ResponseTab::Raw => self.raw_display.clone(),
            ResponseTab::Headers => None,
        };
        crate::utils::trigger_editor_search(editor, window);
    }

    fn copy_response(&self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self
            .response
            .read(cx)
            .data
            .as_ref()
            .map(|d| match self.active_tab {
                ResponseTab::Body => d.formatted_body_ref().to_string(),
                ResponseTab::Raw => d.body.clone(),
                ResponseTab::Headers => {
                    let headers_json: Vec<serde_json::Value> = d
                        .headers
                        .iter()
                        .map(|(k, v)| {
                            serde_json::json!({
                                "key": k,
                                "value": v
                            })
                        })
                        .collect();
                    serde_json::to_string(&headers_json).unwrap_or_else(|_| "[]".to_string())
                }
            });

        if let Some(content) = content {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(content));
            window.push_notification(
                (
                    gpui_component::notification::NotificationType::Success,
                    "Response copied to clipboard",
                ),
                cx,
            );
        }
    }

    fn save_to_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self
            .response
            .read(cx)
            .data
            .as_ref()
            .map(|d| match self.active_tab {
                ResponseTab::Body => d.formatted_body_ref().to_string(),
                ResponseTab::Raw => d.body.clone(),
                ResponseTab::Headers => {
                    let headers_json: Vec<serde_json::Value> = d
                        .headers
                        .iter()
                        .map(|(k, v)| {
                            serde_json::json!({
                                "key": k,
                                "value": v
                            })
                        })
                        .collect();
                    serde_json::to_string_pretty(&headers_json).unwrap_or_else(|_| "[]".to_string())
                }
            });

        let Some(content) = content else { return };

        let default_extension = match self.active_tab {
            ResponseTab::Headers => "json",
            _ => {
                let content_category = self
                    .response
                    .read(cx)
                    .data
                    .as_ref()
                    .map(|d| d.content_category())
                    .unwrap_or(ContentCategory::Text);

                match content_category {
                    ContentCategory::Json => "json",
                    ContentCategory::Xml => "xml",
                    ContentCategory::Html => "html",
                    ContentCategory::Image => "png",
                    _ => "txt",
                }
            }
        };

        let default_name = format!("response.{}", default_extension);
        let this = cx.entity().clone();

        let options = PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select folder to save response".into()),
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

            let Some(dir_path) = paths.first() else {
                return;
            };

            let file_path = dir_path.join(&default_name);

            if let Err(e) = std::fs::write(&file_path, &content) {
                log::error!("Failed to save response: {}", e);
                let _ = cx.update(|window, app| {
                    window.push_notification(
                        (NotificationType::Error, "Failed to save response"),
                        app,
                    );
                });
                return;
            }

            log::info!("Response saved to: {}", file_path.display());
            let _ = cx.update(|window, app| {
                window.push_notification(
                    (NotificationType::Success, "Response saved successfully"),
                    app,
                );
                this.update(app, |_view, cx| {
                    cx.notify();
                });
            });
        })
        .detach();
    }
}

impl Focusable for ResponseView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ResponseView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.active_tab {
            ResponseTab::Body => self.ensure_body_display(window, cx),
            ResponseTab::Raw => self.ensure_raw_display(window, cx),
            ResponseTab::Headers => {}
        }

        let theme = cx.theme();

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
        theme: &gpui_component::theme::ThemeColor,
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

    fn render_empty(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(12.0))
                    .child("Response will appear here"),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(11.0))
                    .child("Send a request to get started"),
            )
    }

    fn render_loading(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
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
                    .text_color(theme.muted_foreground)
                    .text_size(px(12.0))
                    .child("Sending request..."),
            )
    }

    fn render_error(
        &self,
        theme: &gpui_component::theme::ThemeColor,
        message: &str,
    ) -> impl IntoElement {
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
                    .text_color(theme.danger)
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Request failed"),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(11.0))
                    .child(message.to_string()),
            )
    }

    fn render_success(
        &self,
        theme: &gpui_component::theme::ThemeColor,
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
                    .border_color(theme.border)
                    // Left: status + meta
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(StatusBadge::new(data.status_code))
                            .child(
                                div()
                                    .text_color(theme.muted_foreground)
                                    .text_size(px(11.0))
                                    .child(data.formatted_duration()),
                            )
                            .child(
                                div()
                                    .text_color(theme.muted_foreground)
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

    fn render_tabs(
        &self,
        _theme: &gpui_component::theme::ThemeColor,
        this: Entity<ResponseView>,
    ) -> impl IntoElement {
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
                PanelTab::new("Raw")
                    .active(self.active_tab == ResponseTab::Raw)
                    .on_click({
                        let this = this.clone();
                        move |_event, _window, cx| {
                            this.update(cx, |view, cx| view.set_tab(ResponseTab::Raw, cx));
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
        theme: &gpui_component::theme::ThemeColor,
        data: &ResponseData,
        cx: &Context<Self>,
    ) -> AnyElement {
        let this = cx.entity().clone();
        let this_save = cx.entity().clone();
        let this_wrap = cx.entity().clone();
        let this_search = cx.entity().clone();

        let tab_label = match self.active_tab {
            ResponseTab::Body => "Response Body",
            ResponseTab::Raw => "Raw Response",
            ResponseTab::Headers => "Headers List",
        };

        let wrap_lines = self.wrap_lines;
        let show_wrap_toggle = self.active_tab != ResponseTab::Headers;

        div()
            .id("response-tab-content")
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .child(
                div()
                    .id("response-tools-bar")
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(12.0))
                    .py(px(6.0))
                    .bg(theme.secondary)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(tab_label),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .when(show_wrap_toggle, |el| {
                                el.child(
                                    Button::new("toggle-wrap-lines")
                                        .icon(Icon::new(IconName::TextWrap).size(px(14.0)))
                                        .ghost()
                                        .xsmall()
                                        .tooltip(if wrap_lines {
                                            "Disable word wrap"
                                        } else {
                                            "Enable word wrap"
                                        })
                                        .when(wrap_lines, |btn| btn.selected(true))
                                        .on_click(move |_, window, cx| {
                                            this_wrap.update(cx, |view, cx| {
                                                view.toggle_wrap_lines(window, cx);
                                            });
                                        }),
                                )
                            })
                            .when(show_wrap_toggle, |el| {
                                el.child(
                                    Button::new("find-in-response")
                                        .icon(Icon::new(IconName::Funnel).size(px(14.0)))
                                        .ghost()
                                        .xsmall()
                                        .tooltip("Find in response (Cmd+F)")
                                        .on_click(move |_, window, cx| {
                                            this_search.update(cx, |view, cx| {
                                                view.trigger_search(window, cx);
                                            });
                                        }),
                                )
                            })
                            .child(
                                Button::new("copy-response")
                                    .icon(Icon::new(IconName::Copy).size(px(14.0)))
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Copy response")
                                    .on_click(move |_, window, cx| {
                                        this.update(cx, |view, cx| {
                                            view.copy_response(window, cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new("save-response")
                                    .icon(Icon::new(IconName::FileDown).size(px(14.0)))
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Save to file")
                                    .on_click(move |_, window, cx| {
                                        this_save.update(cx, |view, cx| {
                                            view.save_to_file(window, cx);
                                        });
                                    }),
                            ),
                    ),
            )
            .child(match self.active_tab {
                ResponseTab::Body => self.render_body_tab(theme, data).into_any_element(),
                ResponseTab::Raw => self.render_raw_tab(theme).into_any_element(),
                ResponseTab::Headers => self.render_headers_tab(theme, data, cx).into_any_element(),
            })
            .into_any_element()
    }

    fn render_body_tab(
        &self,
        theme: &gpui_component::theme::ThemeColor,
        data: &ResponseData,
    ) -> impl IntoElement {
        let content_type = data.content_category();

        if content_type == ContentCategory::Image {
            // Detect image format from content-type
            let format = match data.content_type.as_deref() {
                Some(ct) if ct.contains("png") => ImageFormat::Png,
                Some(ct) if ct.contains("jpeg") || ct.contains("jpg") => ImageFormat::Jpeg,
                Some(ct) if ct.contains("gif") => ImageFormat::Gif,
                Some(ct) if ct.contains("webp") => ImageFormat::Webp,
                Some(ct) if ct.contains("bmp") => ImageFormat::Bmp,
                Some(ct) if ct.contains("svg") => ImageFormat::Svg,
                _ => ImageFormat::Png, // Default to PNG
            };

            // Only render image if we have bytes
            if !data.body_bytes.is_empty() {
                let image = Arc::new(Image::from_bytes(format, data.body_bytes.clone()));

                return div()
                    .id("body-image-container")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .overflow_x_hidden()
                    .bg(theme.muted)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .p(px(16.0))
                            .child(img(image).max_w_full().object_fit(gpui::ObjectFit::Contain))
                            .child(
                                div()
                                    .pt(px(8.0))
                                    .text_color(theme.muted_foreground)
                                    .text_size(px(11.0))
                                    .child(format!(
                                        "{} • {} bytes",
                                        data.content_type.as_deref().unwrap_or("image"),
                                        data.body_size_bytes
                                    )),
                            ),
                    )
                    .into_any_element();
            } else {
                // No bytes available, show placeholder
                return div()
                    .id("body-image-placeholder")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .h_full()
                    .items_center()
                    .justify_center()
                    .bg(theme.muted)
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .text_size(px(12.0))
                            .child(format!(
                                "Image ({}) - {} bytes",
                                data.content_type.as_deref().unwrap_or("unknown"),
                                data.body_size_bytes
                            )),
                    )
                    .into_any_element();
            }
        }

        // For text/JSON/HTML/XML, show the pretty-formatted editor
        div()
            .id("body-scroll-container")
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .bg(theme.muted)
            .when_some(self.body_display.as_ref(), |el, editor| {
                el.child(Input::new(editor).appearance(false).size_full())
            })
            .into_any_element()
    }

    fn render_raw_tab(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
        // Raw response
        div()
            .id("raw-scroll-container")
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .bg(theme.muted)
            .when_some(self.raw_display.as_ref(), |el, editor| {
                el.child(Input::new(editor).appearance(false).size_full())
            })
    }

    fn render_headers_tab(
        &self,
        theme: &gpui_component::theme::ThemeColor,
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
                .text_color(theme.muted_foreground)
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

        let bg_primary = theme.secondary;
        let bg_alternate = theme.muted;
        let border_color = theme.border.opacity(0.3);
        let key_color = theme.secondary_foreground;
        let value_color = theme.foreground;

        div()
            .id("headers-virtual-container")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .bg(theme.muted)
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
                                let value_for_copy = value.clone();
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
                                    .group("header-row")
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
                                    // Value column with copy button
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_color(value_color)
                                                    .text_size(px(12.0))
                                                    .overflow_hidden()
                                                    .text_ellipsis()
                                                    .child(value.clone()),
                                            )
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "copy-btn-wrapper-{}",
                                                        idx
                                                    )))
                                                    .invisible()
                                                    .group_hover("header-row", |s| s.visible())
                                                    .child(
                                                        Button::new(SharedString::from(format!(
                                                            "copy-header-{}",
                                                            idx
                                                        )))
                                                        .icon(
                                                            Icon::new(IconName::Copy)
                                                                .size(px(14.0)),
                                                        )
                                                        .ghost()
                                                        .xsmall()
                                                        .rounded(px(4.0))
                                                        .cursor_pointer()
                                                        .tooltip("Copy value")
                                                        .on_click(move |_event, window, cx| {
                                                            cx.write_to_clipboard(
                                                                gpui::ClipboardItem::new_string(
                                                                    value_for_copy.clone(),
                                                                ),
                                                            );
                                                            window.push_notification(
                                                                (
                                                                    NotificationType::Success,
                                                                    "Copied to clipboard",
                                                                ),
                                                                cx,
                                                            );
                                                        }),
                                                    ),
                                            ),
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
