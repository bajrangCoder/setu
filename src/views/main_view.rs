use gpui::prelude::*;
use gpui::{
    div, px, App, Entity, FocusHandle, Focusable, IntoElement, Render, ScrollHandle, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::v_flex;
use gpui_component::Root;
use gpui_component::WindowExt;
use uuid::Uuid;

use crate::actions::*;
use crate::components::{
    AppSidebar, BodyType, HistoryFilter, HistoryGroupBy, MethodDropdownState, ProtocolSelector,
    ProtocolType, SidebarTab, TabBar, TabInfo, UrlBar,
};
use crate::entities::{
    CollectionsEntity, Header, HistoryEntity, HttpMethod, RequestBody, RequestData, RequestEntity,
    ResponseData, ResponseEntity,
};
use crate::http::HttpClient;
use crate::views::request_view::RequestView;
use crate::views::response_view::ResponseView;
use crate::views::{CommandId, CommandPaletteEvent, CommandPaletteView};
use gpui_component::ActiveTheme;

pub struct TabState {
    pub id: usize,
    pub name: String,
    pub request: Entity<RequestEntity>,
    pub response: Entity<ResponseEntity>,
    pub url_input: Option<Entity<InputState>>,
    pub method_dropdown: Entity<MethodDropdownState>,
    pub request_view: Entity<RequestView>,
    pub response_view: Entity<ResponseView>,
}

/// Main application view
pub struct MainView {
    // Tabs
    tabs: Vec<TabState>,
    active_tab_index: usize,
    next_tab_id: usize,
    tab_scroll_handle: ScrollHandle,

    // Command palette (shared across tabs)
    command_palette: Entity<CommandPaletteView>,

    // Shared state
    history: Entity<HistoryEntity>,
    collections: Entity<CollectionsEntity>,
    http_client: HttpClient,

    // UI state
    sidebar_visible: bool,
    sidebar_tab: SidebarTab,
    history_search: Option<Entity<InputState>>,
    collections_search: Option<Entity<InputState>>,
    history_filter: HistoryFilter,
    history_group_by: HistoryGroupBy,
    focus_handle: FocusHandle,
}

impl MainView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Create initial tab
        let request = cx.new(|_| RequestEntity::new());
        let response = cx.new(|_| ResponseEntity::new());
        let method_dropdown = cx.new(|_| MethodDropdownState::new(HttpMethod::Get));
        let request_view = cx.new(|cx| RequestView::new(request.clone(), BodyType::None, cx));
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));

        let initial_tab = TabState {
            id: 0,
            name: "Untitled 1".to_string(),
            request: request.clone(),
            response: response.clone(),
            url_input: None,
            method_dropdown,
            request_view,
            response_view,
        };

        let command_palette = cx.new(|cx| CommandPaletteView::new(cx));
        let history = cx.new(|_| HistoryEntity::new());
        let collections = cx.new(|_| CollectionsEntity::new());

        cx.subscribe(&command_palette, |this, _, event, cx| {
            let CommandPaletteEvent::ExecuteCommand(cmd_id) = event;
            this.execute_command(*cmd_id, cx);
        })
        .detach();

        let http_client = HttpClient::new().expect("Failed to create HTTP client");

        Self {
            tabs: vec![initial_tab],
            active_tab_index: 0,
            next_tab_id: 1,
            tab_scroll_handle: ScrollHandle::new(),
            command_palette,
            history,
            collections,
            http_client,
            sidebar_visible: true,
            sidebar_tab: SidebarTab::History,
            history_search: None,
            collections_search: None,
            history_filter: HistoryFilter::All,
            history_group_by: HistoryGroupBy::Time,
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

    /// Ensure sidebar search inputs are initialized
    fn ensure_sidebar_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history_search.is_none() {
            self.history_search =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Search history...")));
        }
        if self.collections_search.is_none() {
            self.collections_search =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Search collections...")));
        }
    }

    /// Set sidebar tab
    pub fn set_sidebar_tab(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        self.sidebar_tab = tab;
        cx.notify();
    }

    pub fn set_history_filter(&mut self, filter: HistoryFilter, cx: &mut Context<Self>) {
        self.history_filter = filter;
        cx.notify();
    }

    pub fn set_history_group_by(&mut self, group_by: HistoryGroupBy, cx: &mut Context<Self>) {
        self.history_group_by = group_by;
        cx.notify();
    }

    /// Load a history entry into a new tab
    pub fn load_history_entry(
        &mut self,
        entry_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // First extract all data we need, then drop the borrow
        let entry_data = {
            let history = self.history.read(cx);
            history.get_entry(entry_id).map(|entry| {
                (
                    entry.request.clone(),
                    entry.response.clone(),
                    entry.display_name(),
                )
            })
        };

        let Some((request_data, response_data, tab_name)) = entry_data else {
            return;
        };

        // Derive body type from the stored request body
        let body_type = BodyType::from_request_body(&request_data.body);

        // Extract body content for text-based body types
        let body_content: Option<String> = match &request_data.body {
            RequestBody::Json(content) | RequestBody::Text(content) => {
                if content.is_empty() {
                    None
                } else {
                    Some(content.clone())
                }
            }
            RequestBody::None | RequestBody::FormData(_) | RequestBody::MultipartFormData(_) => {
                None
            }
        };

        // Extract form data for FormUrlEncoded
        let form_data = match &request_data.body {
            RequestBody::FormData(data) => Some(data.clone()),
            _ => None,
        };

        // Extract multipart data for FormData
        let multipart_data = match &request_data.body {
            RequestBody::MultipartFormData(fields) => Some(fields.clone()),
            _ => None,
        };

        // Now we can use cx freely
        let request = cx.new(|cx| {
            let mut req = RequestEntity::new().with_headers(request_data.headers.clone());
            req.set_url(request_data.url.clone(), cx);
            req.set_method(request_data.method, cx);
            req.set_body(request_data.body.clone(), cx);
            req
        });

        let response = cx.new(|cx| {
            let mut resp = ResponseEntity::new();
            if let Some(resp_data) = response_data {
                resp.set_response(resp_data, cx);
            }
            resp
        });

        let method_dropdown = cx.new(|_| MethodDropdownState::new(request_data.method));
        let request_view = cx.new(|cx| {
            RequestView::new(request.clone(), body_type, cx)
                .with_initial_body_content(body_content)
                .with_initial_form_data(form_data)
                .with_initial_multipart_data(multipart_data)
        });
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));
        let url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter request URL...")
                .default_value(&request_data.url)
        });

        let tab = TabState {
            id: self.next_tab_id,
            name: tab_name,
            request: request.clone(),
            response: response.clone(),
            url_input: Some(url_input),
            method_dropdown,
            request_view,
            response_view,
        };

        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        self.next_tab_id += 1;
        cx.notify();
    }

    /// Delete a history entry
    pub fn delete_history_entry(&mut self, entry_id: Uuid, cx: &mut Context<Self>) {
        self.history.update(cx, |history, cx| {
            history.remove_entry(entry_id, cx);
        });
    }

    /// Toggle star status of a history entry
    pub fn toggle_history_star(&mut self, entry_id: Uuid, cx: &mut Context<Self>) {
        self.history.update(cx, |history, cx| {
            history.toggle_star(entry_id, cx);
        });
    }

    /// Clear all history
    pub fn clear_history(&mut self, cx: &mut Context<Self>) {
        self.history.update(cx, |history, cx| {
            history.clear_unstarred(cx);
        });
    }

    #[allow(dead_code)]
    /// Add current request to history
    pub fn add_to_history(
        &mut self,
        request: RequestData,
        response: Option<ResponseData>,
        cx: &mut Context<Self>,
    ) {
        self.history.update(cx, |history, cx| {
            history.add_entry(request, response, cx);
        });
    }

    /// Load a collection item into a new tab
    pub fn load_collection_item(
        &mut self,
        collection_id: Uuid,
        item_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // First extract all data we need, then drop the borrow
        let item_data = {
            let collections = self.collections.read(cx);
            collections
                .get_collection(collection_id)
                .and_then(|collection| {
                    collection
                        .get_item(item_id)
                        .map(|item| (item.request.clone(), item.display_name()))
                })
        };

        let Some((request_data, tab_name)) = item_data else {
            return;
        };

        // Derive body type from the stored request body
        let body_type = BodyType::from_request_body(&request_data.body);

        // Extract body content for text-based body types
        let body_content: Option<String> = match &request_data.body {
            RequestBody::Json(content) | RequestBody::Text(content) => {
                if content.is_empty() {
                    None
                } else {
                    Some(content.clone())
                }
            }
            RequestBody::None | RequestBody::FormData(_) | RequestBody::MultipartFormData(_) => {
                None
            }
        };

        // Extract form data for FormUrlEncoded
        let form_data = match &request_data.body {
            RequestBody::FormData(data) => Some(data.clone()),
            _ => None,
        };

        // Extract multipart data for FormData
        let multipart_data = match &request_data.body {
            RequestBody::MultipartFormData(fields) => Some(fields.clone()),
            _ => None,
        };

        // Now we can use cx freely
        let request = cx.new(|cx| {
            let mut req = RequestEntity::new().with_headers(request_data.headers.clone());
            req.set_url(request_data.url.clone(), cx);
            req.set_method(request_data.method, cx);
            req.set_body(request_data.body.clone(), cx);
            req
        });

        let response = cx.new(|_| ResponseEntity::new());
        let method_dropdown = cx.new(|_| MethodDropdownState::new(request_data.method));
        let request_view = cx.new(|cx| {
            RequestView::new(request.clone(), body_type, cx)
                .with_initial_body_content(body_content)
                .with_initial_form_data(form_data)
                .with_initial_multipart_data(multipart_data)
        });
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));
        let url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter request URL...")
                .default_value(&request_data.url)
        });

        let tab = TabState {
            id: self.next_tab_id,
            name: tab_name,
            request: request.clone(),
            response: response.clone(),
            url_input: Some(url_input),
            method_dropdown,
            request_view,
            response_view,
        };

        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        self.next_tab_id += 1;
        cx.notify();
    }

    /// Create a new collection
    pub fn create_new_collection(&mut self, cx: &mut Context<Self>) {
        self.collections.update(cx, |collections, cx| {
            collections.create_collection("New Collection", cx);
        });
    }

    /// Delete a collection
    pub fn delete_collection(&mut self, collection_id: Uuid, cx: &mut Context<Self>) {
        self.collections.update(cx, |collections, cx| {
            collections.remove_collection(collection_id, cx);
        });
    }

    /// Delete an item from a collection
    pub fn delete_collection_item(
        &mut self,
        collection_id: Uuid,
        item_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        self.collections.update(cx, |collections, cx| {
            collections.remove_item_from_collection(collection_id, item_id, cx);
        });
    }

    /// Toggle collection expanded state
    pub fn toggle_collection_expand(&mut self, collection_id: Uuid, cx: &mut Context<Self>) {
        self.collections.update(cx, |collections, cx| {
            collections.toggle_collection_expanded(collection_id, cx);
        });
    }

    #[allow(dead_code)]
    /// Save current request to a collection
    pub fn save_to_collection(&mut self, collection_id: Uuid, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            let request = tab.request.read(cx);
            let request_data = RequestData {
                id: Uuid::new_v4(),
                name: tab.name.clone(),
                url: request.url().to_string(),
                method: request.method(),
                headers: request.headers().to_vec(),
                body: request.body().clone(),
                is_sending: false,
            };

            self.collections.update(cx, |collections, cx| {
                collections.add_item_to_collection(collection_id, request_data, cx);
            });
        }
    }

    /// Get the active tab
    fn active_tab(&self) -> Option<&TabState> {
        self.tabs.get(self.active_tab_index)
    }

    /// Add a new tab
    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let request = cx.new(|_| RequestEntity::new());
        let response = cx.new(|_| ResponseEntity::new());
        let method_dropdown = cx.new(|_| MethodDropdownState::new(HttpMethod::Get));
        let request_view = cx.new(|cx| RequestView::new(request.clone(), BodyType::None, cx));
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));

        self.next_tab_id += 1;
        let tab = TabState {
            id: self.next_tab_id,
            name: format!("Untitled {}", self.next_tab_id),
            request: request.clone(),
            response: response.clone(),
            url_input: None,
            method_dropdown,
            request_view,
            response_view,
        };

        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;

        // Scroll to the newly added tab
        self.tab_scroll_handle.scroll_to_item(self.active_tab_index);

        cx.notify();
    }

    /// Switch to a tab by index
    pub fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() && index != self.active_tab_index {
            self.active_tab_index = index;

            // Notify the views to re-render with their current data
            if let Some(tab) = self.tabs.get(index) {
                tab.request_view.update(cx, |_, cx| cx.notify());
                tab.response_view.update(cx, |_, cx| cx.notify());
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

            cx.notify();
        }
    }

    /// Rename a tab
    pub fn rename_tab(&mut self, index: usize, new_name: String, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.tabs[index].name = new_name;
            cx.notify();
        }
    }

    /// Show rename dialog for a tab
    pub fn show_rename_dialog(
        &mut self,
        index: usize,
        current_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity().clone();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(&current_name));

        // Subscribe to Enter key press on the input
        cx.subscribe_in(&input, window, move |view, state, event, window, cx| {
            use gpui_component::input::InputEvent;
            if let InputEvent::PressEnter { .. } = event {
                let new_name = state.read(cx).text().to_string();
                view.rename_tab(index, new_name, cx);
                window.close_dialog(cx);
            }
        })
        .detach();

        // Clone for footer button clicks
        let input_for_footer = input.clone();
        let this_for_footer = this.clone();

        window.open_dialog(cx, move |dialog, _, _| {
            // Clone again before the inner move closure
            let input_for_buttons = input_for_footer.clone();
            let this_for_buttons = this_for_footer.clone();

            dialog
                .title("Rename Tab")
                .child(
                    v_flex()
                        .gap_3()
                        .child("Enter a new name for this tab:")
                        .child(Input::new(&input)),
                )
                .footer({
                    // Clone before moving into footer closure
                    let input_submit = input_for_buttons.clone();
                    let this_submit = this_for_buttons.clone();

                    move |_, _, _, _| {
                        // Clone again for on_click closure
                        let input_click = input_submit.clone();
                        let this_click = this_submit.clone();

                        vec![
                            Button::new("rename-submit")
                                .primary()
                                .label("Rename")
                                .on_click(move |_, window, cx| {
                                    let new_name = input_click.read(cx).text().to_string();
                                    this_click.update(cx, |view, cx| {
                                        view.rename_tab(index, new_name, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                            Button::new("rename-cancel").label("Cancel").on_click(
                                |_, window, cx| {
                                    window.close_dialog(cx);
                                },
                            ),
                        ]
                    }
                })
        });
    }

    /// Close all tabs except the one at the given index
    pub fn close_other_tabs(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            // Keep only the tab at the given index
            let tab_to_keep = self.tabs.remove(index);
            self.tabs.clear();
            self.tabs.push(tab_to_keep);
            self.active_tab_index = 0;

            cx.notify();
        }
    }

    /// Send the current request
    pub fn send_request(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };

        let request_entity = tab.request.clone();
        let response_entity = tab.response.clone();
        let request_view = tab.request_view.clone();

        // Get URL from input state
        let base_url = if let Some(ref url_input) = tab.url_input {
            url_input.read(cx).text().to_string()
        } else {
            String::new()
        };

        if base_url.is_empty() {
            response_entity.update(cx, |resp, cx| {
                resp.set_error("Please enter a URL".to_string(), cx);
            });
            return;
        }

        // Sync body and headers from RequestView to RequestEntity
        request_view.update(cx, |view, cx| {
            view.sync_body_to_request(cx);
            view.sync_headers_to_request(cx);
        });

        // Get query string from params editor
        let query_string = request_view.read(cx).get_query_string(cx);

        // Build final URL with query params
        let url = if !query_string.is_empty() {
            // Check if URL already has query params
            if base_url.contains('?') {
                format!("{}&{}", base_url, &query_string[1..]) // Skip the leading '?'
            } else {
                format!("{}{}", base_url, query_string)
            }
        } else {
            base_url
        };

        // Mark as sending
        request_entity.update(cx, |req, cx| {
            req.set_sending(true, cx);
        });

        response_entity.update(cx, |resp, cx| {
            resp.set_loading(cx);
        });

        // Get request params (clone what we need before async)
        let request = request_entity.read(cx);
        let method = request.method();
        let headers: Vec<Header> = request.headers().to_vec();
        let body: RequestBody = request.body().clone();

        log::info!("Sending {} request to: {}", method.as_str(), url);
        log::info!(
            "Headers: {:?}",
            headers
                .iter()
                .map(|h| format!("{}: {}", h.key, h.value))
                .collect::<Vec<_>>()
        );
        log::info!("Body: {:?}", body);

        // Create request data for history before sending
        let history_request_data = RequestData {
            id: Uuid::new_v4(),
            name: tab.name.clone(),
            url: url.clone(),
            method,
            headers: headers.clone(),
            body: body.clone(),
            is_sending: false,
        };

        let history_entity = self.history.clone();

        // Spawn HTTP request on Tokio runtime
        let result_rx = self.http_client.spawn_request(method, url, headers, body);

        // Spawn foreground task to await result and update UI
        cx.spawn(async move |_view, cx| {
            // Await the result from Tokio runtime
            let result = result_rx.await;

            // Update entities with result in a single sync context
            cx.update(|app| {
                // Mark request as done sending
                request_entity.update(app, |req, cx| {
                    req.set_sending(false, cx);
                });

                // Handle the result
                match result {
                    Ok(Ok(data)) => {
                        log::info!(
                            "Request completed: {} {} - {} bytes in {}ms",
                            data.status_code,
                            data.status_text,
                            data.body_size_bytes,
                            data.duration_ms
                        );

                        // Create response data for history (clone the data)
                        let history_response_data = data.clone();

                        // Add to history
                        history_entity.update(app, |history, cx| {
                            history.add_entry(
                                history_request_data.clone(),
                                Some(history_response_data),
                                cx,
                            );
                        });

                        response_entity.update(app, |resp, cx| {
                            resp.set_success(data, cx);
                        });
                    }
                    Ok(Err(e)) => {
                        log::error!("Request failed: {}", e);

                        // Add to history even on error (no response data)
                        history_entity.update(app, |history, cx| {
                            history.add_entry(history_request_data.clone(), None, cx);
                        });

                        response_entity.update(app, |resp, cx| {
                            resp.set_error(e.to_string(), cx);
                        });
                    }
                    Err(_) => {
                        log::error!("Request channel closed unexpectedly");
                        response_entity.update(app, |resp, cx| {
                            resp.set_error("Request was cancelled".to_string(), cx);
                        });
                    }
                }
            })
        })
        .detach_and_log_err(cx);
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    pub fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_palette.update(cx, |palette, cx| {
            palette.toggle(window, cx);
        });
    }

    pub fn execute_command(&mut self, cmd_id: CommandId, cx: &mut Context<Self>) {
        match cmd_id {
            CommandId::SendRequest => self.send_request(cx),
            CommandId::NewRequest => self.new_tab(cx),
            CommandId::CloseTab => self.close_current_tab(cx),
            CommandId::CloseAllTabs => self.close_all_tabs(cx),
            CommandId::CloseOtherTabs => self.close_current_other_tabs(cx),
            CommandId::NextTab => self.next_tab(cx),
            CommandId::PreviousTab => self.previous_tab(cx),
            CommandId::GoToLastTab => self.go_to_last_tab(cx),
            CommandId::ToggleSidebar => self.toggle_sidebar(cx),
            CommandId::SetMethodGet => self.set_method(HttpMethod::Get, cx),
            CommandId::SetMethodPost => self.set_method(HttpMethod::Post, cx),
            CommandId::SetMethodPut => self.set_method(HttpMethod::Put, cx),
            CommandId::SetMethodDelete => self.set_method(HttpMethod::Delete, cx),
            CommandId::SetMethodPatch => self.set_method(HttpMethod::Patch, cx),
            CommandId::SetMethodHead => self.set_method(HttpMethod::Head, cx),
            CommandId::SetMethodOptions => self.set_method(HttpMethod::Options, cx),
            CommandId::ClearHistory => {
                self.history.update(cx, |h, cx| h.clear(cx));
            }
            CommandId::SwitchToBodyTab => {
                self.switch_to_request_tab(crate::views::request_view::RequestTab::Body, cx);
            }
            CommandId::SwitchToParamsTab => {
                self.switch_to_request_tab(crate::views::request_view::RequestTab::Params, cx);
            }
            CommandId::SwitchToHeadersTab => {
                self.switch_to_request_tab(crate::views::request_view::RequestTab::Headers, cx);
            }
            CommandId::SwitchToAuthTab => {
                self.switch_to_request_tab(crate::views::request_view::RequestTab::Auth, cx);
            }
            CommandId::SwitchToResponseBody => {
                self.switch_to_response_tab(crate::views::response_view::ResponseTab::Body, cx);
            }
            CommandId::SwitchToResponseHeaders => {
                self.switch_to_response_tab(crate::views::response_view::ResponseTab::Headers, cx);
            }
            CommandId::DuplicateRequest | CommandId::FocusUrlBar => {
                // These need window access, handled separately
            }
        }
    }

    pub fn next_tab(&mut self, cx: &mut Context<Self>) {
        let next_index = (self.active_tab_index + 1) % self.tabs.len();
        self.switch_tab(next_index, cx);
    }

    pub fn previous_tab(&mut self, cx: &mut Context<Self>) {
        let prev_index = if self.active_tab_index == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab_index - 1
        };
        self.switch_tab(prev_index, cx);
    }

    pub fn go_to_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.switch_tab(index, cx);
        }
    }

    pub fn go_to_last_tab(&mut self, cx: &mut Context<Self>) {
        let last_index = self.tabs.len().saturating_sub(1);
        self.switch_tab(last_index, cx);
    }

    pub fn close_current_tab(&mut self, cx: &mut Context<Self>) {
        self.close_tab(self.active_tab_index, cx);
    }

    pub fn close_all_tabs(&mut self, cx: &mut Context<Self>) {
        while self.tabs.len() > 1 {
            self.tabs.pop();
        }
        self.active_tab_index = 0;
        cx.notify();
    }

    pub fn close_current_other_tabs(&mut self, cx: &mut Context<Self>) {
        self.close_other_tabs(self.active_tab_index, cx);
    }

    pub fn duplicate_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(current_tab) = self.active_tab() {
            let old_request = current_tab.request.clone();
            let old_url_input = current_tab.url_input.clone();
            let old_method_dropdown = current_tab.method_dropdown.clone();
            let old_name = current_tab.name.clone();
            let old_request_view = current_tab.request_view.clone();

            let new_method = old_method_dropdown.read(cx).method();
            let old_body_type = old_request_view.read(cx).get_body_type();

            let old_headers: Vec<_> = old_request.read(cx).headers().to_vec();
            let old_body = old_request.read(cx).body().clone();

            let new_request = cx.new(|_| RequestEntity::new());
            let new_response = cx.new(|_| ResponseEntity::new());
            let new_method_dropdown = cx.new(|_| MethodDropdownState::new(new_method));

            let new_url_input = if let Some(old_url) = old_url_input {
                let url_text = old_url.read(cx).text().to_string();
                Some(cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Enter request URL...")
                        .default_value(&url_text)
                }))
            } else {
                None
            };

            new_request.update(cx, |new_req, cx| {
                for header in old_headers {
                    new_req.add_header(header, cx);
                }
                new_req.set_body(old_body, cx);
            });

            let new_request_view =
                cx.new(|cx| RequestView::new(new_request.clone(), old_body_type, cx));
            let new_response_view = cx.new(|cx| ResponseView::new(new_response.clone(), cx));

            self.next_tab_id += 1;
            let new_tab = TabState {
                id: self.next_tab_id,
                name: format!("{} (copy)", old_name),
                request: new_request.clone(),
                response: new_response.clone(),
                url_input: new_url_input,
                method_dropdown: new_method_dropdown,
                request_view: new_request_view,
                response_view: new_response_view,
            };

            self.tabs.push(new_tab);
            self.active_tab_index = self.tabs.len() - 1;
            self.tab_scroll_handle.scroll_to_item(self.active_tab_index);
            cx.notify();
        }
    }

    pub fn set_method(&mut self, method: HttpMethod, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(self.active_tab_index) {
            tab.method_dropdown.update(cx, |state, cx| {
                state.set_method(method, cx);
            });
            tab.request.update(cx, |req, cx| {
                req.set_method(method, cx);
            });
        }
    }

    pub fn switch_to_request_tab(
        &mut self,
        tab: crate::views::request_view::RequestTab,
        cx: &mut Context<Self>,
    ) {
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            active_tab.request_view.update(cx, |view, cx| {
                view.set_tab(tab, cx);
            });
        }
    }

    pub fn switch_to_response_tab(
        &mut self,
        tab: crate::views::response_view::ResponseTab,
        cx: &mut Context<Self>,
    ) {
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            active_tab.response_view.update(cx, |view, cx| {
                view.set_tab(tab, cx);
            });
        }
    }

    pub fn focus_url_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(self.active_tab_index) {
            if let Some(url_input) = &tab.url_input {
                url_input.update(cx, |state, cx| {
                    state.focus(window, cx);
                });
            }
        }
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
        // Ensure sidebar search inputs are initialized
        self.ensure_sidebar_inputs(window, cx);

        let theme = cx.theme();

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
        let (url_input, method_dropdown, request_entity, is_loading, request_view, response_view) =
            if let Some(tab) = self.active_tab() {
                let req = tab.request.read(cx);
                (
                    tab.url_input.clone(),
                    tab.method_dropdown.clone(),
                    tab.request.clone(),
                    req.is_sending(),
                    tab.request_view.clone(),
                    tab.response_view.clone(),
                )
            } else {
                return div().child("No active tab").into_any_element();
            };

        let this = cx.entity().clone();
        let this_for_send = this.clone();

        div()
            .id("main-view")
            .key_context("MainView")
            .track_focus(&self.focus_handle)
            .focusable()
            .flex()
            .flex_row()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            // Request actions
            .on_action(cx.listener(|this, _: &SendRequest, _window, cx| {
                this.send_request(cx);
            }))
            .on_action(cx.listener(|this, _: &NewRequest, _window, cx| {
                this.new_tab(cx);
            }))
            .on_action(cx.listener(|this, _: &DuplicateRequest, window, cx| {
                this.duplicate_request(window, cx);
            }))
            // Tab navigation
            .on_action(cx.listener(|this, _: &NextTab, _window, cx| {
                this.next_tab(cx);
            }))
            .on_action(cx.listener(|this, _: &PreviousTab, _window, cx| {
                this.previous_tab(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, _window, cx| {
                this.close_current_tab(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseAllTabs, _window, cx| {
                this.close_all_tabs(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseOtherTabs, _window, cx| {
                this.close_current_other_tabs(cx);
            }))
            // Go to specific tabs
            .on_action(cx.listener(|this, _: &GoToTab1, _window, cx| {
                this.go_to_tab(0, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToTab2, _window, cx| {
                this.go_to_tab(1, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToTab3, _window, cx| {
                this.go_to_tab(2, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToTab4, _window, cx| {
                this.go_to_tab(3, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToTab5, _window, cx| {
                this.go_to_tab(4, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToTab6, _window, cx| {
                this.go_to_tab(5, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToTab7, _window, cx| {
                this.go_to_tab(6, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToTab8, _window, cx| {
                this.go_to_tab(7, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToLastTab, _window, cx| {
                this.go_to_last_tab(cx);
            }))
            // UI toggles
            .on_action(cx.listener(|this, _: &ToggleSidebar, _window, cx| {
                this.toggle_sidebar(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleCommandPalette, window, cx| {
                this.toggle_command_palette(window, cx);
            }))
            // Request panel tabs
            .on_action(cx.listener(|this, _: &SwitchToBodyTab, _window, cx| {
                this.switch_to_request_tab(crate::views::request_view::RequestTab::Body, cx);
            }))
            .on_action(cx.listener(|this, _: &SwitchToParamsTab, _window, cx| {
                this.switch_to_request_tab(crate::views::request_view::RequestTab::Params, cx);
            }))
            .on_action(cx.listener(|this, _: &SwitchToHeadersTab, _window, cx| {
                this.switch_to_request_tab(crate::views::request_view::RequestTab::Headers, cx);
            }))
            .on_action(cx.listener(|this, _: &SwitchToAuthTab, _window, cx| {
                this.switch_to_request_tab(crate::views::request_view::RequestTab::Auth, cx);
            }))
            // Response panel tabs
            .on_action(cx.listener(|this, _: &SwitchToResponseBody, _window, cx| {
                this.switch_to_response_tab(crate::views::response_view::ResponseTab::Body, cx);
            }))
            .on_action(
                cx.listener(|this, _: &SwitchToResponseHeaders, _window, cx| {
                    this.switch_to_response_tab(
                        crate::views::response_view::ResponseTab::Headers,
                        cx,
                    );
                }),
            )
            // Focus URL bar
            .on_action(cx.listener(|this, _: &FocusUrlBar, window, cx| {
                this.focus_url_bar(window, cx);
            }))
            // HTTP method shortcuts
            .on_action(cx.listener(|this, _: &SetMethodGet, _window, cx| {
                this.set_method(HttpMethod::Get, cx);
            }))
            .on_action(cx.listener(|this, _: &SetMethodPost, _window, cx| {
                this.set_method(HttpMethod::Post, cx);
            }))
            .on_action(cx.listener(|this, _: &SetMethodPut, _window, cx| {
                this.set_method(HttpMethod::Put, cx);
            }))
            .on_action(cx.listener(|this, _: &SetMethodDelete, _window, cx| {
                this.set_method(HttpMethod::Delete, cx);
            }))
            .on_action(cx.listener(|this, _: &SetMethodPatch, _window, cx| {
                this.set_method(HttpMethod::Patch, cx);
            }))
            .on_action(cx.listener(|this, _: &SetMethodHead, _window, cx| {
                this.set_method(HttpMethod::Head, cx);
            }))
            .on_action(cx.listener(|this, _: &SetMethodOptions, _window, cx| {
                this.set_method(HttpMethod::Options, cx);
            }))
            // Sidebar
            .when(self.sidebar_visible, |el| {
                let history = self.history.clone();
                let collections = self.collections.clone();
                let history_search = self
                    .history_search
                    .clone()
                    .expect("history_search should be initialized");
                let collections_search = self
                    .collections_search
                    .clone()
                    .expect("collections_search should be initialized");
                let sidebar_tab = self.sidebar_tab;
                let history_filter = self.history_filter;
                let history_group_by = self.history_group_by;

                let this_for_tab = this.clone();
                let this_for_load_history = this.clone();
                let this_for_delete_history = this.clone();
                let this_for_toggle_star = this.clone();
                let this_for_clear_history = this.clone();
                let this_for_load_collection = this.clone();
                let this_for_delete_collection = this.clone();
                let this_for_delete_item = this.clone();
                let this_for_new_collection = this.clone();
                let this_for_toggle_expand = this.clone();
                let this_for_filter_change = this.clone();
                let this_for_group_by_change = this.clone();

                el.child(
                    div().w(px(300.0)).h_full().flex_shrink_0().child(
                        AppSidebar::new(history, collections, history_search, collections_search)
                            .active_tab(sidebar_tab)
                            .history_filter(history_filter)
                            .history_group_by(history_group_by)
                            .on_tab_change(move |tab, _window, cx| {
                                this_for_tab.update(cx, |view, cx| {
                                    view.set_sidebar_tab(tab, cx);
                                });
                            })
                            .on_filter_change(move |filter, _window, cx| {
                                this_for_filter_change.update(cx, |view, cx| {
                                    view.set_history_filter(filter, cx);
                                });
                            })
                            .on_group_by_change(move |group_by, _window, cx| {
                                this_for_group_by_change.update(cx, |view, cx| {
                                    view.set_history_group_by(group_by, cx);
                                });
                            })
                            .on_load_history_request(move |entry_id, window, cx| {
                                this_for_load_history.update(cx, |view, cx| {
                                    view.load_history_entry(entry_id, window, cx);
                                });
                            })
                            .on_delete_history_entry(move |entry_id, _window, cx| {
                                this_for_delete_history.update(cx, |view, cx| {
                                    view.delete_history_entry(entry_id, cx);
                                });
                            })
                            .on_toggle_star(move |entry_id, _window, cx| {
                                this_for_toggle_star.update(cx, |view, cx| {
                                    view.toggle_history_star(entry_id, cx);
                                });
                            })
                            .on_clear_history(move |_window, cx| {
                                this_for_clear_history.update(cx, |view, cx| {
                                    view.clear_history(cx);
                                });
                            })
                            .on_load_collection_request(
                                move |collection_id, item_id, window, cx| {
                                    this_for_load_collection.update(cx, |view, cx| {
                                        view.load_collection_item(
                                            collection_id,
                                            item_id,
                                            window,
                                            cx,
                                        );
                                    });
                                },
                            )
                            .on_delete_collection(move |collection_id, _window, cx| {
                                this_for_delete_collection.update(cx, |view, cx| {
                                    view.delete_collection(collection_id, cx);
                                });
                            })
                            .on_delete_collection_item(
                                move |collection_id, item_id, _window, cx| {
                                    this_for_delete_item.update(cx, |view, cx| {
                                        view.delete_collection_item(collection_id, item_id, cx);
                                    });
                                },
                            )
                            .on_new_collection(move |_window, cx| {
                                this_for_new_collection.update(cx, |view, cx| {
                                    view.create_new_collection(cx);
                                });
                            })
                            .on_toggle_collection_expand(move |collection_id, _window, cx| {
                                this_for_toggle_expand.update(cx, |view, cx| {
                                    view.toggle_collection_expand(collection_id, cx);
                                });
                            }),
                    ),
                )
            })
            // Main content area
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
                                        .size(px(400.0))
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
                                                    request_entity,
                                                    is_loading,
                                                    this_for_send,
                                                    request_view,
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
                                                .child(response_view.clone()),
                                        ),
                                ),
                        ),
                    )
                    // Bottom shortcuts bar
                    .child(self.render_shortcuts(&theme)),
            )
            // Command palette overlay
            .child(self.command_palette.clone())
            // Dialog layer - renders dialogs on top of everything
            .children(Root::render_dialog_layer(window, cx))
            // Notification layer - renders notifications on top of everything
            .children(Root::render_notification_layer(window, cx))
            .into_any_element()
    }
}

impl MainView {
    fn render_request_panel(
        &self,
        url_input: Option<Entity<InputState>>,
        method_dropdown: Entity<MethodDropdownState>,
        request: Entity<RequestEntity>,
        is_loading: bool,
        this: Entity<MainView>,
        request_view: Entity<RequestView>,
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
                                .method_dropdown(method_dropdown, request)
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
            .child(request_view.clone())
    }

    fn render_header(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(44.0))
            .px(px(16.0))
            .border_b_1()
            .border_color(theme.border)
            // Left: Logo + Protocol selector
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_color(theme.primary)
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_size(px(14.0))
                            .child("setu"),
                    )
                    .child(ProtocolSelector::new(ProtocolType::Rest)),
            )
    }

    fn render_shortcuts(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .h(px(32.0))
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .children(
                [
                    ("Send", "⌘↵"),
                    ("New", "⌘N"),
                    ("Close", "⌘W"),
                    ("Next Tab", "⌃⇥"),
                    ("URL", "⌘L"),
                    ("Sidebar", "⌘B"),
                    ("Commands", "⌘K"),
                ]
                .into_iter()
                .map(|(label, shortcut)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .text_size(px(10.0))
                                .child(label),
                        )
                        .child(
                            div()
                                .px(px(4.0))
                                .py(px(1.0))
                                .bg(theme.muted)
                                .rounded(px(2.0))
                                .text_color(theme.muted_foreground)
                                .text_size(px(9.0))
                                .child(shortcut),
                        )
                }),
            )
    }
}
