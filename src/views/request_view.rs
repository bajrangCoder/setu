use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, App, Entity, FocusHandle, Focusable, IntoElement, Render, Styled, Window,
};
use gpui_component::input::{Input, InputState};

use crate::entities::{RequestEntity, RequestEvent};
use crate::theme::Theme;

/// Active tab in the request panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestTab {
    #[default]
    Body,
    Headers,
    Params,
    Auth,
}

/// Request view
pub struct RequestView {
    pub request: Entity<RequestEntity>,
    active_tab: RequestTab,
    body_editor: Option<Entity<InputState>>,
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
            focus_handle: cx.focus_handle(),
        }
    }

    /// Initialize the body editor with Window access
    fn ensure_body_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.body_editor.is_none() {
            // Create code editor with JSON syntax highlighting using gpui-component
            let initial_content = r#"{
  "name": "example",
  "value": 123
}"#;

            let body_editor = cx.new(|cx| {
                InputState::new(window, cx)
                    .code_editor("json")
                    .line_number(true)
                    .default_value(initial_content)
            });

            self.body_editor = Some(body_editor);
        }
    }

    pub fn set_tab(&mut self, tab: RequestTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }
}

impl Focusable for RequestView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RequestView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure body editor is initialized
        self.ensure_body_editor(window, cx);

        let theme = Theme::dark();
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
            // Tab content - fills available space
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
    fn render_tabs(&self, _theme: &Theme, this: Entity<RequestView>) -> impl IntoElement {
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

    fn render_tab_content(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let request = self.request.read(cx);

        match self.active_tab {
            RequestTab::Body => self.render_body_tab(theme).into_any_element(),
            RequestTab::Params => self.render_params_tab(theme).into_any_element(),
            RequestTab::Headers => self.render_headers_tab(theme, request).into_any_element(),
            RequestTab::Auth => self.render_auth_tab(theme).into_any_element(),
        }
    }

    fn render_body_tab(&self, _theme: &Theme) -> impl IntoElement {
        let theme = Theme::dark();

        // Clean minimal container - no padding before line numbers
        div()
            .id("request-body-editor")
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .bg(theme.colors.bg_tertiary)
            .when_some(self.body_editor.as_ref(), |el, editor| {
                el.child(Input::new(editor).appearance(false).size_full())
            })
    }

    fn render_params_tab(&self, theme: &Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_1()
            .w_full()
            .min_h(px(100.0))
            .text_color(theme.colors.text_muted)
            .text_size(px(12.0))
            .child("No query parameters")
    }

    fn render_headers_tab(&self, theme: &Theme, request: &RequestEntity) -> impl IntoElement {
        let headers = request.headers();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .gap(px(4.0))
            .when(headers.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_1()
                        .min_h(px(100.0))
                        .text_color(theme.colors.text_muted)
                        .text_size(px(12.0))
                        .child("No headers"),
                )
            })
            .children(headers.iter().map(|header| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .py(px(8.0))
                    .px(px(12.0))
                    .bg(theme.colors.bg_tertiary)
                    .rounded(px(4.0))
                    .child(
                        div()
                            .w(px(140.0))
                            .text_color(theme.colors.accent)
                            .text_size(px(12.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(header.key.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(theme.colors.text_primary)
                            .text_size(px(12.0))
                            .child(header.value.clone()),
                    )
            }))
    }

    fn render_auth_tab(&self, theme: &Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_1()
            .w_full()
            .min_h(px(100.0))
            .text_color(theme.colors.text_muted)
            .text_size(px(12.0))
            .child("No authentication")
    }
}
