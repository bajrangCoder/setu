use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    div, px, size, AnyElement, App, Context, ElementId, Entity, FocusHandle, Focusable,
    IntoElement, Pixels, Render, SharedString, Size, Styled, Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::Scrollbar;
use gpui_component::v_virtual_list;
use gpui_component::VirtualListScrollHandle;

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
    /// Virtual list scroll handle for headers tab
    headers_scroll_handle: VirtualListScrollHandle,
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
            headers_scroll_handle: VirtualListScrollHandle::new(),
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
            RequestTab::Headers => self
                .render_headers_tab(theme, request, cx)
                .into_any_element(),
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

    fn render_headers_tab(
        &self,
        theme: &Theme,
        request: &RequestEntity,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        // Get headers and convert to Vec for indexing
        let headers: Vec<(String, String)> = request
            .headers()
            .iter()
            .map(|h| (h.key.clone(), h.value.clone()))
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
        let key_color = theme.colors.accent;
        let value_color = theme.colors.text_primary;

        div()
            .id("request-headers-virtual-container")
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
                    "request-headers-list",
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
                                        "req-header-row-{}",
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
                                    // Key column - fixed width with accent color
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
                                    // Value column - fills remaining
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
