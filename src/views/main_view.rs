use gpui::prelude::*;
use gpui::{
    div, px, App, Entity, FocusHandle, Focusable, IntoElement, Render, ScrollHandle, Styled, Window,
};
use gpui_component::input::InputState;
use gpui_component::resizable::{resizable_panel, v_resizable};

use crate::components::{
    MethodDropdownOverlay, MethodDropdownState, ProtocolSelector, ProtocolType, Sidebar, TabBar,
    TabInfo, UrlBar,
};
use crate::entities::{
    Header, HistoryEntity, HttpMethod, RequestBody, RequestEntity, ResponseEntity,
};
use crate::http::HttpClient;
use crate::theme::Theme;
use crate::views::{CommandPaletteView, RequestView, ResponseView};

/// A tab representing a request with its input state
pub struct RequestTab {
    pub id: usize,
    pub name: String,
    pub request: Entity<RequestEntity>,
    pub response: Entity<ResponseEntity>,
    pub url_input: Option<Entity<InputState>>,
    pub method_dropdown: Entity<MethodDropdownState>,
}

/// Main application view
pub struct MainView {
    // Tabs
    tabs: Vec<RequestTab>,
    active_tab_index: usize,
    next_tab_id: usize,
    tab_scroll_handle: ScrollHandle,

    // Child views (for active tab)
    request_view: Entity<RequestView>,
    response_view: Entity<ResponseView>,
    command_palette: Entity<CommandPaletteView>,

    // Shared state
    history: Entity<HistoryEntity>,
    http_client: HttpClient,

    // UI state
    sidebar_visible: bool,
    focus_handle: FocusHandle,
}

impl MainView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Create initial tab
        let request = cx.new(|_| RequestEntity::new());
        let response = cx.new(|_| ResponseEntity::new());
        let method_dropdown = cx.new(|_| MethodDropdownState::new(HttpMethod::Get));

        let initial_tab = RequestTab {
            id: 0,
            name: "Untitled".to_string(),
            request: request.clone(),
            response: response.clone(),
            url_input: None, // Will be initialized lazily with Window access
            method_dropdown,
        };

        let request_view = cx.new(|cx| RequestView::new(request.clone(), cx));
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));
        let command_palette = cx.new(|cx| CommandPaletteView::new(cx));
        let history = cx.new(|_| HistoryEntity::new());

        let http_client = HttpClient::new().expect("Failed to create HTTP client");

        Self {
            tabs: vec![initial_tab],
            active_tab_index: 0,
            next_tab_id: 1,
            tab_scroll_handle: ScrollHandle::new(),
            request_view,
            response_view,
            command_palette,
            history,
            http_client,
            sidebar_visible: true,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Ensure URL input is initialized for a tab
    fn ensure_url_input(&mut self, tab_index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_index) {
            if tab.url_input.is_none() {
                let url_input =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Enter request URL..."));
                tab.url_input = Some(url_input);
            }
        }
    }

    /// Get the active tab
    fn active_tab(&self) -> Option<&RequestTab> {
        self.tabs.get(self.active_tab_index)
    }

    /// Add a new tab
    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let request = cx.new(|_| RequestEntity::new());
        let response = cx.new(|_| ResponseEntity::new());
        let method_dropdown = cx.new(|_| MethodDropdownState::new(HttpMethod::Get));

        let tab = RequestTab {
            id: self.next_tab_id,
            name: "Untitled".to_string(),
            request: request.clone(),
            response: response.clone(),
            url_input: None, // Will be initialized lazily
            method_dropdown,
        };

        self.next_tab_id += 1;
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;

        // Update views for new tab
        self.request_view = cx.new(|cx| RequestView::new(request.clone(), cx));
        self.response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));

        // Scroll to the newly added tab
        self.tab_scroll_handle.scroll_to_item(self.active_tab_index);

        cx.notify();
    }

    /// Switch to a tab by index
    pub fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() && index != self.active_tab_index {
            self.active_tab_index = index;

            // Update views for this tab
            if let Some(tab) = self.tabs.get(index) {
                self.request_view = cx.new(|cx| RequestView::new(tab.request.clone(), cx));
                self.response_view = cx.new(|cx| ResponseView::new(tab.response.clone(), cx));
            }

            // Scroll to the selected tab to ensure it's visible
            self.tab_scroll_handle.scroll_to_item(index);

            cx.notify();
        }
    }

    /// Close a tab
    pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 && index < self.tabs.len() {
            self.tabs.remove(index);

            // Adjust active index
            if self.active_tab_index >= self.tabs.len() {
                self.active_tab_index = self.tabs.len() - 1;
            } else if index < self.active_tab_index {
                self.active_tab_index -= 1;
            }

            // Update views
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                self.request_view = cx.new(|cx| RequestView::new(tab.request.clone(), cx));
                self.response_view = cx.new(|cx| ResponseView::new(tab.response.clone(), cx));
            }

            cx.notify();
        }
    }

    /// Send the current request
    pub fn send_request(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };

        let request_entity = tab.request.clone();
        let response_entity = tab.response.clone();

        // Get URL from input state
        let url = if let Some(ref url_input) = tab.url_input {
            url_input.read(cx).text().to_string()
        } else {
            String::new()
        };

        if url.is_empty() {
            response_entity.update(cx, |resp, cx| {
                resp.set_error("Please enter a URL".to_string(), cx);
            });
            return;
        }

        // Mark as sending
        request_entity.update(cx, |req, cx| {
            req.set_sending(true, cx);
        });

        response_entity.update(cx, |resp, cx| {
            resp.set_loading(cx);
        });

        // Get request params
        let request = request_entity.read(cx);
        let method = request.method();
        let headers: Vec<Header> = request.headers().to_vec();
        let body: RequestBody = request.body().clone();

        log::info!("Sending {} request to: {}", method.as_str(), url);

        // Execute HTTP request synchronously
        let result = self.http_client.execute_sync(method, &url, &headers, &body);

        // Update UI with result
        request_entity.update(cx, |req, cx| {
            req.set_sending(false, cx);
        });

        match result {
            Ok(data) => {
                log::info!(
                    "Request completed: {} {} - {} bytes in {}ms",
                    data.status_code,
                    data.status_text,
                    data.body_size_bytes,
                    data.duration_ms
                );
                response_entity.update(cx, |resp, cx| {
                    resp.set_success(data, cx);
                });
            }
            Err(e) => {
                log::error!("Request failed: {}", e);
                response_entity.update(cx, |resp, cx| {
                    resp.set_error(e.to_string(), cx);
                });
            }
        }

        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    pub fn toggle_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette.update(cx, |palette, cx| {
            palette.toggle(cx);
        });
    }
}

impl Focusable for MainView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure URL input is initialized for the active tab
        self.ensure_url_input(self.active_tab_index, window, cx);

        let theme = Theme::dark();
        let history_entries = self.history.read(cx).entries.clone();

        // Build tab infos with index
        let tab_infos: Vec<TabInfo> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let method = tab.request.read(cx).method();
                let mut info = TabInfo::new(tab.id, i, tab.name.clone(), method);
                if i == self.active_tab_index {
                    info = info.active();
                }
                info
            })
            .collect();

        // Get current request state for URL bar
        let (url_input, method_dropdown, is_loading) = if let Some(tab) = self.active_tab() {
            let req = tab.request.read(cx);
            (
                tab.url_input.clone(),
                tab.method_dropdown.clone(),
                req.is_sending(),
            )
        } else {
            return div().child("No active tab").into_any_element();
        };

        let this = cx.entity().clone();
        let this_for_send = this.clone();

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_row()
            .size_full()
            .bg(theme.colors.bg_primary)
            .text_color(theme.colors.text_primary)
            // Sidebar
            .child(Sidebar::new(history_entries).visible(self.sidebar_visible))
            // Main content
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .h_full()
                    .overflow_hidden()
                    // Header with protocol selector
                    .child(self.render_header(&theme))
                    // Tab bar - pass this entity directly with scroll handle
                    .child(TabBar::new(
                        tab_infos,
                        this.clone(),
                        self.tab_scroll_handle.clone(),
                    ))
                    // Content - vertical resizable split between request and response panels
                    .child(
                        div().flex_1().flex().flex_col().overflow_hidden().child(
                            v_resizable("request-response-split")
                                // Request panel with URL bar
                                .child(
                                    resizable_panel()
                                        .size(px(300.0))
                                        .size_range(px(150.0)..px(600.0))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .size_full()
                                                .overflow_hidden()
                                                .child(self.render_request_panel(
                                                    url_input,
                                                    method_dropdown,
                                                    is_loading,
                                                    this_for_send,
                                                )),
                                        ),
                                )
                                // Response panel
                                .child(
                                    resizable_panel()
                                        .size_range(px(150.0)..gpui::Pixels::MAX)
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .size_full()
                                                .overflow_hidden()
                                                .child(self.response_view.clone()),
                                        ),
                                ),
                        ),
                    )
                    // Bottom shortcuts bar
                    .child(self.render_shortcuts(&theme)),
            )
            // Command palette overlay
            .child(self.command_palette.clone())
            // Method dropdown overlay - renders on top of everything except command palette
            .when_some(self.active_tab(), |el, tab| {
                el.child(MethodDropdownOverlay::new(
                    tab.method_dropdown.clone(),
                    tab.request.clone(),
                ))
            })
            .into_any_element()
    }
}

impl MainView {
    fn render_request_panel(
        &self,
        url_input: Option<Entity<InputState>>,
        method_dropdown: Entity<MethodDropdownState>,
        is_loading: bool,
        this: Entity<MainView>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            // URL Bar
            .child(
                div()
                    .px(px(16.0))
                    .py(px(12.0))
                    .when_some(url_input, |el, input| {
                        el.child(
                            UrlBar::new(input)
                                .method_dropdown(method_dropdown)
                                .loading(is_loading)
                                .on_send(move |_, _, cx| {
                                    this.update(cx, |view, cx| {
                                        view.send_request(cx);
                                    });
                                }),
                        )
                    }),
            )
            // Request view tabs (body, headers, etc.)
            .child(self.request_view.clone())
    }

    fn render_header(&self, theme: &Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(44.0))
            .px(px(16.0))
            .border_b_1()
            .border_color(theme.colors.border_primary)
            // Left: Logo + Protocol selector
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_color(theme.colors.accent)
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_size(px(14.0))
                            .child("setu"),
                    )
                    .child(ProtocolSelector::new(ProtocolType::Rest)),
            )
            // Right
            .child(
                div().flex().items_center().gap(px(8.0)).child(
                    div()
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(theme.colors.bg_secondary)
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.colors.bg_tertiary))
                        .child(
                            div()
                                .text_color(theme.colors.text_muted)
                                .text_size(px(11.0))
                                .child("⌘K"),
                        ),
                ),
            )
    }

    fn render_shortcuts(&self, theme: &Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(24.0))
            .h(px(32.0))
            .border_t_1()
            .border_color(theme.colors.border_primary)
            .bg(theme.colors.bg_secondary)
            .children(
                [("Send", "⌘↵"), ("New Tab", "⌘T"), ("Commands", "⌘K")]
                    .into_iter()
                    .map(|(label, shortcut)| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_color(theme.colors.text_muted)
                                    .text_size(px(10.0))
                                    .child(label),
                            )
                            .child(
                                div()
                                    .px(px(4.0))
                                    .py(px(1.0))
                                    .bg(theme.colors.bg_tertiary)
                                    .rounded(px(2.0))
                                    .text_color(theme.colors.text_muted)
                                    .text_size(px(9.0))
                                    .child(shortcut),
                            )
                    }),
            )
    }
}
