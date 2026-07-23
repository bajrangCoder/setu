use gpui::prelude::*;
use gpui::{
    App, Entity, FocusHandle, Focusable, IntoElement, PathPromptOptions, Render, ScrollHandle,
    SharedString, Styled, Window, div, px,
};
use gpui_component::Root;
use gpui_component::Selectable;
use gpui_component::Sizable;
use gpui_component::WindowExt;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dialog::DialogFooter;
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::notification::NotificationType;
use gpui_component::resizable::{ResizableState, h_resizable, resizable_panel, v_resizable};
use gpui_component::scroll::ScrollableElement;
use gpui_component::select::{Select, SelectItem, SelectState};
use gpui_component::v_flex;
use gpui_component::{ActiveTheme, Icon};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use uuid::Uuid;

use crate::actions::*;
use crate::completion::{CompletionContext, CompletionEngine, configure_completion};
use crate::components::{
    AppSidebar, BodyType, EnvironmentPanel, HistoryFilter, HistoryGroupBy, MethodDropdownState,
    ProtocolSelector, ProtocolType, SidebarTab, TabBar, TabInfo, UrlBar,
};
use crate::entities::{
    CollectionDestination, CollectionDestinationEntry, CollectionsEntity, EnvironmentColor,
    EnvironmentScope, EnvironmentVariable, EnvironmentsEntity, HistoryEntity, HistoryGrouping,
    HistoryRow, HttpMethod, PreferredLayout, RequestBody, RequestData, RequestEntity, RequestEvent,
    ResponseData, ResponseEntity, SidebarLoadState, UiPreferences, UiPreferencesStore,
    WorkspacesEntity,
};
use crate::http::{HttpClient, InFlightRequest};
use crate::icons::IconName;
use crate::importers::{ImportRegistry, ImportWarning, ImportedPayload};
use crate::utils::{close_dialog, open_dialog};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequestGeneration(u64);

impl RequestGeneration {
    fn advance(&mut self) -> Self {
        self.0 = self.0.wrapping_add(1);
        *self
    }
}

pub struct TabState {
    pub id: TabId,
    pub name: String,
    pub is_custom_name: bool,
    pub request: Entity<RequestEntity>,
    pub response: Entity<ResponseEntity>,
    pub url_input: Option<Entity<InputState>>,
    pub method_dropdown: Entity<MethodDropdownState>,
    pub request_view: Entity<RequestView>,
    pub response_view: Entity<ResponseView>,
    pub in_flight_request: Option<InFlightRequest>,
    pub request_generation: RequestGeneration,
    pub collection_id: Option<Uuid>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct EnvironmentScopeOption {
    scope: EnvironmentScope,
    label: String,
}

impl SelectItem for EnvironmentScopeOption {
    type Value = EnvironmentScopeOption;

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
    item_kind: &'static str,
    item_name: String,
    folder_count: Option<usize>,
    request_count: Option<usize>,
    variable_count: usize,
    warnings: Vec<ImportWarning>,
}

/// Main application view
pub struct MainView {
    // Tabs
    tabs: Vec<TabState>,
    active_tab_index: usize,
    next_tab_id: u64,
    tab_scroll_handle: ScrollHandle,

    // Command palette (shared across tabs)
    command_palette: Entity<CommandPaletteView>,

    // Shared state
    history: Entity<HistoryEntity>,
    collections: Entity<CollectionsEntity>,
    environments: Entity<EnvironmentsEntity>,
    workspaces: Entity<WorkspacesEntity>,
    environment_panel: Entity<EnvironmentPanel>,
    completion_engine: CompletionEngine,
    http_client: HttpClient,

    // UI state
    sidebar_visible: bool,
    sidebar_width: f32,
    sidebar_tab: SidebarTab,
    history_search: Option<Entity<InputState>>,
    collections_search: Option<Entity<InputState>>,
    history_filter: HistoryFilter,
    history_group_by: HistoryGroupBy,
    history_rows: Arc<Vec<HistoryRow>>,
    history_rows_initialized: bool,
    history_rows_generation: Arc<AtomicU64>,
    request_response_layout: RequestResponseLayout,
    focus_handle: FocusHandle,
    pending_window_command: Option<CommandId>,
    ui_preferences: UiPreferences,
    ui_preferences_store: UiPreferencesStore,
    stacked_split_state: Entity<ResizableState>,
    side_by_side_split_state: Entity<ResizableState>,
}

impl MainView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Create initial tab
        let request = cx.new(|_| RequestEntity::new());
        let response = cx.new(|_| ResponseEntity::new());
        let method_dropdown = cx.new(|_| MethodDropdownState::new(HttpMethod::Get));
        let focus_handle = cx.focus_handle();
        let command_palette = cx.new(|cx| CommandPaletteView::new(focus_handle.clone(), cx));
        let workspaces = cx.new(|_| WorkspacesEntity::load());
        let active_workspace_id = workspaces.read(cx).active_workspace_id();
        let history = cx.new(|_| HistoryEntity::new_for_workspace(active_workspace_id));
        let collections = cx.new(|_| CollectionsEntity::new_for_workspace(active_workspace_id));
        let environments = cx.new(|_| EnvironmentsEntity::new_for_workspace(active_workspace_id));
        let completion_engine = CompletionEngine::for_environments(environments.clone());
        let completion_engine_for_request = completion_engine.clone();
        let request_view = cx.new(|cx| {
            RequestView::new(request.clone(), BodyType::None, cx)
                .with_completion_engine(completion_engine_for_request)
        });
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));

        let initial_tab = TabState {
            id: TabId(0),
            name: "New Request".to_string(),
            is_custom_name: false,
            request: request.clone(),
            response: response.clone(),
            url_input: None,
            method_dropdown,
            request_view,
            response_view,
            in_flight_request: None,
            request_generation: RequestGeneration::default(),
            collection_id: None,
        };
        let environment_panel =
            cx.new(|cx| EnvironmentPanel::new(environments.clone(), collections.clone(), cx));
        let history_load = HistoryEntity::spawn_storage_load();
        let history_for_load = history.clone();
        cx.spawn(async move |_view, cx| {
            let result = history_load
                .await
                .unwrap_or_else(|_| Err("History loader stopped unexpectedly".to_string()));
            cx.update(|app| {
                history_for_load.update(app, |history, cx| {
                    history.apply_storage_load(result, cx);
                });
            })
        })
        .detach();

        let environments_load = EnvironmentsEntity::spawn_storage_load();
        let environments_for_load = environments.clone();
        cx.spawn(async move |_view, cx| {
            let result = environments_load
                .await
                .unwrap_or_else(|_| Err("Environment loader stopped unexpectedly".to_string()));
            cx.update(|app| {
                environments_for_load.update(app, |environments, cx| {
                    environments.apply_storage_load(result, cx);
                });
            })
        })
        .detach();

        let collections_load = CollectionsEntity::spawn_storage_load();
        let collections_for_load = collections.clone();
        cx.spawn(async move |_view, cx| {
            let result = collections_load
                .await
                .unwrap_or_else(|_| Err("Collections loader stopped unexpectedly".to_string()));
            cx.update(|app| {
                collections_for_load.update(app, |collections, cx| {
                    collections.apply_storage_load(result, cx);
                });
            })
        })
        .detach();
        let (ui_preferences, ui_preferences_store) = UiPreferencesStore::load();
        let stacked_split_state = cx.new(|_| ResizableState::default());
        let side_by_side_split_state = cx.new(|_| ResizableState::default());

        cx.subscribe(&command_palette, |this, _, event, cx| {
            let CommandPaletteEvent::ExecuteCommand(cmd_id) = event;
            this.execute_command(*cmd_id, cx);
        })
        .detach();
        Self::subscribe_request_changes(&request, cx);
        cx.subscribe(&history, |this, _, _event, cx| {
            this.schedule_history_rows(cx);
            cx.notify();
        })
        .detach();
        cx.subscribe(&collections, |_this, _, _event, cx| cx.notify())
            .detach();
        cx.subscribe(&environments, |_this, _, _event, cx| cx.notify())
            .detach();
        cx.subscribe(&workspaces, |_this, _, _event, cx| cx.notify())
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
            environments,
            workspaces,
            environment_panel,
            completion_engine,
            http_client,
            sidebar_visible: ui_preferences.sidebar_visible,
            sidebar_width: ui_preferences.sidebar_width,
            sidebar_tab: SidebarTab::History,
            history_search: None,
            collections_search: None,
            history_filter: HistoryFilter::All,
            history_group_by: HistoryGroupBy::Time,
            history_rows: Arc::new(Vec::new()),
            history_rows_initialized: false,
            history_rows_generation: Arc::new(AtomicU64::new(0)),
            request_response_layout: match ui_preferences.layout {
                PreferredLayout::Stacked => RequestResponseLayout::Stacked,
                PreferredLayout::SideBySide => RequestResponseLayout::SideBySide,
            },
            focus_handle,
            pending_window_command: None,
            ui_preferences,
            ui_preferences_store,
            stacked_split_state,
            side_by_side_split_state,
        }
    }

    fn persist_ui_preferences(&self) {
        self.ui_preferences_store.save(&self.ui_preferences);
    }

    fn subscribe_request_changes(request: &Entity<RequestEntity>, cx: &mut Context<Self>) {
        cx.subscribe(request, |_this, _request, event: &RequestEvent, cx| {
            if matches!(
                event,
                RequestEvent::UrlChanged | RequestEvent::MethodChanged
            ) {
                cx.notify();
            }
        })
        .detach();
    }

    /// Ensure URL input is initialized for a tab
    fn ensure_url_input(&mut self, tab_index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let completion_engine = self.completion_engine.clone();
        if let Some(tab) = self.tabs.get_mut(tab_index)
            && tab.url_input.is_none()
        {
            let url_input = cx.new(|cx| {
                configure_completion(
                    InputState::new(window, cx).placeholder("Enter request URL..."),
                    Some(&completion_engine),
                    CompletionContext::Url,
                )
            });
            let tab_id = tab.id;
            Self::subscribe_url_input(&url_input, tab_id, window, cx);
            tab.url_input = Some(url_input);
        }
    }

    /// Subscribe to URL input changes to detect pasted curl commands.
    fn subscribe_url_input(
        url_input: &Entity<InputState>,
        tab_id: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe_in(url_input, window, move |this, state, event, window, cx| {
            use gpui_component::input::InputEvent;
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let text = state.read(cx).text().to_string();
            if let Some(tab) = this.tabs.iter().find(|tab| tab.id == tab_id) {
                tab.request.update(cx, |request, cx| {
                    request.set_url(text.clone(), cx);
                });
            }
            cx.notify();
            if !crate::utils::looks_like_curl(&text) {
                return;
            }
            match crate::utils::parse_curl(&text) {
                Ok(parsed) => {
                    let url_value = parsed.url.clone();
                    state.update(cx, |s, cx| {
                        s.set_value(url_value, window, cx);
                    });
                    this.apply_parsed_curl_to_tab(tab_id, &parsed, window, cx);
                }
                Err(err) => {
                    log::warn!("Failed to parse curl from URL bar: {}", err);
                }
            }
        })
        .detach();
    }

    /// Apply a parsed curl to the currently-active tab.
    fn apply_parsed_curl_to_tab(
        &mut self,
        tab_id: TabId,
        parsed: &crate::utils::ParsedCurl,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            return;
        };
        let method_dropdown = tab.method_dropdown.clone();
        let request_view = tab.request_view.clone();

        method_dropdown.update(cx, |state, cx| {
            state.set_method(parsed.method, cx);
        });
        request_view.update(cx, |view, cx| {
            view.apply_parsed_curl(parsed, window, cx);
        });
        cx.notify();
    }

    /// Ensure sidebar search inputs are initialized
    fn ensure_sidebar_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history_search.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search history..."));
            cx.subscribe_in(&input, window, |this, _, event, _, cx| {
                if matches!(event, gpui_component::input::InputEvent::Change) {
                    this.schedule_history_rows(cx);
                    cx.notify();
                }
            })
            .detach();
            self.history_search = Some(input);
        }
        if self.collections_search.is_none() {
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Search collections..."));
            cx.subscribe_in(&input, window, |_, _, event, _, cx| {
                if matches!(event, gpui_component::input::InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            self.collections_search = Some(input);
        }
    }

    /// Rebuild the history sidebar snapshot on the Tokio blocking pool.
    ///
    /// The source history is immutable and shared, so scheduling this work only
    /// clones an `Arc` on the UI thread. Rapid search changes are debounced and
    /// stale generations are discarded before and after the worker runs.
    fn schedule_history_rows(&mut self, cx: &mut Context<Self>) {
        let generation = self
            .history_rows_generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);

        let (load_state, snapshot) = {
            let history = self.history.read(cx);
            (history.load_state.clone(), history.rows_snapshot())
        };

        if !matches!(load_state, SidebarLoadState::Ready) {
            self.history_rows = Arc::new(Vec::new());
            self.history_rows_initialized = false;
            return;
        }

        let query = self
            .history_search
            .as_ref()
            .map(|input| input.read(cx).text().to_string())
            .unwrap_or_default();
        let starred_only = self.history_filter == HistoryFilter::Starred;
        let grouping = match self.history_group_by {
            HistoryGroupBy::Time => HistoryGrouping::Time,
            HistoryGroupBy::Url => HistoryGrouping::Url,
        };

        let generation_clock = self.history_rows_generation.clone();
        let worker_generation_clock = generation_clock.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();

        crate::utils::shared_tokio_runtime().spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if worker_generation_clock.load(Ordering::Acquire) != generation {
                return;
            }

            let rows = tokio::task::spawn_blocking(move || {
                snapshot.flattened_rows(&query, starred_only, grouping)
            })
            .await;

            if worker_generation_clock.load(Ordering::Acquire) == generation
                && let Ok(rows) = rows
            {
                let _ = tx.send(Arc::new(rows));
            }
        });

        cx.spawn(async move |view, cx| {
            let result = rx.await;
            cx.update(|app| {
                let _ = view.update(app, |main, cx| {
                    if generation_clock.load(Ordering::Acquire) != generation {
                        return;
                    }

                    match result {
                        Ok(rows) => main.history_rows = rows,
                        Err(error) => {
                            log::error!("History row worker stopped unexpectedly: {error}");
                            main.history_rows = Arc::new(Vec::new());
                        }
                    }
                    main.history_rows_initialized = true;
                    cx.notify();
                });
            })
        })
        .detach();
    }

    /// Set sidebar tab
    pub fn set_sidebar_tab(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        self.sidebar_tab = tab;
        cx.notify();
    }

    pub fn set_history_filter(&mut self, filter: HistoryFilter, cx: &mut Context<Self>) {
        self.history_filter = filter;
        self.schedule_history_rows(cx);
        cx.notify();
    }

    pub fn set_history_group_by(&mut self, group_by: HistoryGroupBy, cx: &mut Context<Self>) {
        self.history_group_by = group_by;
        self.schedule_history_rows(cx);
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
        let completion_engine = self.completion_engine.clone();
        let request_view = cx.new(|cx| {
            RequestView::new(request.clone(), body_type, cx)
                .with_completion_engine(completion_engine)
                .with_initial_body_content(body_content)
                .with_initial_form_data(form_data)
                .with_initial_multipart_data(multipart_data)
        });
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));
        let completion_engine = self.completion_engine.clone();
        let url_input = cx.new(|cx| {
            configure_completion(
                InputState::new(window, cx)
                    .placeholder("Enter request URL...")
                    .default_value(&request_data.url),
                Some(&completion_engine),
                CompletionContext::Url,
            )
        });
        let tab_id = TabId(self.next_tab_id);
        Self::subscribe_url_input(&url_input, tab_id, window, cx);
        Self::subscribe_request_changes(&request, cx);

        let tab = TabState {
            id: tab_id,
            name: tab_name,
            is_custom_name: true,
            request: request.clone(),
            response: response.clone(),
            url_input: Some(url_input),
            method_dropdown,
            request_view,
            response_view,
            in_flight_request: None,
            request_generation: RequestGeneration::default(),
            collection_id: None,
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
        let completion_engine = self.completion_engine.clone();
        let request_view = cx.new(|cx| {
            RequestView::new(request.clone(), body_type, cx)
                .with_completion_engine(completion_engine)
                .with_initial_body_content(body_content)
                .with_initial_form_data(form_data)
                .with_initial_multipart_data(multipart_data)
        });
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));
        let completion_engine = self.completion_engine.clone();
        let url_input = cx.new(|cx| {
            configure_completion(
                InputState::new(window, cx)
                    .placeholder("Enter request URL...")
                    .default_value(&request_data.url),
                Some(&completion_engine),
                CompletionContext::Url,
            )
        });
        let tab_id = TabId(self.next_tab_id);
        Self::subscribe_url_input(&url_input, tab_id, window, cx);
        Self::subscribe_request_changes(&request, cx);

        let tab = TabState {
            id: tab_id,
            name: tab_name,
            is_custom_name: true,
            request: request.clone(),
            response: response.clone(),
            url_input: Some(url_input),
            method_dropdown,
            request_view,
            response_view,
            in_flight_request: None,
            request_generation: RequestGeneration::default(),
            collection_id: Some(collection_id),
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

    fn switch_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if self.workspaces.read(cx).active_workspace_id() == workspace_id {
            return;
        }
        self.cancel_in_flight_for_all_tabs(cx);
        self.collections.update(cx, |collections, cx| {
            collections.set_active_workspace(workspace_id, cx);
        });
        self.history.update(cx, |history, cx| {
            history.set_active_workspace(workspace_id, cx);
        });
        self.environments.update(cx, |environments, cx| {
            environments.set_active_workspace(workspace_id, cx);
        });
        self.workspaces.update(cx, |workspaces, cx| {
            workspaces.set_active_workspace(workspace_id, cx);
        });

        self.tabs.clear();
        self.active_tab_index = 0;
        self.new_tab(cx);
        self.schedule_history_rows(cx);
        cx.notify();
    }

    fn show_new_workspace_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("My Workspace")
                .default_value("New Workspace")
        });
        let input_for_create = input.clone();
        let this = cx.entity().clone();
        open_dialog(window, cx, move |dialog, _, cx| {
            let this_for_create = this.clone();
            let input_for_create = input_for_create.clone();
            dialog
                .title("New Workspace")
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    "Collections, request history, and environments stay isolated inside this workspace.",
                                ),
                        )
                        .child(Input::new(&input)),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("create-workspace-confirm")
                                .label("Create workspace")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let name =
                                        input_for_create.read(cx).text().to_string();
                                    this_for_create.update(cx, |view, cx| {
                                        let workspace_id =
                                            view.workspaces.update(cx, |workspaces, cx| {
                                                workspaces.create_workspace(name, cx)
                                            });
                                        view.switch_workspace(workspace_id, cx);
                                    });
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(
                            Button::new("create-workspace-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| close_dialog(window, cx)),
                        ),
                )
        });
    }

    fn show_rename_workspace_dialog(
        &mut self,
        workspace_id: Uuid,
        current_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Workspace name")
                .default_value(current_name)
        });
        let input_for_rename = input.clone();
        let this = cx.entity().clone();
        open_dialog(window, cx, move |dialog, _, _| {
            let this_for_rename = this.clone();
            let input_for_rename = input_for_rename.clone();
            dialog
                .title("Rename Workspace")
                .child(Input::new(&input))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("rename-workspace-confirm")
                                .label("Rename")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let name = input_for_rename.read(cx).text().to_string();
                                    this_for_rename.update(cx, |view, cx| {
                                        view.workspaces.update(cx, |workspaces, cx| {
                                            workspaces.rename_workspace(workspace_id, name, cx);
                                        });
                                    });
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(
                            Button::new("rename-workspace-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| close_dialog(window, cx)),
                        ),
                )
        });
    }

    fn show_delete_workspace_dialog(
        &mut self,
        workspace_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (name, fallback_id) = {
            let workspaces = self.workspaces.read(cx);
            let Some(workspace) = workspaces
                .workspaces()
                .iter()
                .find(|workspace| workspace.id == workspace_id)
            else {
                return;
            };
            let Some(fallback) = workspaces
                .workspaces()
                .iter()
                .find(|workspace| workspace.id != workspace_id)
            else {
                return;
            };
            (workspace.name.clone(), fallback.id)
        };
        let this = cx.entity().clone();
        open_dialog(window, cx, move |dialog, _, _| {
            let this_for_delete = this.clone();
            dialog
                .title("Delete Workspace")
                .child(format!(
                    "Delete “{name}” and its collections, history, and environments? This cannot be undone."
                ))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("delete-workspace-confirm")
                                .label("Delete workspace")
                                .danger()
                                .on_click(move |_, window, cx| {
                                    this_for_delete.update(cx, |view, cx| {
                                        view.switch_workspace(fallback_id, cx);
                                        view.collections.update(cx, |collections, _| {
                                            collections.remove_workspace(workspace_id);
                                        });
                                        view.history.update(cx, |history, _| {
                                            history.remove_workspace(workspace_id);
                                        });
                                        view.environments.update(cx, |environments, _| {
                                            environments.remove_workspace(workspace_id);
                                        });
                                        view.workspaces.update(cx, |workspaces, cx| {
                                            workspaces.remove_workspace(workspace_id, cx);
                                        });
                                    });
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(
                            Button::new("delete-workspace-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| close_dialog(window, cx)),
                        ),
                )
        });
    }

    pub fn show_new_environment_dialog(
        &mut self,
        project_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut scope_options = vec![
            EnvironmentScopeOption {
                scope: EnvironmentScope::Global,
                label: "Global · Available in every workspace".to_string(),
            },
            EnvironmentScopeOption {
                scope: EnvironmentScope::Workspace,
                label: "Workspace · Available in this workspace".to_string(),
            },
        ];
        scope_options.extend(
            self.collections
                .read(cx)
                .collections
                .iter()
                .map(|collection| EnvironmentScopeOption {
                    scope: EnvironmentScope::Project(collection.id),
                    label: format!("Project · {}", collection.name),
                }),
        );
        let selected_index = project_id
            .and_then(|project_id| {
                scope_options
                    .iter()
                    .position(|option| option.scope == EnvironmentScope::Project(project_id))
            })
            .unwrap_or(1);
        let scope_select = cx.new(|cx| {
            SelectState::new(
                scope_options,
                Some(gpui_component::IndexPath::new(selected_index)),
                window,
                cx,
            )
        });
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Development, Staging, Production…")
                .default_value("Development")
        });
        let this = cx.entity().clone();
        let name_for_footer = name_input.clone();
        let scope_for_footer = scope_select.clone();

        open_dialog(window, cx, move |dialog, _, cx| {
            let this_for_create = this.clone();
            let name_for_create = name_for_footer.clone();
            let scope_for_create = scope_for_footer.clone();

            dialog
                .title("New Environment")
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    "Global environments are shared by every workspace. Workspace and project environments override them with more specific values.",
                                ),
                        )
                        .child("Environment name")
                        .child(Input::new(&name_input))
                        .child("Scope")
                        .child(Select::new(&scope_select).menu_width(px(360.0))),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("create-environment")
                                .label("Create environment")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let name = name_for_create
                                        .read(cx)
                                        .text()
                                        .to_string()
                                        .trim()
                                        .to_string();
                                    let name = if name.is_empty() {
                                        "Development".to_string()
                                    } else {
                                        name
                                    };
                                    let Some(scope) = scope_for_create
                                        .read(cx)
                                        .selected_value()
                                        .map(|option| option.scope)
                                    else {
                                        return;
                                    };
                                    this_for_create.update(cx, |view, cx| {
                                        let environment_id = view.environments.update(
                                            cx,
                                            |environments, cx| {
                                                environments.create_environment(name, scope, cx)
                                            },
                                        );
                                        view.environment_panel.update(cx, |panel, cx| {
                                            panel.select_environment(environment_id, cx);
                                        });
                                    });
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(
                            Button::new("create-environment-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| close_dialog(window, cx)),
                        ),
                )
        });
    }

    pub fn show_delete_environment_dialog(
        &mut self,
        environment_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self
            .environments
            .read(cx)
            .get(environment_id)
            .map(|environment| environment.name.clone())
        else {
            return;
        };
        let this = cx.entity().clone();
        open_dialog(window, cx, move |dialog, _, _| {
            let this_for_delete = this.clone();
            dialog
                .title("Delete Environment")
                .child(format!(
                    "Delete “{name}”? Requests using its variables will stop resolving."
                ))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("delete-environment-confirm")
                                .label("Delete")
                                .danger()
                                .on_click(move |_, window, cx| {
                                    this_for_delete.update(cx, |view, cx| {
                                        view.environments.update(cx, |environments, cx| {
                                            environments.remove_environment(environment_id, cx);
                                        });
                                    });
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(
                            Button::new("delete-environment-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| close_dialog(window, cx)),
                        ),
                )
        });
    }

    pub fn show_rename_environment_dialog(
        &mut self,
        environment_id: Uuid,
        current_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Environment name")
                .default_value(current_name)
        });
        let this = cx.entity().clone();
        let input_for_footer = input.clone();
        open_dialog(window, cx, move |dialog, _, _| {
            let this_for_rename = this.clone();
            let input_for_rename = input_for_footer.clone();
            dialog
                .title("Rename Environment")
                .child(Input::new(&input))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("rename-environment-confirm")
                                .label("Rename")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let name = input_for_rename.read(cx).text().to_string();
                                    this_for_rename.update(cx, |view, cx| {
                                        view.environments.update(cx, |environments, cx| {
                                            environments.rename_environment(
                                                environment_id,
                                                name,
                                                cx,
                                            );
                                        });
                                    });
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(
                            Button::new("rename-environment-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| close_dialog(window, cx)),
                        ),
                )
        });
    }

    /// Delete a collection
    pub fn delete_collection(&mut self, collection_id: Uuid, cx: &mut Context<Self>) {
        self.collections.update(cx, |collections, cx| {
            collections.remove_collection(collection_id, cx);
        });
        self.environments.update(cx, |environments, cx| {
            environments.remove_project_environments(collection_id, cx);
        });
        for tab in &mut self.tabs {
            if tab.collection_id == Some(collection_id) {
                tab.collection_id = None;
            }
        }
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
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.collection_id = Some(destination.collection_id);
        }
        cx.notify();
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

    fn cancel_in_flight_for_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(index)
            && let Some(mut in_flight) = tab.in_flight_request.take()
        {
            let _ = in_flight.cancel();
            tab.request_generation.advance();
            tab.request
                .update(cx, |request, cx| request.set_sending(false, cx));
            tab.response
                .update(cx, |response, cx| response.set_cancelled(cx));
        }
    }

    fn cancel_in_flight_for_all_tabs(&mut self, cx: &mut Context<Self>) {
        for index in 0..self.tabs.len() {
            self.cancel_in_flight_for_tab(index, cx);
        }
    }

    /// Add a new tab
    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let request = cx.new(|_| RequestEntity::new());
        let response = cx.new(|_| ResponseEntity::new());
        let method_dropdown = cx.new(|_| MethodDropdownState::new(HttpMethod::Get));
        let completion_engine = self.completion_engine.clone();
        let request_view = cx.new(|cx| {
            RequestView::new(request.clone(), BodyType::None, cx)
                .with_completion_engine(completion_engine)
        });
        let response_view = cx.new(|cx| ResponseView::new(response.clone(), cx));

        let tab_id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        Self::subscribe_request_changes(&request, cx);
        let tab = TabState {
            id: tab_id,
            name: "New Request".to_string(),
            is_custom_name: false,
            request: request.clone(),
            response: response.clone(),
            url_input: None,
            method_dropdown,
            request_view,
            response_view,
            in_flight_request: None,
            request_generation: RequestGeneration::default(),
            collection_id: None,
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
            self.cancel_in_flight_for_tab(index, cx);
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
                close_dialog(window, cx);
            }
        })
        .detach();

        // Clone for footer button clicks
        let input_for_footer = input.clone();
        let this_for_footer = this.clone();

        open_dialog(window, cx, move |dialog, _, _| {
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
                    let input_click = input_for_buttons.clone();
                    let this_click = this_for_buttons.clone();

                    DialogFooter::new()
                        .child(
                            Button::new("rename-submit")
                                .primary()
                                .label("Rename")
                                .on_click(move |_, window, cx| {
                                    let new_name = input_click.read(cx).text().to_string();
                                    this_click.update(cx, |view, cx| {
                                        view.rename_tab(index, new_name, cx);
                                    });
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(Button::new("rename-cancel").label("Cancel").on_click(
                            |_, window, cx| {
                                close_dialog(window, cx);
                            },
                        ))
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
                close_dialog(window, cx);
            }
        })
        .detach();

        let input_for_footer = input.clone();
        let this_for_footer = this.clone();

        open_dialog(window, cx, move |dialog, _, _| {
            let input_for_buttons = input_for_footer.clone();
            let this_for_buttons = this_for_footer.clone();

            dialog
                .title(title)
                .child(v_flex().gap_3().child(prompt).child(Input::new(&input)))
                .footer({
                    let input_click = input_for_buttons.clone();
                    let this_click = this_for_buttons.clone();

                    DialogFooter::new()
                        .child(
                            Button::new("rename-collection-target-submit")
                                .primary()
                                .label("Rename")
                                .on_click(move |_, window, cx| {
                                    let new_name = input_click.read(cx).text().to_string();
                                    this_click.update(cx, |view, cx| {
                                        view.rename_collection_target(target, new_name, cx);
                                    });
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(
                            Button::new("rename-collection-target-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| {
                                    close_dialog(window, cx);
                                }),
                        )
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
                close_dialog(window, cx);
            }
        })
        .detach();

        let input_for_footer = input.clone();
        let this_for_footer = this.clone();

        open_dialog(window, cx, move |dialog, _, _| {
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
                    let input_click = input_for_buttons.clone();
                    let this_click = this_for_buttons.clone();

                    DialogFooter::new()
                        .child(
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
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(Button::new("new-folder-cancel").label("Cancel").on_click(
                            |_, window, cx| {
                                close_dialog(window, cx);
                            },
                        ))
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

        open_dialog(window, cx, move |dialog, _, _| {
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
                .footer({
                    let select_click = select_for_footer.clone();
                    let this_click = this_for_footer.clone();

                    DialogFooter::new()
                        .child(
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
                                        Ok(()) => close_dialog(window, cx),
                                        Err(error) => window.push_notification(
                                            (
                                                NotificationType::Error,
                                                SharedString::from(error.to_string()),
                                            ),
                                            cx,
                                        ),
                                    }
                                }),
                        )
                        .child(Button::new("move-node-cancel").label("Cancel").on_click(
                            |_, window, cx| {
                                close_dialog(window, cx);
                            },
                        ))
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

            open_dialog(window, cx, move |dialog, _, _| {
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
                    .footer({
                        let request_name_click = request_name_for_buttons.clone();
                        let collection_name_click = collection_name_for_buttons.clone();
                        let this_click = this_for_buttons.clone();
                        let request_click = request_for_buttons.clone();

                        DialogFooter::new()
                            .child(
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
                                                    collections
                                                        .create_collection(&collection_name, cx)
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
                                        close_dialog(window, cx);
                                    }),
                            )
                            .child(
                                Button::new("save-to-new-collection-cancel")
                                    .label("Cancel")
                                    .on_click(|_, window, cx| {
                                        close_dialog(window, cx);
                                    }),
                            )
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

        open_dialog(window, cx, move |dialog, _, _| {
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
                .footer({
                    let request_name_click = request_name_for_buttons.clone();
                    let destination_click = destination_for_buttons.clone();
                    let this_click = this_for_buttons.clone();
                    let request_click = request_for_buttons.clone();

                    DialogFooter::new()
                        .child(
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
                                    close_dialog(window, cx);
                                }),
                        )
                        .child(
                            Button::new("save-to-collection-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| {
                                    close_dialog(window, cx);
                                }),
                        )
                })
        });
    }

    pub fn import_collection_from_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().clone();
        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select a Postman collection or environment".into()),
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

            let result = ImportRegistry::default().import_any_file(&path);
            let _ = cx.update(|window, app| match result {
                Ok(result) => {
                    this.update(app, |view, cx| {
                        let summary = match result.payload {
                            ImportedPayload::Collection(collection) => {
                                let name = collection.name.clone();
                                let folder_count = collection.folder_count();
                                let request_count = collection.request_count();
                                let variables = collection.variables.clone();
                                let variable_count = variables.len();

                                let workspace_id = view.workspaces.update(cx, |workspaces, cx| {
                                    workspaces.create_workspace(name.clone(), cx)
                                });
                                view.switch_workspace(workspace_id, cx);

                                let collection_id =
                                    view.collections.update(cx, |collections, cx| {
                                        collections.import_collection(collection, cx)
                                    });
                                if !variables.is_empty() {
                                    view.environments.update(cx, |environments, cx| {
                                        environments.import_environment(
                                            format!("{name} Variables"),
                                            EnvironmentScope::Project(collection_id),
                                            variables
                                                .into_iter()
                                                .map(|variable| EnvironmentVariable {
                                                    key: variable.key,
                                                    value: variable.value,
                                                    enabled: variable.enabled,
                                                    secret: variable.secret,
                                                    ..EnvironmentVariable::default()
                                                })
                                                .collect(),
                                            cx,
                                        );
                                    });
                                }
                                ImportSummary {
                                    provider: result.provider,
                                    item_kind: "Workspace",
                                    item_name: name,
                                    folder_count: Some(folder_count),
                                    request_count: Some(request_count),
                                    variable_count,
                                    warnings: result.warnings,
                                }
                            }
                            ImportedPayload::Environment(environment) => {
                                let name = environment.name.clone();
                                let variable_count = environment.variables.len();
                                let environment_id =
                                    view.environments.update(cx, |environments, cx| {
                                        environments.import_environment(
                                            name.clone(),
                                            EnvironmentScope::Workspace,
                                            environment
                                                .variables
                                                .into_iter()
                                                .map(|variable| EnvironmentVariable {
                                                    key: variable.key,
                                                    value: variable.value,
                                                    enabled: variable.enabled,
                                                    secret: variable.secret,
                                                    ..EnvironmentVariable::default()
                                                })
                                                .collect(),
                                            cx,
                                        )
                                    });
                                view.environment_panel.update(cx, |panel, cx| {
                                    panel.select_environment(environment_id, cx);
                                });
                                view.sidebar_visible = true;
                                view.sidebar_tab = SidebarTab::Environments;
                                ImportSummary {
                                    provider: result.provider,
                                    item_kind: "Environment",
                                    item_name: name,
                                    folder_count: None,
                                    request_count: None,
                                    variable_count,
                                    warnings: result.warnings,
                                }
                            }
                        };
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
        open_dialog(window, cx, move |dialog, _, _| {
            let warning_count = summary.warnings.len();
            let warnings = summary.warnings.clone();

            dialog
                .title("Import Summary")
                .child(
                    v_flex()
                        .gap_3()
                        .child(format!("Provider: {}", summary.provider))
                        .child(format!("{}: {}", summary.item_kind, summary.item_name))
                        .when_some(summary.folder_count, |element, count| {
                            element.child(format!("Folders imported: {count}"))
                        })
                        .when_some(summary.request_count, |element, count| {
                            element.child(format!("Requests imported: {count}"))
                        })
                        .child(format!("Variables imported: {}", summary.variable_count))
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
                .footer(
                    DialogFooter::new().child(
                        Button::new("import-summary-close")
                            .primary()
                            .label("Close")
                            .on_click(|_, window, cx| {
                                close_dialog(window, cx);
                            }),
                    ),
                )
        });
    }

    /// Close all tabs except the one at the given index
    pub fn close_other_tabs(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            for i in 0..self.tabs.len() {
                if i != index {
                    self.cancel_in_flight_for_tab(i, cx);
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

        let tab_id = tab.id;
        let request_entity = tab.request.clone();
        let response_entity = tab.response.clone();
        let request_view = tab.request_view.clone();
        let tab_name = tab.name.clone();
        let collection_id = tab.collection_id;

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

        // Get request params, then resolve templates only for the outgoing request.
        // Stored requests and history retain {{variables}} so secrets are not copied there.
        let (method, template_headers, template_body) = {
            let request = request_entity.read(cx);
            (
                request.method(),
                request.headers().to_vec(),
                request.body().clone(),
            )
        };
        let resolved = self.environments.read(cx).resolve_request(
            collection_id,
            &url,
            &template_headers,
            &template_body,
        );
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(error) => {
                response_entity.update(cx, |response, cx| {
                    response.set_error(error.user_message(), cx);
                });
                return;
            }
        };
        let resolved_url = resolved.url;
        let resolved_headers = resolved.headers;
        let resolved_body = resolved.body;
        request_entity.update(cx, |request, cx| {
            request.set_sending(true, cx);
        });
        response_entity.update(cx, |response, cx| {
            response.set_loading(cx);
        });

        let started_at = std::time::Instant::now();
        log::info!("Sending {} request", method.as_str());

        // Create request data for history before sending
        let history_request_data = RequestData {
            id: Uuid::new_v4(),
            name: tab_name,
            url: url.clone(),
            method,
            headers: template_headers,
            body: template_body,
            is_sending: false,
        };

        let history_entity = self.history.clone();

        // Spawn HTTP request on Tokio runtime and keep a cancel handle on the tab.
        let (result_rx, in_flight_request) =
            self.http_client
                .spawn_request(method, resolved_url, resolved_headers, resolved_body);
        let generation = if let Some(tab) = self.tabs.get_mut(tab_index) {
            let generation = tab.request_generation.advance();
            tab.in_flight_request = Some(in_flight_request);
            generation
        } else {
            return;
        };

        // Spawn foreground task to await result and update UI
        cx.spawn(async move |view, cx| {
            // Await the result from Tokio runtime
            let result = result_rx.await;

            cx.update(|app| {
                let _ = view.update(app, |main, cx| {
                    let Some(tab) = main.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
                        return;
                    };
                    if tab.request_generation != generation {
                        log::debug!("Discarded stale result for tab {}", tab_id.0);
                        return;
                    }

                    tab.in_flight_request = None;
                    request_entity.update(cx, |req, cx| req.set_sending(false, cx));

                    match result {
                        Ok(Ok(data)) => {
                            log::info!(
                                "Completed {} request: status={}, duration={}ms, size={} bytes",
                                history_request_data.method.as_str(),
                                data.status_code,
                                data.duration_ms,
                                data.body_size_bytes
                            );
                            history_entity.update(cx, |history, cx| {
                                history.add_entry(
                                    history_request_data.clone(),
                                    Some(data.clone()),
                                    cx,
                                );
                            });
                            response_entity.update(cx, |resp, cx| resp.set_success(data, cx));
                        }
                        Ok(Err(error)) => {
                            log::error!(
                                "{} request failed after {}ms: {}",
                                history_request_data.method.as_str(),
                                started_at.elapsed().as_millis(),
                                error
                            );
                            history_entity.update(cx, |history, cx| {
                                history.add_entry(history_request_data.clone(), None, cx);
                            });
                            response_entity
                                .update(cx, |resp, cx| resp.set_error(error.to_string(), cx));
                        }
                        Err(_) => {
                            // Cancellation advances the generation immediately, so this is only
                            // reachable if the worker exits without an explicit user cancel.
                            response_entity.update(cx, |resp, cx| resp.set_cancelled(cx));
                        }
                    }
                });
            })
        })
        .detach();
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
        tab.request_generation.advance();
        tab.request
            .update(cx, |request, cx| request.set_sending(false, cx));
        tab.response
            .update(cx, |response, cx| response.set_cancelled(cx));
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        self.ui_preferences.sidebar_visible = self.sidebar_visible;
        self.persist_ui_preferences();
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
            self.ui_preferences.layout = match layout {
                RequestResponseLayout::Stacked => PreferredLayout::Stacked,
                RequestResponseLayout::SideBySide => PreferredLayout::SideBySide,
            };
            self.persist_ui_preferences();
            cx.notify();
        }
    }

    pub fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_palette.read(cx).is_open() {
            self.command_palette.update(cx, |palette, cx| {
                palette.close(cx);
            });
            close_dialog(window, cx);
            return;
        }

        self.command_palette.update(cx, |palette, cx| {
            palette.toggle(window, cx);
        });
        let palette = self.command_palette.clone();
        let palette_for_close = palette.clone();
        let palette_for_focus = palette.clone();
        let app_focus_for_close = self.focus_handle.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let palette_for_close = palette_for_close.clone();
            let app_focus_for_close = app_focus_for_close.clone();
            dialog
                .w(px(560.0))
                .margin_top(px(48.0))
                .p_0()
                .overflow_hidden()
                .close_button(false)
                .on_close(move |_, window, cx| {
                    palette_for_close.update(cx, |palette, cx| palette.close(cx));
                    app_focus_for_close.focus(window, cx);
                })
                .child(palette.clone())
        });
        palette_for_focus.update(cx, |palette, cx| {
            palette.focus_input(window, cx);
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
        self.cancel_in_flight_for_all_tabs(cx);
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
            let old_collection_id = current_tab.collection_id;

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
                let completion_engine = self.completion_engine.clone();
                let input = cx.new(|cx| {
                    configure_completion(
                        InputState::new(window, cx)
                            .placeholder("Enter request URL...")
                            .default_value(&duplicated_url),
                        Some(&completion_engine),
                        CompletionContext::Url,
                    )
                });
                let tab_id = TabId(self.next_tab_id);
                Self::subscribe_url_input(&input, tab_id, window, cx);
                Some(input)
            } else {
                None
            };

            let completion_engine = self.completion_engine.clone();
            let new_request_view = cx.new(|cx| {
                RequestView::new(new_request.clone(), old_body_type, cx)
                    .with_completion_engine(completion_engine)
                    .with_initial_body_content(body_content)
                    .with_initial_form_data(form_data)
                    .with_initial_multipart_data(multipart_data)
            });
            let new_response_view = cx.new(|cx| ResponseView::new(new_response.clone(), cx));

            let tab_id = TabId(self.next_tab_id);
            self.next_tab_id += 1;
            Self::subscribe_request_changes(&new_request, cx);
            let new_tab = TabState {
                id: tab_id,
                name: format!("{} (copy)", old_name),
                is_custom_name: true,
                request: new_request.clone(),
                response: new_response.clone(),
                url_input: new_url_input,
                method_dropdown: new_method_dropdown,
                request_view: new_request_view,
                response_view: new_response_view,
                in_flight_request: None,
                request_generation: RequestGeneration::default(),
                collection_id: old_collection_id,
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
        if let Some(tab) = self.tabs.get(self.active_tab_index)
            && let Some(url_input) = &tab.url_input
        {
            url_input.update(cx, |state, cx| {
                state.focus(window, cx);
            });
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
        self.completion_engine
            .set_collection_id(self.active_tab().and_then(|tab| tab.collection_id));
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

        let theme = cx.theme().clone();
        let viewport_width = window.viewport_size().width;
        let show_sidebar_rail = self.sidebar_visible && viewport_width < px(960.0);
        let show_full_sidebar = self.sidebar_visible && !show_sidebar_rail;
        let effective_layout = if viewport_width < px(1050.0) {
            RequestResponseLayout::Stacked
        } else {
            self.request_response_layout
        };

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
                let mut info = TabInfo::new(tab.id.0 as usize, i, display_name, method);
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
        let active_collection_id = self.active_tab().and_then(|tab| tab.collection_id);
        let this_for_new_environment = this.clone();
        let this_for_import_environment = this.clone();
        let this_for_delete_environment = this.clone();
        let this_for_rename_environment = this.clone();
        self.environment_panel.update(cx, |panel, cx| {
            panel.set_collection_context(active_collection_id, cx);
            panel.on_new_environment(move |project_id, window, cx| {
                this_for_new_environment.update(cx, |view, cx| {
                    view.show_new_environment_dialog(project_id, window, cx);
                });
            });
            panel.on_import_environment(move |window, cx| {
                this_for_import_environment.update(cx, |view, cx| {
                    view.import_collection_from_file(window, cx);
                });
            });
            panel.on_delete_environment(move |environment_id, window, cx| {
                this_for_delete_environment.update(cx, |view, cx| {
                    view.show_delete_environment_dialog(environment_id, window, cx);
                });
            });
            panel.on_rename_environment(move |environment_id, name, window, cx| {
                this_for_rename_environment.update(cx, |view, cx| {
                    view.show_rename_environment_dialog(environment_id, name, window, cx);
                });
            });
        });

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
            .when(show_sidebar_rail, |el| {
                let history_active = self.sidebar_tab == SidebarTab::History;
                let collections_active = self.sidebar_tab == SidebarTab::Collections;
                let environments_active = self.sidebar_tab == SidebarTab::Environments;
                let this_for_history = this.clone();
                let this_for_collections = this.clone();
                let this_for_environments = this.clone();
                el.child(
                    div()
                        .w(px(44.0))
                        .h_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(6.0))
                        .pt(px(8.0))
                        .bg(theme.secondary)
                        .border_r_1()
                        .border_color(theme.border)
                        .child(
                            Button::new("rail-history")
                                .icon(Icon::new(IconName::History).size(px(15.0)))
                                .ghost()
                                .xsmall()
                                .selected(history_active)
                                .tooltip("History")
                                .on_click(move |_, _, cx| {
                                    this_for_history.update(cx, |view, cx| {
                                        view.set_sidebar_tab(SidebarTab::History, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("rail-collections")
                                .icon(Icon::new(IconName::Folder).size(px(15.0)))
                                .ghost()
                                .xsmall()
                                .selected(collections_active)
                                .tooltip("Collections")
                                .on_click(move |_, _, cx| {
                                    this_for_collections.update(cx, |view, cx| {
                                        view.set_sidebar_tab(SidebarTab::Collections, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("rail-environments")
                                .icon(Icon::new(IconName::Package).size(px(15.0)))
                                .ghost()
                                .xsmall()
                                .selected(environments_active)
                                .tooltip("Environments")
                                .on_click(move |_, _, cx| {
                                    this_for_environments.update(cx, |view, cx| {
                                        view.set_sidebar_tab(SidebarTab::Environments, cx);
                                    });
                                }),
                        ),
                )
            })
            // Sidebar
            .when(show_full_sidebar, |el| {
                let history = self.history.clone();
                let collections = self.collections.clone();
                let environment_panel = self.environment_panel.clone();
                let history_search = self
                    .history_search
                    .clone()
                    .expect("history_search should be initialized");
                let collections_search = self
                    .collections_search
                    .clone()
                    .expect("collections_search should be initialized");
                let sidebar_tab = self.sidebar_tab;
                let sidebar_render_width = if sidebar_tab == SidebarTab::Environments {
                    self.sidebar_width.max(340.0)
                } else {
                    self.sidebar_width
                };
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
                        .w(px(sidebar_render_width))
                        .h_full()
                        .flex_shrink_0()
                        .child(
                            AppSidebar::new(
                                history,
                                collections,
                                environment_panel,
                                history_search,
                                collections_search,
                                self.history_rows.clone(),
                                self.history_rows_initialized,
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
            .when(show_full_sidebar, |el| {
                let border_color = theme.border;
                let this_for_resize = this.clone();
                el.child(
                    div()
                        .id("sidebar-resize-handle")
                        .relative()
                        .w(px(1.0))
                        .h_full()
                        .bg(border_color)
                        .flex_shrink_0()
                        .child(
                            div()
                                .id("sidebar-resize-hit-area")
                                .absolute()
                                .left(px(-3.0))
                                .right(px(-3.0))
                                .top_0()
                                .bottom_0()
                                .cursor_col_resize()
                                .hover(move |style| style.bg(border_color.opacity(0.45)))
                                .on_drag(SidebarResizeDrag, |_, _, _, cx| {
                                    cx.new(|_| SidebarResizeDrag)
                                })
                                .on_drag_move(
                                    move |event: &gpui::DragMoveEvent<SidebarResizeDrag>,
                                          _window,
                                          cx| {
                                        let pos: f32 = event.event.position.x.into();
                                        let new_width = pos.clamp(200.0, 500.0);
                                        this_for_resize.update(cx, |view, cx| {
                                            view.sidebar_width = new_width;
                                            view.ui_preferences.sidebar_width = new_width;
                                            view.persist_ui_preferences();
                                            cx.notify();
                                        });
                                    },
                                ),
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
                    // Compact application toolbar
                    .child(self.render_header(&theme, this.clone(), cx))
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
                            effective_layout,
                        ),
                    )),
            )
            // Dialog layer - renders dialogs on top of everything
            .children(Root::render_dialog_layer(window, cx))
            // Notification layer - renders notifications on top of everything
            .children(Root::render_notification_layer(window, cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{MainView, RequestGeneration, TabId};

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

    #[test]
    fn tab_ids_are_stable_values() {
        assert_eq!(TabId(7), TabId(7));
        assert_ne!(TabId(7), TabId(8));
    }

    #[test]
    fn request_generations_advance_monotonically() {
        let mut current = RequestGeneration::default();
        let first = current.advance();
        let second = current.advance();
        assert_ne!(first, second);
        assert_eq!(current, second);
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
        layout: RequestResponseLayout,
    ) -> impl IntoElement {
        let initial_sizes = match layout {
            RequestResponseLayout::Stacked => self.ui_preferences.stacked_split,
            RequestResponseLayout::SideBySide => self.ui_preferences.side_by_side_split,
        };
        let this_for_resize = this.clone();
        let request_panel = resizable_panel()
            .size(px(initial_sizes[0]))
            .size_range(
                px(150.0)..match layout {
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
            .size(px(initial_sizes[1]))
            .size_range(px(150.0)..gpui::Pixels::MAX)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .overflow_hidden()
                    .child(response_view),
            );

        let on_resize =
            move |state: &Entity<ResizableState>, _window: &mut Window, cx: &mut App| {
                let sizes = state.read(cx).sizes();
                if sizes.len() != 2 {
                    return;
                }
                let split = [sizes[0].as_f32(), sizes[1].as_f32()];
                this_for_resize.update(cx, |view, _cx| {
                    match layout {
                        RequestResponseLayout::Stacked => view.ui_preferences.stacked_split = split,
                        RequestResponseLayout::SideBySide => {
                            view.ui_preferences.side_by_side_split = split
                        }
                    }
                    view.persist_ui_preferences();
                });
            };

        match layout {
            RequestResponseLayout::Stacked => v_resizable("request-response-split-stacked")
                .with_state(&self.stacked_split_state)
                .on_resize(on_resize)
                .child(request_panel)
                .child(response_panel)
                .into_any_element(),
            RequestResponseLayout::SideBySide => h_resizable("request-response-split-side-by-side")
                .with_state(&self.side_by_side_split_state)
                .on_resize(on_resize)
                .child(request_panel)
                .child(response_panel)
                .into_any_element(),
        }
    }

    fn render_header(
        &self,
        theme: &gpui_component::theme::ThemeColor,
        this: Entity<MainView>,
        cx: &App,
    ) -> impl IntoElement {
        let layout_icon = match self.request_response_layout {
            RequestResponseLayout::Stacked => IconName::LayoutSplit,
            RequestResponseLayout::SideBySide => IconName::LayoutStacked,
        };
        let this_for_sidebar = this.clone();
        let this_for_commands = this.clone();
        let this_for_layout = this.clone();
        let this_for_manage_environments = this.clone();
        let this_for_workspace_menu = this;
        let (active_workspace_id, active_workspace_name, workspace_options) = {
            let workspaces = self.workspaces.read(cx);
            (
                workspaces.active_workspace_id(),
                workspaces.active_workspace().name.clone(),
                workspaces.workspaces().to_vec(),
            )
        };
        let collection_id = self.active_tab().and_then(|tab| tab.collection_id);
        let (active_id, active_name, active_color, environment_options) = {
            let environments = self.environments.read(cx);
            let active_id = environments.active_environment_id(collection_id);
            let active = environments.active_environment(collection_id);
            let active_name = active
                .map(|environment| environment.name.clone())
                .unwrap_or_else(|| "No environment".to_string());
            let active_color = active
                .map(|environment| environment.color.clone())
                .unwrap_or(EnvironmentColor::Slate);
            let environment_options: Vec<_> = environments
                .available_for(collection_id)
                .into_iter()
                .map(|environment| {
                    let scope = match environment.scope {
                        EnvironmentScope::Global => "Global".to_string(),
                        EnvironmentScope::Workspace => "Workspace".to_string(),
                        EnvironmentScope::Project(project_id) => self
                            .collections
                            .read(cx)
                            .collections
                            .iter()
                            .find(|collection| collection.id == project_id)
                            .map(|collection| collection.name.clone())
                            .unwrap_or_else(|| "Project".to_string()),
                    };
                    (
                        environment.id,
                        environment.name.clone(),
                        scope,
                        environment.color.clone(),
                    )
                })
                .collect();
            (active_id, active_name, active_color, environment_options)
        };
        let environments_for_menu = self.environments.clone();

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(40.0))
            .px(px(10.0))
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        Button::new("toolbar-sidebar")
                            .icon(Icon::new(IconName::PanelLeft).size(px(15.0)))
                            .ghost()
                            .xsmall()
                            .tooltip(if self.sidebar_visible {
                                "Hide sidebar (⌘B)"
                            } else {
                                "Show sidebar (⌘B)"
                            })
                            .on_click(move |_, _, cx| {
                                this_for_sidebar.update(cx, |view, cx| view.toggle_sidebar(cx));
                            }),
                    )
                    .child(
                        div()
                            .text_color(theme.foreground)
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_size(px(15.0))
                            .child("setu"),
                    )
                    .child(div().w(px(1.0)).h(px(18.0)).bg(theme.border))
                    .child(
                        Button::new("toolbar-workspace")
                            .icon(Icon::new(IconName::Box).size(px(14.0)))
                            .label(active_workspace_name.clone())
                            .ghost()
                            .xsmall()
                            .tooltip("Switch workspace")
                            .dropdown_menu(move |mut menu, _window, _cx| {
                                menu = menu.label("Workspace");
                                for workspace in &workspace_options {
                                    let workspace_id = workspace.id;
                                    let this_for_switch = this_for_workspace_menu.clone();
                                    let mut item = PopupMenuItem::new(workspace.name.clone());
                                    if workspace_id == active_workspace_id {
                                        item = item.icon(IconName::Check);
                                    }
                                    menu = menu.item(item.on_click(move |_, _, cx| {
                                        this_for_switch.update(cx, |view, cx| {
                                            view.switch_workspace(workspace_id, cx);
                                        });
                                    }));
                                }

                                let this_for_new = this_for_workspace_menu.clone();
                                let this_for_rename = this_for_workspace_menu.clone();
                                let this_for_delete = this_for_workspace_menu.clone();
                                let active_name = active_workspace_name.clone();
                                let mut menu = menu.separator().item(
                                    PopupMenuItem::new("New workspace")
                                        .icon(IconName::Plus)
                                        .on_click(move |_, window, cx| {
                                            this_for_new.update(cx, |view, cx| {
                                                view.show_new_workspace_dialog(window, cx);
                                            });
                                        }),
                                );
                                menu = menu.item(
                                    PopupMenuItem::new("Rename workspace")
                                        .icon(IconName::FilePen)
                                        .on_click(move |_, window, cx| {
                                            this_for_rename.update(cx, |view, cx| {
                                                view.show_rename_workspace_dialog(
                                                    active_workspace_id,
                                                    active_name.clone(),
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }),
                                );
                                if workspace_options.len() > 1 {
                                    menu = menu.item(
                                        PopupMenuItem::new("Delete workspace")
                                            .icon(IconName::Trash)
                                            .on_click(move |_, window, cx| {
                                                this_for_delete.update(cx, |view, cx| {
                                                    view.show_delete_workspace_dialog(
                                                        active_workspace_id,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(ProtocolSelector::new(ProtocolType::Rest)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        Button::new("toolbar-environment")
                            .icon(
                                Icon::new(IconName::Package)
                                    .size(px(14.0))
                                    .text_color(active_color.accent()),
                            )
                            .label(active_name)
                            .ghost()
                            .xsmall()
                            .tooltip("Active environment")
                            .dropdown_menu(move |mut menu, _window, _cx| {
                                menu = menu.label("Environment");
                                for (id, name, scope, _color) in &environment_options {
                                    let environment_id = *id;
                                    let environments = environments_for_menu.clone();
                                    let mut item = PopupMenuItem::new(format!("{name} · {scope}"));
                                    if Some(environment_id) == active_id {
                                        item = item.icon(IconName::Check);
                                    }
                                    menu = menu.item(item.on_click(move |_, _, cx| {
                                        environments.update(cx, |environments, cx| {
                                            environments.set_active(
                                                collection_id,
                                                Some(environment_id),
                                                cx,
                                            );
                                        });
                                    }));
                                }
                                let this_for_manage = this_for_manage_environments.clone();
                                menu.separator().item(
                                    PopupMenuItem::new("Manage environments")
                                        .icon(IconName::Package)
                                        .on_click(move |_, _, cx| {
                                            this_for_manage.update(cx, |view, cx| {
                                                view.sidebar_visible = true;
                                                view.ui_preferences.sidebar_visible = true;
                                                view.persist_ui_preferences();
                                                view.set_sidebar_tab(SidebarTab::Environments, cx);
                                            });
                                        }),
                                )
                            }),
                    )
                    .child(div().w(px(1.0)).h(px(18.0)).bg(theme.border))
                    .child(
                        Button::new("toolbar-command-search")
                            .icon(Icon::new(IconName::Search).size(px(14.0)))
                            .label("Commands")
                            .ghost()
                            .xsmall()
                            .tooltip("Search commands (⌘K)")
                            .on_click(move |_, window, cx| {
                                this_for_commands.update(cx, |view, cx| {
                                    view.toggle_command_palette(window, cx);
                                });
                            }),
                    )
                    .child(
                        Button::new("toolbar-layout")
                            .icon(Icon::new(layout_icon).size(px(14.0)))
                            .ghost()
                            .xsmall()
                            .tooltip("Toggle request/response layout")
                            .on_click(move |_, _, cx| {
                                this_for_layout.update(cx, |view, cx| {
                                    view.toggle_request_response_layout(cx);
                                });
                            }),
                    ),
            )
    }
}
