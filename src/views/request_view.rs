use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, App, Context, Entity, FocusHandle, Focusable, IntoElement,
    PathPromptOptions, Render, Styled, Window,
};
use gpui_component::input::{Input, InputState};

use crate::components::{
    AuthEditor, BodyType, BodyTypeSelector, BodyTypeSelectorEvent, FormDataEditor, HeaderEditor,
    MultipartFormDataEditor, ParamsEditor,
};
use crate::entities::{Header, RequestBody, RequestEntity, RequestEvent};
use crate::icons::IconName;
use gpui_component::{ActiveTheme, Icon};

/// Active tab in the request panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestTab {
    #[default]
    Body,
    Headers,
    Params,
    Auth,
}

pub struct RequestView {
    pub request: Entity<RequestEntity>,
    active_tab: RequestTab,
    body_editor: Option<Entity<InputState>>,
    body_type: BodyType,
    /// Last body type applied to the editor (for syntax highlighting)
    last_applied_body_type: BodyType,
    body_type_selector: Option<Entity<BodyTypeSelector>>,
    form_data_editor: Option<Entity<FormDataEditor>>,
    multipart_form_data_editor: Option<Entity<MultipartFormDataEditor>>,
    header_editor: Option<Entity<HeaderEditor>>,
    params_editor: Option<Entity<ParamsEditor>>,
    auth_editor: Option<Entity<AuthEditor>>,
    focus_handle: FocusHandle,
}

impl RequestView {
    pub fn new(request: Entity<RequestEntity>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&request, |_this, _request, _event: &RequestEvent, cx| {
            cx.notify();
        })
        .detach();

        Self {
            request,
            active_tab: RequestTab::Body,
            body_editor: None,
            body_type: BodyType::None,
            last_applied_body_type: BodyType::None,
            body_type_selector: None,
            form_data_editor: None,
            multipart_form_data_editor: None,
            header_editor: None,
            params_editor: None,
            auth_editor: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Initialize the body editor with Window access
    fn ensure_body_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let syntax_lang = self.body_type.syntax_language();

        if self.body_editor.is_none() {
            // Create code editor with current body type syntax highlighting
            let initial_content = r#"{
  "name": "example",
  "value": 123
}"#;

            let body_editor = cx.new(|cx| {
                InputState::new(window, cx)
                    .code_editor(syntax_lang)
                    .line_number(true)
                    .searchable(true)
                    .default_value(initial_content)
            });

            self.body_editor = Some(body_editor);
            self.last_applied_body_type = self.body_type;
        } else if self.body_type != self.last_applied_body_type {
            // Body type changed, update syntax highlighting
            if let Some(ref body_editor) = self.body_editor {
                // Get current text, change highlighter, then re-set the text to force refresh
                let current_text = body_editor.read(cx).text().to_string();
                body_editor.update(cx, |state, cx| {
                    state.set_highlighter(syntax_lang, cx);
                    // Force refresh by re-setting the value - this triggers _pending_update
                    state.set_value(current_text, window, cx);
                });
                self.last_applied_body_type = self.body_type;
            }
        }
    }

    /// Ensure editors are initialized
    fn ensure_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // First, ensure header editor is created
        if self.header_editor.is_none() {
            let request = self.request.clone();
            self.header_editor = Some(cx.new(|cx| HeaderEditor::new(request, cx)));
        }

        // Check if body type changed BEFORE updating syntax (which updates last_applied_body_type)
        let body_type_changed = self.body_type != self.last_applied_body_type;

        // Update Content-Type header when body type changes
        // Skip for FormData (multipart) - reqwest sets this automatically with boundary
        if body_type_changed && self.body_type != BodyType::FormData {
            if let Some(content_type) = self.body_type.content_type() {
                if let Some(ref header_editor) = self.header_editor {
                    header_editor.update(cx, |editor, cx| {
                        editor.set_or_update_header("Content-Type", content_type, window, cx);
                    });
                }
            }
        }

        // Now update body editor (which will update last_applied_body_type)
        self.ensure_body_editor(window, cx);

        if self.body_type_selector.is_none() {
            let selector = cx.new(|cx| BodyTypeSelector::new(window, cx));

            // Subscribe to body type selector events
            cx.subscribe_in(
                &selector,
                window,
                |this, _selector, event: &BodyTypeSelectorEvent, window, cx| match event {
                    BodyTypeSelectorEvent::TypeChanged(body_type) => {
                        this.body_type = *body_type;
                        cx.notify();
                    }
                    BodyTypeSelectorEvent::ImportRequested => {
                        this.import_body_from_file(window, cx);
                    }
                },
            )
            .detach();

            self.body_type_selector = Some(selector);
        }

        if self.params_editor.is_none() {
            self.params_editor = Some(cx.new(|cx| ParamsEditor::new(cx)));
        }

        if self.form_data_editor.is_none() {
            self.form_data_editor = Some(cx.new(|cx| FormDataEditor::new(cx)));
        }

        if self.multipart_form_data_editor.is_none() {
            self.multipart_form_data_editor = Some(cx.new(|cx| MultipartFormDataEditor::new(cx)));
        }

        if self.auth_editor.is_none() {
            self.auth_editor = Some(cx.new(|cx| AuthEditor::new(window, cx)));
        }
    }

    pub fn set_tab(&mut self, tab: RequestTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }

    /// Get body content from editor
    pub fn get_body_content(&self, cx: &App) -> Option<String> {
        self.body_editor
            .as_ref()
            .map(|editor| editor.read(cx).text().to_string())
    }

    /// Get the request body with proper type
    pub fn get_request_body(&self, cx: &App) -> RequestBody {
        let content = self.get_body_content(cx).unwrap_or_default();

        match self.body_type {
            BodyType::None => RequestBody::None,
            BodyType::Json => RequestBody::Json(content),
            BodyType::Text | BodyType::Html | BodyType::Xml => RequestBody::Text(content),
            BodyType::FormUrlEncoded => {
                if let Some(ref editor) = self.form_data_editor {
                    RequestBody::FormData(editor.read(cx).get_form_data(cx))
                } else {
                    RequestBody::FormData(std::collections::HashMap::new())
                }
            }
            BodyType::FormData => {
                if let Some(ref editor) = self.multipart_form_data_editor {
                    RequestBody::MultipartFormData(editor.read(cx).get_multipart_fields(cx))
                } else {
                    RequestBody::MultipartFormData(Vec::new())
                }
            }
        }
    }

    /// Get all headers including auth headers
    pub fn get_all_headers(&self, cx: &App) -> Vec<Header> {
        let mut headers = Vec::new();

        // Get headers from header editor
        if let Some(ref editor) = self.header_editor {
            headers.extend(editor.read(cx).get_headers(cx));
        }

        // Add Content-Type header based on body type
        // Skip for FormData (multipart) - reqwest sets this automatically with boundary
        if self.body_type != BodyType::FormData {
            if let Some(content_type) = self.body_type.content_type() {
                // Check if Content-Type is already present
                if !headers
                    .iter()
                    .any(|h| h.key.to_lowercase() == "content-type")
                {
                    headers.push(Header::new("Content-Type", content_type));
                }
            }
        }

        // Add auth header if applicable
        if let Some(ref auth_editor) = self.auth_editor {
            let config = auth_editor.read(cx).get_config(cx);
            if let Some((key, value)) = config.to_header() {
                headers.push(Header::new(key, value));
            }
        }

        headers
    }

    /// Sync body to request entity
    pub fn sync_body_to_request(&self, cx: &mut Context<Self>) {
        let body = self.get_request_body(cx);
        self.request.update(cx, |req, cx| {
            req.set_body(body, cx);
        });
    }

    /// Sync headers to request entity
    pub fn sync_headers_to_request(&self, cx: &mut Context<Self>) {
        let headers = self.get_all_headers(cx);
        self.request.update(cx, |req, cx| {
            // Clear existing headers
            while !req.headers().is_empty() {
                req.remove_header(0, cx);
            }
            // Add new headers
            for header in headers {
                req.add_header(header, cx);
            }
        });
    }

    /// Get query string from params editor
    pub fn get_query_string(&self, cx: &App) -> String {
        if let Some(ref params_editor) = self.params_editor {
            params_editor.read(cx).build_query_string(cx)
        } else {
            String::new()
        }
    }

    /// Import body content from a file
    pub fn import_body_from_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body_editor = self.body_editor.clone();
        let body_type = self.body_type;
        let this = cx.entity().clone();

        // Build file filter extensions based on content type
        let file_extensions: Option<Vec<&'static str>> = match body_type {
            BodyType::Json => Some(vec!["json"]),
            BodyType::Xml => Some(vec!["xml"]),
            BodyType::Html => Some(vec!["html", "htm"]),
            BodyType::Text => Some(vec!["txt", "text"]),
            _ => None,
        };

        // Create file prompt options
        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select file to import".into()),
        };

        // Open file picker - returns oneshot::Receiver
        let paths_receiver = cx.prompt_for_paths(options);

        cx.spawn_in(window, async move |_weak_this, cx| {
            // Await the file picker result from the oneshot channel
            // Returns Result<Result<Option<Vec<PathBuf>>, anyhow::Error>, oneshot::Canceled>
            let channel_result = paths_receiver.await;

            // Handle channel error (oneshot::Canceled)
            let Ok(platform_result) = channel_result else {
                log::error!("File picker channel closed unexpectedly");
                return;
            };

            // Handle platform error (anyhow::Error)
            let Ok(paths_opt) = platform_result else {
                log::error!("File picker failed");
                return;
            };

            // User cancelled the dialog
            let Some(paths) = paths_opt else {
                return;
            };

            let Some(path) = paths.first() else {
                return;
            };

            // Validate file extension if we have a filter
            if let Some(ref extensions) = file_extensions {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if !extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                        log::warn!(
                            "File extension '{}' doesn't match expected type for {:?}",
                            ext,
                            body_type
                        );
                    }
                }
            }

            // Read file content
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to read file: {}", e);
                    return;
                }
            };

            // Update UI using AsyncWindowContext.update
            let _ = cx.update(|window, app| {
                // Update body editor
                if let Some(ref editor) = body_editor {
                    editor.update(app, |state, cx| {
                        state.set_value(content, window, cx);
                    });
                }
                // Notify view to refresh
                this.update(app, |_view, cx| {
                    cx.notify();
                });
            });
        })
        .detach();
    }
}

impl Focusable for RequestView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RequestView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure all editors are initialized
        self.ensure_editors(window, cx);

        let theme = cx.theme();
        let this = cx.entity().clone();

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .overflow_hidden()
            // Tab bar with click handlers
            .child(self.render_tabs(&theme, this))
            // Tab content
            .child(
                div()
                    .id("request-content")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.render_tab_content(&theme, cx)),
            )
    }
}

impl RequestView {
    fn render_tabs(
        &self,
        _theme: &gpui_component::theme::ThemeColor,
        this: Entity<RequestView>,
    ) -> impl IntoElement {
        use crate::components::{PanelTab, PanelTabBar};

        PanelTabBar::new()
            .child(
                PanelTab::new("Body")
                    .active(self.active_tab == RequestTab::Body)
                    .on_click({
                        let this = this.clone();
                        move |_event, _window, cx| {
                            this.update(cx, |view, cx| view.set_tab(RequestTab::Body, cx));
                        }
                    }),
            )
            .child(
                PanelTab::new("Params")
                    .active(self.active_tab == RequestTab::Params)
                    .on_click({
                        let this = this.clone();
                        move |_event, _window, cx| {
                            this.update(cx, |view, cx| view.set_tab(RequestTab::Params, cx));
                        }
                    }),
            )
            .child(
                PanelTab::new("Headers")
                    .active(self.active_tab == RequestTab::Headers)
                    .on_click({
                        let this = this.clone();
                        move |_event, _window, cx| {
                            this.update(cx, |view, cx| view.set_tab(RequestTab::Headers, cx));
                        }
                    }),
            )
            .child(
                PanelTab::new("Auth")
                    .active(self.active_tab == RequestTab::Auth)
                    .on_click({
                        let this = this.clone();
                        move |_event, _window, cx| {
                            this.update(cx, |view, cx| view.set_tab(RequestTab::Auth, cx));
                        }
                    }),
            )
    }

    fn render_tab_content(
        &self,
        theme: &gpui_component::theme::ThemeColor,
        cx: &Context<Self>,
    ) -> AnyElement {
        let _request = self.request.read(cx);

        match self.active_tab {
            RequestTab::Body => self.render_body_tab(theme).into_any_element(),
            RequestTab::Params => self.render_params_tab().into_any_element(),
            RequestTab::Headers => self.render_headers_tab().into_any_element(),
            RequestTab::Auth => self.render_auth_tab().into_any_element(),
        }
    }

    fn render_body_tab(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
        // Container with body type selector and editor
        div()
            .id("request-body-editor")
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .overflow_hidden()
            // Body type selector
            .when_some(self.body_type_selector.as_ref(), |el, selector| {
                el.child(selector.clone())
            })
            // Form data editor for x-www-form-urlencoded
            .when(self.body_type == BodyType::FormUrlEncoded, |el| {
                el.when_some(self.form_data_editor.as_ref(), |el, editor| {
                    el.child(editor.clone())
                })
            })
            // Multipart form data editor for form-data
            .when(self.body_type == BodyType::FormData, |el| {
                el.when_some(self.multipart_form_data_editor.as_ref(), |el, editor| {
                    el.child(editor.clone())
                })
            })
            // Body editor (only show when body type is not None, FormUrlEncoded, or FormData)
            .when(
                self.body_type != BodyType::None
                    && self.body_type != BodyType::FormUrlEncoded
                    && self.body_type != BodyType::FormData,
                |el| {
                    el.child(
                        div()
                            .id("request-body-editor-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .bg(theme.muted)
                            .when_some(self.body_editor.as_ref(), |el, editor| {
                                el.child(Input::new(editor).appearance(false).size_full())
                            }),
                    )
                },
            )
            // Placeholder when body type is None
            .when(self.body_type == BodyType::None, |el| {
                el.child(
                    div()
                        .id("request-body-none-placeholder")
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(12.0))
                        .items_center()
                        .justify_center()
                        .bg(theme.muted)
                        .child(
                            Icon::new(IconName::Ban)
                                .size(px(32.0))
                                .text_color(theme.muted_foreground),
                        )
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .text_size(px(13.0))
                                .child("This request does not have a body"),
                        ),
                )
            })
    }

    fn render_params_tab(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .when_some(self.params_editor.as_ref(), |el, editor| {
                el.child(editor.clone())
            })
    }

    fn render_headers_tab(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .when_some(self.header_editor.as_ref(), |el, header_editor| {
                el.child(header_editor.clone())
            })
    }

    fn render_auth_tab(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .when_some(self.auth_editor.as_ref(), |el, editor| {
                el.child(editor.clone())
            })
    }
}
