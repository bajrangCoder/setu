use gpui::prelude::*;
use gpui::{
    div, px, App, Entity, FocusHandle, Focusable, IntoElement, PathPromptOptions, Render,
    ScrollHandle, SharedString, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::notification::NotificationType;
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_component::scroll::ScrollableElement;
use gpui_component::select::{Select, SelectItem, SelectState};
use gpui_component::v_flex;
use gpui_component::Root;
use gpui_component::Sizable;
use gpui_component::WindowExt;
use gpui_component::{ActiveTheme, Icon};
use uuid::Uuid;

use crate::actions::*;
use crate::components::{
    AppSidebar, BodyType, HistoryFilter, HistoryGroupBy, MethodDropdownState, ProtocolSelector,
    ProtocolType, SidebarTab, TabBar, TabInfo, UrlBar,
};
use crate::entities::{
    CollectionDestination, CollectionDestinationEntry, CollectionsEntity, Header, HistoryEntity,
    HttpMethod, RequestBody, RequestData, RequestEntity, ResponseData, ResponseEntity,
};
use crate::http::{HttpClient, InFlightRequest};
use crate::icons::IconName;
use crate::importers::{ImportRegistry, ImportWarning};
use crate::views::request_view::RequestView;
use crate::views::response_view::ResponseView;
use crate::views::{CommandId, CommandPaletteEvent, CommandPaletteView};

#[derive(Clone)]
struct SidebarResizeDrag;

impl Render for SidebarResizeDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(4.0)).h_full()
    }
}

pub struct TabState {
    pub id: usize,
    pub name: String,
    pub is_custom_name: bool,
    pub request: Entity<RequestEntity>,
    pub response: Entity<ResponseEntity>,
    pub url_input: Option<Entity<InputState>>,
    pub method_dropdown: Entity<MethodDropdownState>,
    pub request_view: Entity<RequestView>,
    pub response_view: Entity<ResponseView>,
    pub in_flight_request: Option<InFlightRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestResponseLayout {
    Stacked,
    SideBySide,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DestinationOption {
    destination: CollectionDestination,
    label: String,
}

impl SelectItem for DestinationOption {
    type Value = DestinationOption;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

#[derive(Clone, Copy, Debug)]
enum RenameTarget {
    Collection(Uuid),
    Node { collection_id: Uuid, node_id: Uuid },
}

#[derive(Clone, Debug)]
struct ImportSummary {
    provider: &'static str,
    collection_name: String,
    folder_count: usize,
    request_count: usize,
    warnings: Vec<ImportWarning>,
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
    sidebar_width: f32,
    sidebar_tab: SidebarTab,
    history_search: Option<Entity<InputState>>,
    collections_search: Option<Entity<InputState>>,
    history_filter: HistoryFilter,
    history_group_by: HistoryGroupBy,
    request_response_layout: RequestResponseLayout,
    focus_handle: FocusHandle,
    pending_window_command: Option<CommandId>,
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
            name: "New Request".to_string(),
            is_custom_name: false,
            request: request.clone(),
            response: response.clone(),
            url_input: None,
            method_dropdown,
            request_view,
            response_view,
            in_flight_request: None,
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
            sidebar_width: 300.0,
            sidebar_tab: SidebarTab::History,
            history_search: None,
            collections_search: None,
            history_filter: HistoryFilter::All,
            history_group_by: HistoryGroupBy::Time,
            request_response_layout: RequestResponseLayout::Stacked,
            focus_handle: cx.focus_handle(),
            pending_window_command: None,
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
            is_custom_name: true,
            request: request.clone(),
            response: response.clone(),
            url_input: Some(url_input),
            method_dropdown,
            request_view,
            response_view,
            in_flight_request: None,
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
        node_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // First extract all data we need, then drop the borrow
        let item_data = {
            let collections = self.collections.read(cx);
            collections
                .get_request_node(collection_id, node_id)
                .map(|node| (node.request.clone(), node.display_name()))
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
            is_custom_name: true,
            request: request.clone(),
            response: response.clone(),
            url_input: Some(url_input),
            method_dropdown,
            request_view,
            response_view,
            in_flight_request: None,
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
    pub fn delete_collection_node(
        &mut self,
        collection_id: Uuid,
        node_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        self.collections.update(cx, |collections, cx| {
            collections.remove_node(collection_id, node_id, cx);
        });
    }

    /// Toggle collection expanded state
    pub fn toggle_collection_expand(&mut self, collection_id: Uuid, cx: &mut Context<Self>) {
        self.collections.update(cx, |collections, cx| {
            collections.toggle_collection_expanded(collection_id, cx);
        });
    }

    pub fn toggle_collection_node_expand(
        &mut self,
        collection_id: Uuid,
        node_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        self.collections.update(cx, |collections, cx| {
            collections.toggle_node_expanded(collection_id, node_id, cx);
        });
    }

    pub fn create_collection_folder(
        &mut self,
        collection_id: Uuid,
        parent_folder_id: Option<Uuid>,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        self.collections.update(cx, |collections, cx| {
            collections.create_folder(collection_id, parent_folder_id, name, cx);
        });
    }

    pub fn save_request_to_destination(
        &mut self,
        destination: CollectionDestination,
        request_name: String,
        request_data: RequestData,
        cx: &mut Context<Self>,
    ) {
        let mut request_data = request_data;
        request_data.name = request_name;
        self.collections.update(cx, |collections, cx| {
            collections.add_request_node(
                destination.collection_id,
                destination.folder_id,
                request_data,
                cx,
            );
        });
    }

    fn build_active_request_snapshot(&mut self, cx: &mut Context<Self>) -> Option<RequestData> {
        self.build_request_snapshot_for_tab(self.active_tab_index, cx)
    }

    fn build_request_snapshot_for_tab(
        &mut self,
        tab_index: usize,
        cx: &mut Context<Self>,
    ) -> Option<RequestData> {
        let (tab_name, is_custom_name, request_entity, request_view, url_input) = {
            let tab = self.tabs.get(tab_index)?;
            (
                tab.name.clone(),
                tab.is_custom_name,
                tab.request.clone(),
                tab.request_view.clone(),
                tab.url_input.clone(),
            )
        };

        request_view.update(cx, |view, cx| {
            view.sync_body_to_request(cx);
            view.sync_headers_to_request(cx);
        });

        let query_string = request_view.read(cx).get_query_string(cx);
        let base_url = if let Some(ref input) = url_input {
            input.read(cx).text().to_string()
        } else {
            request_entity.read(cx).url().to_string()
        };
        let final_url = Self::compose_request_url(base_url, query_string);

        request_entity.update(cx, |request, cx| {
            request.set_url(final_url.clone(), cx);
        });

        let request = request_entity.read(cx);
        let snapshot_name = if is_custom_name {
            tab_name
        } else {
            Self::derive_request_display_name(request.method(), &final_url)
        };
        Some(RequestData {
            id: Uuid::new_v4(),
            name: snapshot_name,
            url: final_url,
            method: request.method(),
            headers: request.headers().to_vec(),
            body: request.body().clone(),
            is_sending: false,
        })
    }

    fn compose_request_url(base_url: String, query_string: String) -> String {
        if query_string.is_empty() {
            return base_url;
        }

        if base_url.contains('?') {
            format!("{}&{}", base_url, &query_string[1..])
        } else {
            format!("{base_url}{query_string}")
        }
    }

    fn extract_url_path(url: &str) -> Option<String> {
        let url = url.trim();
        if url.is_empty() {
            return None;
        }

        let path = if let Some(after_scheme) = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
        {
            after_scheme
                .find('/')
                .map(|i| &after_scheme[i..])
                .unwrap_or("/")
        } else if url.starts_with('/') {
            url
        } else {
            url.find('/').map(|i| &url[i..]).unwrap_or(url)
        };

        let path = path.split('?').next().unwrap_or(path);

        if path.len() > 32 {
            Some(path[..32].to_string())
        } else {
            Some(path.to_string())
        }
    }

    /// Tab display name — just the path, since the tab bar already shows a method badge
    fn derive_tab_name(url: &str) -> String {
        Self::extract_url_path(url).unwrap_or_else(|| "New Request".to_string())
    }

    /// Full display name for history entries and saved requests — includes method prefix
    fn derive_request_display_name(method: HttpMethod, url: &str) -> String {
        match Self::extract_url_path(url) {
            Some(path) => format!("{} {}", method.as_str(), path),
            None => "New Request".to_string(),
        }
    }

    fn destination_options(entries: Vec<CollectionDestinationEntry>) -> Vec<DestinationOption> {
        entries
            .into_iter()
            .map(|entry| DestinationOption {
                destination: entry.destination,
                label: entry.label,
            })
            .collect()
    }

    fn active_tab(&self) -> Option<&TabState> {
        self.tabs.get(self.active_tab_index)
    }

    fn cancel_in_flight_for_tab(&mut self, index: usize) {
        if let Some(tab) = self.tabs.get_mut(index) {
            if let Some(mut in_flight) = tab.in_flight_request.take() {
                let _ = in_flight.cancel();
            }
        }
    }

    fn cancel_in_flight_for_all_tabs(&mut self) {
        for tab in &mut self.tabs {
            if let Some(mut in_flight) = tab.in_flight_request.take() {
                let _ = in_flight.cancel();
            }
        }
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
            name: "New Request".to_string(),
            is_custom_name: false,
            request: request.clone(),
            response: response.clone(),
            url_input: None,
            method_dropdown,
            request_view,
            response_view,
            in_flight_request: None,
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
            self.cancel_in_flight_for_tab(index);
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

    /// Rename a tab (marks as custom since user explicitly renamed)
    pub fn rename_tab(&mut self, index: usize, new_name: String, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.tabs[index].name = new_name;
            self.tabs[index].is_custom_name = true;
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

    fn rename_collection_target(
        &mut self,
        target: RenameTarget,
        new_name: String,
        cx: &mut Context<Self>,
    ) {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return;
        }

        match target {
            RenameTarget::Collection(collection_id) => {
                self.collections.update(cx, |collections, cx| {
                    collections.rename_collection(collection_id, new_name, cx);
                });
            }
            RenameTarget::Node {
                collection_id,
                node_id,
            } => {
                self.collections.update(cx, |collections, cx| {
                    collections.rename_node(collection_id, node_id, new_name, cx);
                });
            }
        }
    }

    fn show_collection_rename_dialog_internal(
        &mut self,
        target: RenameTarget,
        title: &'static str,
        prompt: &'static str,
        current_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity().clone();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(&current_name));

        cx.subscribe_in(&input, window, move |view, state, event, window, cx| {
            use gpui_component::input::InputEvent;
            if let InputEvent::PressEnter { .. } = event {
                view.rename_collection_target(target, state.read(cx).text().to_string(), cx);
                window.close_dialog(cx);
            }
        })
        .detach();

        let input_for_footer = input.clone();
        let this_for_footer = this.clone();

        window.open_dialog(cx, move |dialog, _, _| {
            let input_for_buttons = input_for_footer.clone();
            let this_for_buttons = this_for_footer.clone();

            dialog
                .title(title)
                .child(v_flex().gap_3().child(prompt).child(Input::new(&input)))
                .footer({
                    let input_submit = input_for_buttons.clone();
                    let this_submit = this_for_buttons.clone();

                    move |_, _, _, _| {
                        let input_click = input_submit.clone();
                        let this_click = this_submit.clone();

                        vec![
                            Button::new("rename-collection-target-submit")
                                .primary()
                                .label("Rename")
                                .on_click(move |_, window, cx| {
                                    let new_name = input_click.read(cx).text().to_string();
                                    this_click.update(cx, |view, cx| {
                                        view.rename_collection_target(target, new_name, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                            Button::new("rename-collection-target-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                }),
                        ]
                    }
                })
        });
    }

    pub fn show_rename_collection_dialog(
        &mut self,
        collection_id: Uuid,
        current_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_collection_rename_dialog_internal(
            RenameTarget::Collection(collection_id),
            "Rename Collection",
            "Enter a new name for this collection:",
            current_name,
            window,
            cx,
        );
    }

    pub fn show_rename_collection_node_dialog(
        &mut self,
        collection_id: Uuid,
        node_id: Uuid,
        current_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_collection_rename_dialog_internal(
            RenameTarget::Node {
                collection_id,
                node_id,
            },
            "Rename Item",
            "Enter a new name:",
            current_name,
            window,
            cx,
        );
    }

    pub fn show_new_folder_dialog(
        &mut self,
        collection_id: Uuid,
        parent_folder_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity().clone();
        let input = cx.new(|cx| InputState::new(window, cx).default_value("New Folder"));

        cx.subscribe_in(&input, window, move |view, state, event, window, cx| {
            use gpui_component::input::InputEvent;
            if let InputEvent::PressEnter { .. } = event {
                let name = state.read(cx).text().to_string().trim().to_string();
                if !name.is_empty() {
                    view.create_collection_folder(collection_id, parent_folder_id, &name, cx);
                }
                window.close_dialog(cx);
            }
        })
        .detach();

        let input_for_footer = input.clone();
        let this_for_footer = this.clone();

        window.open_dialog(cx, move |dialog, _, _| {
            let input_for_buttons = input_for_footer.clone();
            let this_for_buttons = this_for_footer.clone();

            dialog
                .title("New Folder")
                .child(
                    v_flex()
                        .gap_3()
                        .child("Enter a name for the new folder:")
                        .child(Input::new(&input)),
                )
                .footer({
                    let input_submit = input_for_buttons.clone();
                    let this_submit = this_for_buttons.clone();

                    move |_, _, _, _| {
                        let input_click = input_submit.clone();
                        let this_click = this_submit.clone();

                        vec![
                            Button::new("new-folder-submit")
                                .primary()
                                .label("Create")
                                .on_click(move |_, window, cx| {
                                    let name =
                                        input_click.read(cx).text().to_string().trim().to_string();
                                    if !name.is_empty() {
                                        this_click.update(cx, |view, cx| {
                                            view.create_collection_folder(
                                                collection_id,
                                                parent_folder_id,
                                                &name,
                                                cx,
                                            );
                                        });
                                    }
                                    window.close_dialog(cx);
                                }),
                            Button::new("new-folder-cancel").label("Cancel").on_click(
                                |_, window, cx| {
                                    window.close_dialog(cx);
                                },
                            ),
                        ]
                    }
                })
        });
    }

    pub fn show_move_collection_node_dialog(
        &mut self,
        source_collection_id: Uuid,
        node_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let options = {
            let collections = self.collections.read(cx);
            Self::destination_options(
                collections.move_destinations_for_node(source_collection_id, node_id),
            )
        };

        if options.is_empty() {
            window.push_notification(
                (
                    NotificationType::Warning,
                    "No valid destinations are available for this item.",
                ),
                cx,
            );
            return;
        }

        let select_state = cx.new(|cx| {
            SelectState::new(
                options.clone(),
                Some(gpui_component::IndexPath::new(0)),
                window,
                cx,
            )
        });
        let this = cx.entity().clone();

        window.open_dialog(cx, move |dialog, _, _| {
            let select_for_footer = select_state.clone();
            let this_for_footer = this.clone();

            dialog
                .title("Move Item")
                .child(
                    v_flex()
                        .gap_3()
                        .child("Choose a new destination:")
                        .child(Select::new(&select_state).menu_width(px(360.0))),
                )
                .footer(move |_, _, _, _| {
                    let select_click = select_for_footer.clone();
                    let this_click = this_for_footer.clone();

                    vec![
                        Button::new("move-node-submit")
                            .primary()
                            .label("Move")
                            .on_click(move |_, window, cx| {
                                let Some(selection) =
                                    select_click.read(cx).selected_value().cloned()
                                else {
                                    return;
                                };

                                let result = this_click.update(cx, |view, cx| {
                                    view.collections.update(cx, |collections, cx| {
                                        collections.move_node(
                                            source_collection_id,
                                            node_id,
                                            selection.destination.collection_id,
                                            selection.destination.folder_id,
                                            cx,
                                        )
                                    })
                                });

                                match result {
                                    Ok(()) => window.close_dialog(cx),
                                    Err(error) => window.push_notification(
                                        (
                                            NotificationType::Error,
                                            SharedString::from(error.to_string()),
                                        ),
                                        cx,
                                    ),
                                }
                            }),
                        Button::new("move-node-cancel").label("Cancel").on_click(
                            |_, window, cx| {
                                window.close_dialog(cx);
                            },
                        ),
                    ]
                })
        });
    }

    pub fn show_save_to_collection_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(request_data) = self.build_active_request_snapshot(cx) else {
            window.push_notification(
                (NotificationType::Warning, "No active request to save."),
                cx,
            );
            return;
        };

        let destination_options = {
            let collections = self.collections.read(cx);
            Self::destination_options(collections.destination_entries())
        };

        let this = cx.entity().clone();
        let request_name_input =
            cx.new(|cx| InputState::new(window, cx).default_value(&request_data.name));

        if destination_options.is_empty() {
            let collection_name_input =
                cx.new(|cx| InputState::new(window, cx).default_value("New Collection"));
            let request_name_for_footer = request_name_input.clone();
            let collection_name_for_footer = collection_name_input.clone();
            let this_for_footer = this.clone();
            let request_for_footer = request_data.clone();

            window.open_dialog(cx, move |dialog, _, _| {
                let request_name_for_buttons = request_name_for_footer.clone();
                let collection_name_for_buttons = collection_name_for_footer.clone();
                let this_for_buttons = this_for_footer.clone();
                let request_for_buttons = request_for_footer.clone();

                dialog
                    .title("Save to Collection")
                    .child(
                        v_flex()
                            .gap_3()
                            .child("No collections exist yet, so a new collection will be created.")
                            .child("Request name")
                            .child(Input::new(&request_name_input))
                            .child("Collection name")
                            .child(Input::new(&collection_name_input)),
                    )
                    .footer(move |_, _, _, _| {
                        let request_name_click = request_name_for_buttons.clone();
                        let collection_name_click = collection_name_for_buttons.clone();
                        let this_click = this_for_buttons.clone();
                        let request_click = request_for_buttons.clone();

                        vec![
                            Button::new("save-to-new-collection")
                                .primary()
                                .label("Save")
                                .on_click(move |_, window, cx| {
                                    let request_name = request_name_click
                                        .read(cx)
                                        .text()
                                        .to_string()
                                        .trim()
                                        .to_string();
                                    let collection_name = collection_name_click
                                        .read(cx)
                                        .text()
                                        .to_string()
                                        .trim()
                                        .to_string();
                                    let request_name = if request_name.is_empty() {
                                        "New Request".to_string()
                                    } else {
                                        request_name
                                    };
                                    let collection_name = if collection_name.is_empty() {
                                        "New Collection".to_string()
                                    } else {
                                        collection_name
                                    };

                                    this_click.update(cx, |view, cx| {
                                        let collection_id =
                                            view.collections.update(cx, |collections, cx| {
                                                collections.create_collection(&collection_name, cx)
                                            });
                                        view.save_request_to_destination(
                                            CollectionDestination {
                                                collection_id,
                                                folder_id: None,
                                            },
                                            request_name,
                                            request_click.clone(),
                                            cx,
                                        );
                                    });
                                    window.close_dialog(cx);
                                }),
                            Button::new("save-to-new-collection-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                }),
                        ]
                    })
            });
            return;
        }

        let destination_select = cx.new(|cx| {
            SelectState::new(
                destination_options.clone(),
                Some(gpui_component::IndexPath::new(0)),
                window,
                cx,
            )
        });
        let request_name_for_footer = request_name_input.clone();
        let destination_for_footer = destination_select.clone();
        let this_for_footer = this.clone();
        let request_for_footer = request_data.clone();

        window.open_dialog(cx, move |dialog, _, _| {
            let request_name_for_buttons = request_name_for_footer.clone();
            let destination_for_buttons = destination_for_footer.clone();
            let this_for_buttons = this_for_footer.clone();
            let request_for_buttons = request_for_footer.clone();

            dialog
                .title("Save to Collection")
                .child(
                    v_flex()
                        .gap_3()
                        .child("Request name")
                        .child(Input::new(&request_name_input))
                        .child("Destination")
                        .child(Select::new(&destination_select).menu_width(px(360.0))),
                )
                .footer(move |_, _, _, _| {
                    let request_name_click = request_name_for_buttons.clone();
                    let destination_click = destination_for_buttons.clone();
                    let this_click = this_for_buttons.clone();
                    let request_click = request_for_buttons.clone();

                    vec![
                        Button::new("save-to-collection-submit")
                            .primary()
                            .label("Save")
                            .on_click(move |_, window, cx| {
                                let Some(selection) =
                                    destination_click.read(cx).selected_value().cloned()
                                else {
                                    return;
                                };

                                let request_name = request_name_click
                                    .read(cx)
                                    .text()
                                    .to_string()
                                    .trim()
                                    .to_string();
                                let request_name = if request_name.is_empty() {
                                    "New Request".to_string()
                                } else {
                                    request_name
                                };

                                this_click.update(cx, |view, cx| {
                                    view.save_request_to_destination(
                                        selection.destination,
                                        request_name,
                                        request_click.clone(),
                                        cx,
                                    );
                                });
                                window.close_dialog(cx);
                            }),
                        Button::new("save-to-collection-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                    ]
                })
        });
    }

    pub fn import_collection_from_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().clone();
        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select collection file to import".into()),
        };
        let paths_receiver = cx.prompt_for_paths(options);

        cx.spawn_in(window, async move |_weak_this, cx| {
            let Ok(platform_result) = paths_receiver.await else {
                log::error!("Collection import file picker channel closed unexpectedly");
                return;
            };

            let Ok(paths_opt) = platform_result else {
                let _ = cx.update(|window, app| {
                    window.push_notification(
                        (NotificationType::Error, "Failed to open file picker"),
                        app,
                    );
                });
                return;
            };

            let Some(paths) = paths_opt else {
                return;
            };
            let Some(path) = paths.first().cloned() else {
                return;
            };

            let result = ImportRegistry::default().import_file(&path);
            let _ = cx.update(|window, app| match result {
                Ok(result) => {
                    let summary = ImportSummary {
                        provider: result.provider,
                        collection_name: result.collection.name.clone(),
                        folder_count: result.collection.folder_count(),
                        request_count: result.collection.request_count(),
                        warnings: result.warnings.clone(),
                    };

                    this.update(app, |view, cx| {
                        view.collections.update(cx, |collections, cx| {
                            collections.import_collection(result.collection, cx);
                        });
                        view.show_import_summary_dialog(summary, window, cx);
                    });
                }
                Err(error) => {
                    window.push_notification(
                        (
                            NotificationType::Error,
                            SharedString::from(format!("Import failed: {error}")),
                        ),
                        app,
                    );
                }
            });
        })
        .detach();
    }

    fn show_import_summary_dialog(
        &mut self,
        summary: ImportSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let muted_foreground = cx.theme().muted_foreground;
        window.open_dialog(cx, move |dialog, _, _| {
            let warning_count = summary.warnings.len();
            let warnings = summary.warnings.clone();

            dialog
                .title("Import Summary")
                .child(
                    v_flex()
                        .gap_3()
                        .child(format!("Provider: {}", summary.provider))
                        .child(format!("Collection: {}", summary.collection_name))
                        .child(format!("Folders imported: {}", summary.folder_count))
                        .child(format!("Requests imported: {}", summary.request_count))
                        .child(format!("Warnings: {}", warning_count))
                        .child(if warnings.is_empty() {
                            div()
                                .text_size(px(12.0))
                                .child("No warnings.")
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .max_h(px(220.0))
                                .overflow_y_scrollbar()
                                .children(warnings.into_iter().map(|warning| {
                                    let location =
                                        warning.path.unwrap_or_else(|| "Import".to_string());
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .child(location),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(muted_foreground)
                                                .child(warning.message),
                                        )
                                }))
                                .into_any_element()
                        }),
                )
                .footer(|_, _, _, _| {
                    vec![Button::new("import-summary-close")
                        .primary()
                        .label("Close")
                        .on_click(|_, window, cx| {
                            window.close_dialog(cx);
                        })]
                })
        });
    }

    /// Close all tabs except the one at the given index
    pub fn close_other_tabs(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            for i in 0..self.tabs.len() {
                if i != index {
                    self.cancel_in_flight_for_tab(i);
                }
            }

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
        let tab_index = self.active_tab_index;
        let Some(tab) = self.tabs.get(tab_index) else {
            return;
        };

        let request_entity = tab.request.clone();
        let response_entity = tab.response.clone();
        let request_view = tab.request_view.clone();
        let tab_name = tab.name.clone();

        // Toggle behavior: send when idle, cancel when already sending.
        if request_entity.read(cx).is_sending() {
            self.cancel_request(cx);
            return;
        }

        // Get URL from input state.
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
            name: tab_name,
            url: url.clone(),
            method,
            headers: headers.clone(),
            body: body.clone(),
            is_sending: false,
        };

        let history_entity = self.history.clone();

        // Spawn HTTP request on Tokio runtime and keep a cancel handle on the tab.
        let (result_rx, in_flight_request) =
            self.http_client.spawn_request(method, url, headers, body);
        if let Some(tab) = self.tabs.get_mut(tab_index) {
            tab.in_flight_request = Some(in_flight_request);
        }

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
                        log::info!("Request was cancelled");
                        response_entity.update(app, |resp, cx| {
                            resp.set_error("Request was cancelled".to_string(), cx);
                        });
                    }
                }
            })
        })
        .detach_and_log_err(cx);
    }

    pub fn cancel_request(&mut self, cx: &mut Context<Self>) {
        let tab_index = self.active_tab_index;
        let Some(tab) = self.tabs.get_mut(tab_index) else {
            return;
        };

        let is_sending = tab.request.read(cx).is_sending();
        if !is_sending {
            return;
        }

        if let Some(mut in_flight) = tab.in_flight_request.take() {
            let _ = in_flight.cancel();
        }
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    fn toggle_request_response_layout(&mut self, cx: &mut Context<Self>) {
        let next_layout = match self.request_response_layout {
            RequestResponseLayout::Stacked => RequestResponseLayout::SideBySide,
            RequestResponseLayout::SideBySide => RequestResponseLayout::Stacked,
        };
        self.set_request_response_layout(next_layout, cx);
    }

    fn set_request_response_layout(
        &mut self,
        layout: RequestResponseLayout,
        cx: &mut Context<Self>,
    ) {
        if self.request_response_layout != layout {
            self.request_response_layout = layout;
            cx.notify();
        }
    }

    pub fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_palette.update(cx, |palette, cx| {
            palette.toggle(window, cx);
        });
    }

    pub fn execute_command(&mut self, cmd_id: CommandId, cx: &mut Context<Self>) {
        match cmd_id {
            CommandId::SendRequest => self.send_request(cx),
            CommandId::CancelRequest => self.cancel_request(cx),
            CommandId::NewRequest => self.new_tab(cx),
            CommandId::CloseTab => self.close_current_tab(cx),
            CommandId::CloseAllTabs => self.close_all_tabs(cx),
            CommandId::CloseOtherTabs => self.close_current_other_tabs(cx),
            CommandId::NextTab => self.next_tab(cx),
            CommandId::PreviousTab => self.previous_tab(cx),
            CommandId::GoToTab1 => self.go_to_tab(0, cx),
            CommandId::GoToTab2 => self.go_to_tab(1, cx),
            CommandId::GoToTab3 => self.go_to_tab(2, cx),
            CommandId::GoToTab4 => self.go_to_tab(3, cx),
            CommandId::GoToTab5 => self.go_to_tab(4, cx),
            CommandId::GoToTab6 => self.go_to_tab(5, cx),
            CommandId::GoToTab7 => self.go_to_tab(6, cx),
            CommandId::GoToTab8 => self.go_to_tab(7, cx),
            CommandId::GoToLastTab => self.go_to_last_tab(cx),
            CommandId::ToggleSidebar => self.toggle_sidebar(cx),
            CommandId::ToggleRequestResponseLayout => self.toggle_request_response_layout(cx),
            CommandId::SaveToCollection | CommandId::ImportCollection => {
                self.pending_window_command = Some(cmd_id);
                cx.notify();
            }
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
                self.pending_window_command = Some(cmd_id);
                cx.notify();
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
        self.cancel_in_flight_for_all_tabs();
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
            let old_name = current_tab.name.clone();
            let old_request_view = current_tab.request_view.clone();

            let old_body_type = old_request_view.read(cx).get_body_type();
            let old_method = old_request.read(cx).method();
            let old_headers: Vec<_> = old_request.read(cx).headers().to_vec();
            let old_body = old_request.read(cx).body().clone();
            let new_response = cx.new(|_| ResponseEntity::new());

            let url_text = if let Some(ref old_url) = old_url_input {
                old_url.read(cx).text().to_string()
            } else {
                old_request.read(cx).url().to_string()
            };
            let duplicated_url = url_text.clone();

            let body_content: Option<String> = match &old_body {
                RequestBody::Json(content) | RequestBody::Text(content) => {
                    if content.is_empty() {
                        None
                    } else {
                        Some(content.clone())
                    }
                }
                RequestBody::None
                | RequestBody::FormData(_)
                | RequestBody::MultipartFormData(_) => None,
            };

            let form_data = match &old_body {
                RequestBody::FormData(data) => Some(data.clone()),
                _ => None,
            };

            let multipart_data = match &old_body {
                RequestBody::MultipartFormData(fields) => Some(fields.clone()),
                _ => None,
            };

            let new_request = cx.new(|_| {
                let mut req = RequestEntity::new()
                    .with_method(old_method)
                    .with_headers(old_headers);
                req.data.url = url_text;
                req.data.body = old_body;
                req
            });
            let new_method_dropdown = cx.new(|_| MethodDropdownState::new(old_method));

            let new_url_input = if old_url_input.is_some() {
                Some(cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Enter request URL...")
                        .default_value(&duplicated_url)
                }))
            } else {
                None
            };

            let new_request_view = cx.new(|cx| {
                RequestView::new(new_request.clone(), old_body_type, cx)
                    .with_initial_body_content(body_content)
                    .with_initial_form_data(form_data)
                    .with_initial_multipart_data(multipart_data)
            });
            let new_response_view = cx.new(|cx| ResponseView::new(new_response.clone(), cx));

            self.next_tab_id += 1;
            let new_tab = TabState {
                id: self.next_tab_id,
                name: format!("{} (copy)", old_name),
                is_custom_name: true,
                request: new_request.clone(),
                response: new_response.clone(),
                url_input: new_url_input,
                method_dropdown: new_method_dropdown,
                request_view: new_request_view,
                response_view: new_response_view,
                in_flight_request: None,
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
        if let Some(cmd_id) = self.pending_window_command.take() {
            match cmd_id {
                CommandId::DuplicateRequest => self.duplicate_request(window, cx),
                CommandId::FocusUrlBar => self.focus_url_bar(window, cx),
                CommandId::SaveToCollection => self.show_save_to_collection_dialog(window, cx),
                CommandId::ImportCollection => self.import_collection_from_file(window, cx),
                _ => {}
            }
        }

        // Ensure URL input is initialized for the active tab
        self.ensure_url_input(self.active_tab_index, window, cx);
        // Ensure sidebar search inputs are initialized
        self.ensure_sidebar_inputs(window, cx);

        let theme = cx.theme();

        // Build tab infos — auto-derive names for non-custom tabs
        let tab_infos: Vec<TabInfo> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let method = tab.request.read(cx).method();
                let display_name = if tab.is_custom_name {
                    tab.name.clone()
                } else {
                    let url = tab
                        .url_input
                        .as_ref()
                        .map(|input| input.read(cx).text().to_string())
                        .unwrap_or_default();
                    Self::derive_tab_name(&url)
                };
                let mut info = TabInfo::new(tab.id, i, display_name, method);
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
            .on_action(cx.listener(|this, _: &CancelRequest, _window, cx| {
                this.cancel_request(cx);
            }))
            .on_action(cx.listener(|this, _: &NewRequest, _window, cx| {
                this.new_tab(cx);
            }))
            .on_action(cx.listener(|this, _: &DuplicateRequest, window, cx| {
                this.duplicate_request(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveToCollection, window, cx| {
                this.show_save_to_collection_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ImportCollection, window, cx| {
                this.import_collection_from_file(window, cx);
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
            .on_action(
                cx.listener(|this, _: &ToggleRequestResponseLayout, _window, cx| {
                    this.toggle_request_response_layout(cx);
                }),
            )
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
                let this_for_rename_collection = this.clone();
                let this_for_rename_node = this.clone();
                let this_for_new_collection = this.clone();
                let this_for_import_collection = this.clone();
                let this_for_new_folder = this.clone();
                let this_for_move_node = this.clone();
                let this_for_toggle_expand = this.clone();
                let this_for_toggle_node_expand = this.clone();
                let this_for_filter_change = this.clone();
                let this_for_group_by_change = this.clone();

                el.child(
                    div()
                        .w(px(self.sidebar_width))
                        .h_full()
                        .flex_shrink_0()
                        .child(
                            AppSidebar::new(
                                history,
                                collections,
                                history_search,
                                collections_search,
                            )
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
                            .on_delete_collection_node(
                                move |collection_id, item_id, _window, cx| {
                                    this_for_delete_item.update(cx, |view, cx| {
                                        view.delete_collection_node(collection_id, item_id, cx);
                                    });
                                },
                            )
                            .on_rename_collection(move |collection_id, current_name, window, cx| {
                                this_for_rename_collection.update(cx, |view, cx| {
                                    view.show_rename_collection_dialog(
                                        collection_id,
                                        current_name,
                                        window,
                                        cx,
                                    );
                                });
                            })
                            .on_rename_collection_node(
                                move |collection_id, node_id, current_name, window, cx| {
                                    this_for_rename_node.update(cx, |view, cx| {
                                        view.show_rename_collection_node_dialog(
                                            collection_id,
                                            node_id,
                                            current_name,
                                            window,
                                            cx,
                                        );
                                    });
                                },
                            )
                            .on_new_collection(move |_window, cx| {
                                this_for_new_collection.update(cx, |view, cx| {
                                    view.create_new_collection(cx);
                                });
                            })
                            .on_import_collection(move |window, cx| {
                                this_for_import_collection.update(cx, |view, cx| {
                                    view.import_collection_from_file(window, cx);
                                });
                            })
                            .on_new_folder(move |collection_id, folder_id, window, cx| {
                                this_for_new_folder.update(cx, |view, cx| {
                                    view.show_new_folder_dialog(
                                        collection_id,
                                        folder_id,
                                        window,
                                        cx,
                                    );
                                });
                            })
                            .on_move_collection_node(move |collection_id, node_id, window, cx| {
                                this_for_move_node.update(cx, |view, cx| {
                                    view.show_move_collection_node_dialog(
                                        collection_id,
                                        node_id,
                                        window,
                                        cx,
                                    );
                                });
                            })
                            .on_toggle_collection_expand(move |collection_id, _window, cx| {
                                this_for_toggle_expand.update(cx, |view, cx| {
                                    view.toggle_collection_expand(collection_id, cx);
                                });
                            })
                            .on_toggle_collection_node_expand(
                                move |collection_id, node_id, _window, cx| {
                                    this_for_toggle_node_expand.update(cx, |view, cx| {
                                        view.toggle_collection_node_expand(
                                            collection_id,
                                            node_id,
                                            cx,
                                        );
                                    });
                                },
                            ),
                        ),
                )
            })
            .when(self.sidebar_visible, |el| {
                let border_color = theme.border;
                let this_for_resize = this.clone();
                el.child(
                    div()
                        .id("sidebar-resize-handle")
                        .w(px(5.0))
                        .h_full()
                        .cursor_col_resize()
                        .hover(move |s| s.bg(border_color))
                        .on_drag(SidebarResizeDrag, |_, _, _, cx| {
                            cx.new(|_| SidebarResizeDrag)
                        })
                        .on_drag_move(
                            move |event: &gpui::DragMoveEvent<SidebarResizeDrag>, _window, cx| {
                                let pos: f32 = event.event.position.x.into();
                                let new_width = pos.clamp(200.0, 500.0);
                                this_for_resize.update(cx, |view, cx| {
                                    view.sidebar_width = new_width;
                                    cx.notify();
                                });
                            },
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
                    .child(div().flex_1().flex().flex_col().overflow_hidden().child(
                        self.render_request_response_split(
                            url_input,
                            method_dropdown,
                            request_entity,
                            is_loading,
                            this_for_send,
                            request_view,
                            response_view,
                        ),
                    ))
                    // Bottom shortcuts bar
                    .child(self.render_shortcuts(&theme, this.clone())),
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

#[cfg(test)]
mod tests {
    use super::MainView;

    #[test]
    fn compose_request_url_appends_query_to_plain_url() {
        assert_eq!(
            MainView::compose_request_url(
                "https://api.example.com/users".to_string(),
                "?page=1".to_string()
            ),
            "https://api.example.com/users?page=1"
        );
    }

    #[test]
    fn compose_request_url_merges_with_existing_query() {
        assert_eq!(
            MainView::compose_request_url(
                "https://api.example.com/users?sort=name".to_string(),
                "?page=1".to_string()
            ),
            "https://api.example.com/users?sort=name&page=1"
        );
    }

    #[test]
    fn compose_request_url_preserves_base_when_query_is_empty() {
        assert_eq!(
            MainView::compose_request_url(
                "https://api.example.com/users?sort=name".to_string(),
                String::new()
            ),
            "https://api.example.com/users?sort=name"
        );
    }

    use crate::entities::HttpMethod;

    #[test]
    fn derive_tab_name_with_full_url() {
        assert_eq!(
            MainView::derive_tab_name("https://api.example.com/users"),
            "/users"
        );
    }

    #[test]
    fn derive_tab_name_with_nested_path() {
        assert_eq!(
            MainView::derive_tab_name("https://api.example.com/v2/users/123"),
            "/v2/users/123"
        );
    }

    #[test]
    fn derive_tab_name_empty_url() {
        assert_eq!(MainView::derive_tab_name(""), "New Request");
    }

    #[test]
    fn derive_tab_name_whitespace_url() {
        assert_eq!(MainView::derive_tab_name("   "), "New Request");
    }

    #[test]
    fn derive_tab_name_strips_query_params() {
        assert_eq!(
            MainView::derive_tab_name("https://api.example.com/users?id=5"),
            "/users"
        );
    }

    #[test]
    fn derive_tab_name_domain_only() {
        assert_eq!(MainView::derive_tab_name("https://api.example.com"), "/");
    }

    #[test]
    fn derive_tab_name_no_scheme() {
        assert_eq!(
            MainView::derive_tab_name("api.example.com/items/42"),
            "/items/42"
        );
    }

    #[test]
    fn derive_tab_name_path_only() {
        assert_eq!(MainView::derive_tab_name("/api/resource"), "/api/resource");
    }

    #[test]
    fn derive_request_display_name_includes_method() {
        assert_eq!(
            MainView::derive_request_display_name(
                HttpMethod::Post,
                "https://api.example.com/users"
            ),
            "POST /users"
        );
    }

    #[test]
    fn derive_request_display_name_empty_url() {
        assert_eq!(
            MainView::derive_request_display_name(HttpMethod::Get, ""),
            "New Request"
        );
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
                        let this_for_send = this.clone();
                        let this_for_cancel = this.clone();
                        el.child(
                            UrlBar::new(input)
                                .method_dropdown(method_dropdown, request)
                                .loading(is_loading)
                                .on_send(move |_, _, cx| {
                                    this_for_send.update(cx, |view, cx| {
                                        view.send_request(cx);
                                    });
                                })
                                .on_cancel(move |_, _, cx| {
                                    this_for_cancel.update(cx, |view, cx| {
                                        view.cancel_request(cx);
                                    });
                                })
                                .on_save_to_collection({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        this.update(cx, |view, cx| {
                                            view.show_save_to_collection_dialog(window, cx);
                                        });
                                    }
                                }),
                        )
                    }),
            )
            // Request view tabs (body, headers, etc.)
            .child(request_view.clone())
    }

    fn render_request_response_split(
        &self,
        url_input: Option<Entity<InputState>>,
        method_dropdown: Entity<MethodDropdownState>,
        request: Entity<RequestEntity>,
        is_loading: bool,
        this: Entity<MainView>,
        request_view: Entity<RequestView>,
        response_view: Entity<ResponseView>,
    ) -> impl IntoElement {
        let request_panel = resizable_panel()
            .size(match self.request_response_layout {
                RequestResponseLayout::Stacked => px(400.0),
                RequestResponseLayout::SideBySide => px(520.0),
            })
            .size_range(
                px(150.0)..match self.request_response_layout {
                    RequestResponseLayout::Stacked => px(600.0),
                    RequestResponseLayout::SideBySide => px(900.0),
                },
            )
            .child(div().flex().flex_col().size_full().overflow_hidden().child(
                self.render_request_panel(
                    url_input,
                    method_dropdown,
                    request,
                    is_loading,
                    this,
                    request_view,
                ),
            ));

        let response_panel = resizable_panel()
            .size_range(px(150.0)..gpui::Pixels::MAX)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .overflow_hidden()
                    .child(response_view),
            );

        match self.request_response_layout {
            RequestResponseLayout::Stacked => v_resizable("request-response-split-stacked")
                .child(request_panel)
                .child(response_panel)
                .into_any_element(),
            RequestResponseLayout::SideBySide => h_resizable("request-response-split-side-by-side")
                .child(request_panel)
                .child(response_panel)
                .into_any_element(),
        }
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

    fn render_shortcuts(
        &self,
        theme: &gpui_component::theme::ThemeColor,
        this: Entity<MainView>,
    ) -> impl IntoElement {
        let (button_id, button_icon, tooltip) = match self.request_response_layout {
            RequestResponseLayout::Stacked => (
                "footer-layout-side-by-side",
                IconName::LayoutSplit,
                "Switch to side by side layout",
            ),
            RequestResponseLayout::SideBySide => (
                "footer-layout-stacked",
                IconName::LayoutStacked,
                "Switch to stacked layout",
            ),
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .h(px(36.0))
            .px(px(12.0))
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .child(
                Button::new("footer-toggle-sidebar")
                    .icon(Icon::new(IconName::PanelLeft).size(px(14.0)))
                    .ghost()
                    .xsmall()
                    .tooltip(if self.sidebar_visible {
                        "Hide sidebar"
                    } else {
                        "Show sidebar"
                    })
                    .on_click({
                        let this = this.clone();
                        move |_, _, cx| {
                            this.update(cx, |view, cx| {
                                view.toggle_sidebar(cx);
                            });
                        }
                    }),
            )
            .child(
                div().flex().items_center().gap(px(12.0)).children(
                    [
                        ("Send", "⌘↵"),
                        ("URL", "⌘L"),
                        ("Tabs", "⌃⇥"),
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
                ),
            )
            .child(
                div().flex().items_center().child(
                    Button::new(button_id)
                        .icon(Icon::new(button_icon).size(px(14.0)))
                        .ghost()
                        .xsmall()
                        .tooltip(tooltip)
                        .on_click(move |_, _, cx| {
                            this.update(cx, |view, cx| {
                                view.toggle_request_response_layout(cx);
                            });
                        }),
                ),
            )
    }
}
