use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Corner, DismissEvent, Entity, Focusable, IntoElement, MouseButton,
    Point, SharedString, Styled, Subscription, Window, anchored, deferred, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::list::ListItem;
use gpui_component::menu::{DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_component::spinner::Spinner;
use gpui_component::tree::{TreeItem, TreeState, tree};
use gpui_component::{ActiveTheme, Icon, IconName as ComponentIconName, Sizable};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use uuid::Uuid;

use crate::entities::{CollectionNode, CollectionsEntity, HttpMethod, SidebarLoadState};
use crate::icons::IconName;
use crate::theme::method_color;

#[derive(Clone, Default)]
struct PanelCallbacks {
    on_load_request: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_collection: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_node: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_rename_collection: Option<Rc<dyn Fn(Uuid, String, &mut Window, &mut App) + 'static>>,
    on_rename_node: Option<Rc<dyn Fn(Uuid, Uuid, String, &mut Window, &mut App) + 'static>>,
    on_new_collection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_import_collection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_new_folder: Option<Rc<dyn Fn(Uuid, Option<Uuid>, &mut Window, &mut App) + 'static>>,
    on_move_node: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_toggle_collection_expand: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_toggle_node_expand: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
}

#[derive(Default)]
struct RowContextMenuState {
    menu: Option<Entity<PopupMenu>>,
    open: bool,
    position: Point<gpui::Pixels>,
    subscription: Option<Subscription>,
}

#[derive(Clone)]
enum CollectionTreeRow {
    Collection {
        id: Uuid,
        name: String,
        expanded: bool,
        request_count: usize,
    },
    Folder {
        collection_id: Uuid,
        id: Uuid,
        name: String,
        expanded: bool,
        has_children: bool,
    },
    Request {
        collection_id: Uuid,
        id: Uuid,
        name: String,
        method: HttpMethod,
    },
}

#[derive(Default)]
struct CollectionsTreeSyncState {
    initialized: bool,
    revision: u64,
    query: String,
    rows: Arc<HashMap<SharedString, CollectionTreeRow>>,
}

#[derive(IntoElement)]
pub struct CollectionsPanel {
    collections: Entity<CollectionsEntity>,
    search_input: Entity<InputState>,
    on_load_request: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_collection: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_node: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_rename_collection: Option<Rc<dyn Fn(Uuid, String, &mut Window, &mut App) + 'static>>,
    on_rename_node: Option<Rc<dyn Fn(Uuid, Uuid, String, &mut Window, &mut App) + 'static>>,
    on_new_collection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_import_collection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_new_folder: Option<Rc<dyn Fn(Uuid, Option<Uuid>, &mut Window, &mut App) + 'static>>,
    on_move_node: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_toggle_collection_expand: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_toggle_node_expand: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
}

impl CollectionsPanel {
    pub fn new(collections: Entity<CollectionsEntity>, search_input: Entity<InputState>) -> Self {
        Self {
            collections,
            search_input,
            on_load_request: None,
            on_delete_collection: None,
            on_delete_node: None,
            on_rename_collection: None,
            on_rename_node: None,
            on_new_collection: None,
            on_import_collection: None,
            on_new_folder: None,
            on_move_node: None,
            on_toggle_collection_expand: None,
            on_toggle_node_expand: None,
        }
    }

    pub fn on_load_request(
        mut self,
        f: impl Fn(Uuid, Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_load_request = Some(Rc::new(f));
        self
    }

    pub fn on_delete_collection(
        mut self,
        f: impl Fn(Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete_collection = Some(Rc::new(f));
        self
    }

    pub fn on_delete_node(
        mut self,
        f: impl Fn(Uuid, Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete_node = Some(Rc::new(f));
        self
    }

    pub fn on_rename_collection(
        mut self,
        f: impl Fn(Uuid, String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_rename_collection = Some(Rc::new(f));
        self
    }

    pub fn on_rename_node(
        mut self,
        f: impl Fn(Uuid, Uuid, String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_rename_node = Some(Rc::new(f));
        self
    }

    pub fn on_new_collection(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_collection = Some(Rc::new(f));
        self
    }

    pub fn on_import_collection(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_import_collection = Some(Rc::new(f));
        self
    }

    pub fn on_new_folder(
        mut self,
        f: impl Fn(Uuid, Option<Uuid>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_new_folder = Some(Rc::new(f));
        self
    }

    pub fn on_move_node(mut self, f: impl Fn(Uuid, Uuid, &mut Window, &mut App) + 'static) -> Self {
        self.on_move_node = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_collection_expand(
        mut self,
        f: impl Fn(Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_collection_expand = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_node_expand(
        mut self,
        f: impl Fn(Uuid, Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_node_expand = Some(Rc::new(f));
        self
    }

    fn collection_tree_id(collection_id: Uuid) -> SharedString {
        format!("collection:{collection_id}").into()
    }

    fn folder_tree_id(collection_id: Uuid, node_id: Uuid) -> SharedString {
        format!("folder:{collection_id}:{node_id}").into()
    }

    fn request_tree_id(collection_id: Uuid, node_id: Uuid) -> SharedString {
        format!("request:{collection_id}:{node_id}").into()
    }

    fn build_node_snapshot(
        collection_id: Uuid,
        node: &CollectionNode,
        rows: &mut HashMap<SharedString, CollectionTreeRow>,
    ) -> TreeItem {
        match node {
            CollectionNode::Folder(folder) => {
                let id = Self::folder_tree_id(collection_id, folder.id);
                let children = folder
                    .children
                    .iter()
                    .map(|child| Self::build_node_snapshot(collection_id, child, rows))
                    .collect::<Vec<_>>();
                rows.insert(
                    id.clone(),
                    CollectionTreeRow::Folder {
                        collection_id,
                        id: folder.id,
                        name: folder.name.clone(),
                        expanded: folder.expanded,
                        has_children: !folder.children.is_empty(),
                    },
                );
                TreeItem::new(id, folder.name.clone())
                    .expanded(folder.expanded)
                    .children(children)
            }
            CollectionNode::Request(request) => {
                let id = Self::request_tree_id(collection_id, request.id);
                let name = request.display_name();
                rows.insert(
                    id.clone(),
                    CollectionTreeRow::Request {
                        collection_id,
                        id: request.id,
                        name: name.clone(),
                        method: request.request.method,
                    },
                );
                TreeItem::new(id, name)
            }
        }
    }

    fn build_tree_snapshot(
        collections: &CollectionsEntity,
        query: &str,
    ) -> (Vec<TreeItem>, Arc<HashMap<SharedString, CollectionTreeRow>>) {
        let mut rows = HashMap::new();
        let items = collections
            .filtered_collections(query)
            .into_iter()
            .map(|collection| {
                let id = Self::collection_tree_id(collection.id);
                let children = collection
                    .nodes
                    .iter()
                    .map(|node| Self::build_node_snapshot(collection.id, node, &mut rows))
                    .collect::<Vec<_>>();
                rows.insert(
                    id.clone(),
                    CollectionTreeRow::Collection {
                        id: collection.id,
                        name: collection.name.clone(),
                        expanded: collection.expanded,
                        request_count: collection.request_count(),
                    },
                );
                TreeItem::new(id, collection.name)
                    .expanded(collection.expanded)
                    .children(children)
            })
            .collect();

        (items, Arc::new(rows))
    }

    fn callbacks(&self) -> PanelCallbacks {
        PanelCallbacks {
            on_load_request: self.on_load_request.clone(),
            on_delete_collection: self.on_delete_collection.clone(),
            on_delete_node: self.on_delete_node.clone(),
            on_rename_collection: self.on_rename_collection.clone(),
            on_rename_node: self.on_rename_node.clone(),
            on_new_collection: self.on_new_collection.clone(),
            on_import_collection: self.on_import_collection.clone(),
            on_new_folder: self.on_new_folder.clone(),
            on_move_node: self.on_move_node.clone(),
            on_toggle_collection_expand: self.on_toggle_collection_expand.clone(),
            on_toggle_node_expand: self.on_toggle_node_expand.clone(),
        }
    }

    fn render_empty_state(
        theme: &gpui_component::theme::ThemeColor,
        on_new: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
        on_import: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    ) -> AnyElement {
        let mut container = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h_full()
            .py(px(48.0))
            .gap(px(16.0))
            .child(
                Icon::new(IconName::Folder)
                    .size(px(40.0))
                    .text_color(theme.muted_foreground.opacity(0.5)),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(13.0))
                    .text_center()
                    .child("No collections yet"),
            );

        if let Some(handler) = on_new {
            container = container.child(
                Button::new("create-first-collection")
                    .small()
                    .primary()
                    .label("Create Collection")
                    .icon(Icon::new(IconName::FolderPlus).size(px(14.0)))
                    .on_click(move |_, window, cx| handler(window, cx)),
            );
        }

        if let Some(handler) = on_import {
            container = container.child(
                Button::new("import-first-collection")
                    .small()
                    .label("Import Collection")
                    .icon(Icon::new(IconName::FileUp).size(px(14.0)))
                    .on_click(move |_, window, cx| handler(window, cx)),
            );
        }

        container.into_any_element()
    }

    fn render_search_empty_state(theme: &gpui_component::theme::ThemeColor) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h_full()
            .py(px(48.0))
            .gap(px(8.0))
            .child(
                div()
                    .text_color(theme.foreground)
                    .text_size(px(13.0))
                    .child("No matching collections"),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(12.0))
                    .child("Try a different search query."),
            )
            .into_any_element()
    }

    fn build_collection_popup_menu(
        menu: PopupMenu,
        callbacks: &PanelCallbacks,
        collection_id: Uuid,
        current_name: String,
    ) -> PopupMenu {
        let on_rename_collection = callbacks.on_rename_collection.clone();
        let on_new_folder = callbacks.on_new_folder.clone();
        let on_delete_collection = callbacks.on_delete_collection.clone();

        menu.item(
            PopupMenuItem::new("Rename")
                .icon(IconName::FilePen)
                .on_click(move |_event, window, cx| {
                    if let Some(ref handler) = on_rename_collection {
                        handler(collection_id, current_name.clone(), window, cx);
                    }
                }),
        )
        .item(
            PopupMenuItem::new("New Folder")
                .icon(IconName::FolderPlus)
                .on_click(move |_event, window, cx| {
                    if let Some(ref handler) = on_new_folder {
                        handler(collection_id, None, window, cx);
                    }
                }),
        )
        .separator()
        .item(PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
            move |_event, window, cx| {
                if let Some(ref handler) = on_delete_collection {
                    handler(collection_id, window, cx);
                }
            },
        ))
    }

    fn build_folder_popup_menu(
        menu: PopupMenu,
        callbacks: &PanelCallbacks,
        collection_id: Uuid,
        node_id: Uuid,
        current_name: String,
    ) -> PopupMenu {
        let on_rename_node = callbacks.on_rename_node.clone();
        let on_new_folder = callbacks.on_new_folder.clone();
        let on_move_node = callbacks.on_move_node.clone();
        let on_delete_node = callbacks.on_delete_node.clone();

        menu.item(
            PopupMenuItem::new("Rename")
                .icon(IconName::FilePen)
                .on_click(move |_event, window, cx| {
                    if let Some(ref handler) = on_rename_node {
                        handler(collection_id, node_id, current_name.clone(), window, cx);
                    }
                }),
        )
        .item(
            PopupMenuItem::new("New Folder")
                .icon(IconName::FolderPlus)
                .on_click(move |_event, window, cx| {
                    if let Some(ref handler) = on_new_folder {
                        handler(collection_id, Some(node_id), window, cx);
                    }
                }),
        )
        .item(PopupMenuItem::new("Move").icon(IconName::Replace).on_click(
            move |_event, window, cx| {
                if let Some(ref handler) = on_move_node {
                    handler(collection_id, node_id, window, cx);
                }
            },
        ))
        .item(PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
            move |_event, window, cx| {
                if let Some(ref handler) = on_delete_node {
                    handler(collection_id, node_id, window, cx);
                }
            },
        ))
    }

    fn build_request_popup_menu(
        menu: PopupMenu,
        callbacks: &PanelCallbacks,
        collection_id: Uuid,
        node_id: Uuid,
        current_name: String,
    ) -> PopupMenu {
        let on_rename_node = callbacks.on_rename_node.clone();
        let on_move_node = callbacks.on_move_node.clone();
        let on_delete_node = callbacks.on_delete_node.clone();

        menu.item(
            PopupMenuItem::new("Rename")
                .icon(IconName::FilePen)
                .on_click(move |_event, window, cx| {
                    if let Some(ref handler) = on_rename_node {
                        handler(collection_id, node_id, current_name.clone(), window, cx);
                    }
                }),
        )
        .item(PopupMenuItem::new("Move").icon(IconName::Replace).on_click(
            move |_event, window, cx| {
                if let Some(ref handler) = on_move_node {
                    handler(collection_id, node_id, window, cx);
                }
            },
        ))
        .item(PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
            move |_event, window, cx| {
                if let Some(ref handler) = on_delete_node {
                    handler(collection_id, node_id, window, cx);
                }
            },
        ))
    }

    fn render_chevron(expanded: bool, theme: &gpui_component::theme::ThemeColor) -> AnyElement {
        div()
            .w(px(16.0))
            .h(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .child(
                Icon::new(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size(px(12.0))
                .text_color(theme.muted_foreground.opacity(0.7)),
            )
            .into_any_element()
    }

    fn render_count_text(theme: &gpui_component::theme::ThemeColor, count: usize) -> AnyElement {
        div()
            .flex_shrink_0()
            .text_color(theme.muted_foreground.opacity(0.55))
            .text_size(px(11.0))
            .child(count.to_string())
            .into_any_element()
    }

    fn render_action_button(
        theme: &gpui_component::theme::ThemeColor,
        group_id: SharedString,
        action_id: SharedString,
        tooltip: &'static str,
        right_offset: gpui::Pixels,
        menu_builder: impl Fn(PopupMenu, &mut Window, &mut gpui::Context<PopupMenu>) -> PopupMenu
        + 'static,
    ) -> AnyElement {
        div()
            .absolute()
            .right(right_offset)
            .top_0()
            .bottom_0()
            .flex()
            .flex_row()
            .items_center()
            .px(px(4.0))
            .invisible()
            .group_hover(group_id, |this| this.visible().bg(theme.list_hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Button::new(action_id)
                    .ghost()
                    .xsmall()
                    .icon(
                        Icon::new(ComponentIconName::Ellipsis)
                            .size(px(13.0))
                            .text_color(theme.muted_foreground),
                    )
                    .tooltip(tooltip)
                    .dropdown_menu(menu_builder),
            )
            .into_any_element()
    }

    fn render_method_badge(method_str: &'static str, method_color: gpui::Hsla) -> AnyElement {
        div()
            .min_w(px(36.0))
            .flex_shrink_0()
            .px(px(6.0))
            .py(px(2.0))
            .bg(method_color.opacity(0.15))
            .rounded(px(4.0))
            .text_color(method_color)
            .font_weight(gpui::FontWeight::BOLD)
            .text_size(px(9.0))
            .text_center()
            .child(method_str)
            .into_any_element()
    }

    fn wrap_with_row_context_menu(
        window: &mut Window,
        cx: &mut App,
        id: SharedString,
        trigger: AnyElement,
        builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> AnyElement {
        let menu_state =
            window.use_keyed_state(id.clone(), cx, |_, _| RowContextMenuState::default());
        let builder = Rc::new(builder);
        let (open, position, menu) = {
            let state = menu_state.read(cx);
            (state.open, state.position, state.menu.clone())
        };

        let mut wrapper = div()
            .relative()
            .child(trigger)
            .on_mouse_down(MouseButton::Right, {
                let menu_state = menu_state.clone();
                let builder = builder.clone();
                move |event, window, cx| {
                    let click_position = event.position;

                    menu_state.update(cx, |state, _| {
                        state.open = true;
                        state.position = click_position;
                        state.menu = None;
                        state.subscription = None;
                    });

                    window.defer(cx, {
                        let menu_state = menu_state.clone();
                        let builder = builder.clone();
                        move |window, cx| {
                            let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
                                builder(menu, window, cx)
                            });

                            menu.focus_handle(cx).focus(window);

                            let subscription = window.subscribe(&menu, cx, {
                                let menu_state = menu_state.clone();
                                move |_, _: &DismissEvent, window, cx| {
                                    menu_state.update(cx, |state, _| {
                                        state.open = false;
                                        state.menu = None;
                                        state.subscription = None;
                                    });
                                    window.refresh();
                                }
                            });

                            menu_state.update(cx, |state, _| {
                                state.menu = Some(menu.clone());
                                state.subscription = Some(subscription);
                            });

                            window.refresh();
                        }
                    });
                }
            });

        if open {
            if let Some(menu) = menu {
                wrapper = wrapper.child(
                    deferred(
                        anchored()
                            .position(position)
                            .snap_to_window_with_margin(px(8.))
                            .anchor(Corner::TopLeft)
                            .child(menu),
                    )
                    .with_priority(1),
                );
            }
        }

        wrapper.into_any_element()
    }

    fn render_collection_row(
        window: &mut Window,
        cx: &mut App,
        collection_id: Uuid,
        name: &str,
        is_expanded: bool,
        request_count: usize,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
    ) -> AnyElement {
        let group_id = SharedString::from(format!("collection-row-{collection_id}"));
        let action_id = SharedString::from(format!("collection-actions-{collection_id}"));
        let row_name = name.to_string();
        let callbacks_for_toggle = callbacks.clone();
        let callbacks_for_menu = callbacks.clone();
        let callbacks_for_action = callbacks.clone();
        let action_name = name.to_string();
        let action_collection_id = collection_id;
        let row_id = SharedString::from(format!("collection-row-{collection_id}"));

        let row = div()
            .id(row_id.clone())
            .group(group_id.clone())
            .relative()
            .flex()
            .items_center()
            .w_full()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(5.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(|this| this.bg(theme.list_hover))
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = callbacks_for_toggle.on_toggle_collection_expand {
                    handler(collection_id, window, cx);
                }
            })
            .child(Self::render_chevron(is_expanded, theme))
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        Icon::new(if is_expanded {
                            IconName::FolderOpen
                        } else {
                            IconName::Folder
                        })
                        .size(px(14.0))
                        .text_color(theme.muted_foreground.opacity(0.85)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_color(theme.foreground)
                    .text_size(px(11.5))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_ellipsis()
                    .child(name.to_string()),
            )
            .child(Self::render_count_text(theme, request_count))
            .child(Self::render_action_button(
                theme,
                group_id,
                action_id,
                "Collection actions",
                px(8.0),
                move |menu, _window, _cx| {
                    Self::build_collection_popup_menu(
                        menu,
                        &callbacks_for_action,
                        action_collection_id,
                        action_name.clone(),
                    )
                },
            ))
            .into_any_element();

        Self::wrap_with_row_context_menu(window, cx, row_id, row, move |menu, _window, _cx| {
            Self::build_collection_popup_menu(
                menu,
                &callbacks_for_menu,
                collection_id,
                row_name.clone(),
            )
        })
    }

    fn render_folder_row(
        window: &mut Window,
        cx: &mut App,
        collection_id: Uuid,
        node_id: Uuid,
        name: &str,
        is_expanded: bool,
        has_children: bool,
        depth: usize,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
    ) -> AnyElement {
        let group_id = SharedString::from(format!("folder-row-{collection_id}-{node_id}"));
        let action_id = SharedString::from(format!("folder-actions-{collection_id}-{node_id}"));
        let row_name = name.to_string();
        let callbacks_for_toggle = callbacks.clone();
        let callbacks_for_menu = callbacks.clone();
        let callbacks_for_action = callbacks.clone();
        let action_name = name.to_string();
        let action_node_id = node_id;
        let row_id = SharedString::from(format!("folder-row-{collection_id}-{node_id}"));

        let row = div()
            .id(row_id.clone())
            .group(group_id.clone())
            .relative()
            .flex()
            .items_center()
            .w_full()
            .gap(px(8.0))
            .pl(px(8.0 + depth as f32 * 14.0))
            .pr(px(12.0))
            .py(px(5.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(|this| this.bg(theme.list_hover))
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = callbacks_for_toggle.on_toggle_node_expand {
                    handler(collection_id, node_id, window, cx);
                }
            })
            .child(Self::render_chevron(has_children && is_expanded, theme))
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        Icon::new(if has_children && is_expanded {
                            IconName::FolderOpen
                        } else {
                            IconName::Folder
                        })
                        .size(px(14.0))
                        .text_color(theme.muted_foreground.opacity(0.85)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_color(theme.foreground)
                    .text_size(px(11.5))
                    .text_ellipsis()
                    .child(name.to_string()),
            )
            .child(Self::render_action_button(
                theme,
                group_id,
                action_id,
                "Folder actions",
                px(8.0),
                move |menu, _window, _cx| {
                    Self::build_folder_popup_menu(
                        menu,
                        &callbacks_for_action,
                        collection_id,
                        action_node_id,
                        action_name.clone(),
                    )
                },
            ))
            .into_any_element();

        Self::wrap_with_row_context_menu(window, cx, row_id, row, move |menu, _window, _cx| {
            Self::build_folder_popup_menu(
                menu,
                &callbacks_for_menu,
                collection_id,
                node_id,
                row_name.clone(),
            )
        })
    }

    fn render_request_row(
        window: &mut Window,
        cx: &mut App,
        collection_id: Uuid,
        request_id: Uuid,
        name: &str,
        method: HttpMethod,
        depth: usize,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
    ) -> AnyElement {
        let group_id = SharedString::from(format!("request-row-{collection_id}-{request_id}"));
        let action_id = SharedString::from(format!("request-actions-{collection_id}-{request_id}"));
        let m_color = method_color(&method, cx);
        let callbacks_for_load = callbacks.clone();
        let callbacks_for_menu = callbacks.clone();
        let callbacks_for_action = callbacks.clone();
        let context_name = name.to_string();
        let action_name = name.to_string();
        let method_str = method.as_str();
        let row_id = SharedString::from(format!("request-row-{collection_id}-{request_id}"));

        let row = div()
            .id(row_id.clone())
            .group(group_id.clone())
            .relative()
            .flex()
            .items_center()
            .w_full()
            .gap(px(8.0))
            .pl(px(22.0 + depth as f32 * 14.0))
            .pr(px(12.0))
            .py(px(5.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(|this| this.bg(theme.list_hover))
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = callbacks_for_load.on_load_request {
                    handler(collection_id, request_id, window, cx);
                }
            })
            .child(Self::render_method_badge(method_str, m_color))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_color(theme.foreground.opacity(0.9))
                    .text_size(px(11.5))
                    .text_ellipsis()
                    .child(name.to_string()),
            )
            .child(Self::render_action_button(
                theme,
                group_id,
                action_id,
                "Request actions",
                px(8.0),
                move |menu, _window, _cx| {
                    Self::build_request_popup_menu(
                        menu,
                        &callbacks_for_action,
                        collection_id,
                        request_id,
                        action_name.clone(),
                    )
                },
            ))
            .into_any_element();

        Self::wrap_with_row_context_menu(window, cx, row_id, row, move |menu, _window, _cx| {
            Self::build_request_popup_menu(
                menu,
                &callbacks_for_menu,
                collection_id,
                request_id,
                context_name.clone(),
            )
        })
    }

    fn render_tree_row(
        window: &mut Window,
        cx: &mut App,
        row: &CollectionTreeRow,
        depth: usize,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
    ) -> AnyElement {
        match row {
            CollectionTreeRow::Collection {
                id,
                name,
                expanded,
                request_count,
            } => Self::render_collection_row(
                window,
                cx,
                *id,
                name,
                *expanded,
                *request_count,
                theme,
                callbacks,
            ),
            CollectionTreeRow::Folder {
                collection_id,
                id,
                name,
                expanded,
                has_children,
            } => Self::render_folder_row(
                window,
                cx,
                *collection_id,
                *id,
                name,
                *expanded,
                *has_children,
                depth,
                theme,
                callbacks,
            ),
            CollectionTreeRow::Request {
                collection_id,
                id,
                name,
                method,
            } => Self::render_request_row(
                window,
                cx,
                *collection_id,
                *id,
                name,
                *method,
                depth,
                theme,
                callbacks,
            ),
        }
    }
}

impl RenderOnce for CollectionsPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let search_query = self.search_input.read(cx).text().to_string();
        let collections_data = self.collections.read(cx);
        let is_empty = collections_data.is_empty();
        let revision = collections_data.revision();
        let load_state = collections_data.load_state.clone();
        let callbacks = self.callbacks();

        let tree_state = window.use_keyed_state("collections-tree", cx, |_, cx| TreeState::new(cx));
        let sync_state = window.use_keyed_state("collections-tree-sync", cx, |_, _| {
            CollectionsTreeSyncState::default()
        });
        let needs_tree_sync = {
            let sync = sync_state.read(cx);
            !sync.initialized || sync.revision != revision || sync.query != search_query
        };
        if needs_tree_sync {
            let (items, rows) = Self::build_tree_snapshot(self.collections.read(cx), &search_query);
            tree_state.update(cx, |state, cx| state.set_items(items, cx));
            sync_state.update(cx, |sync, _| {
                sync.initialized = true;
                sync.revision = revision;
                sync.query.clone_from(&search_query);
                sync.rows = rows;
            });
        }
        let tree_rows = sync_state.read(cx).rows.clone();
        let has_tree_rows = !tree_rows.is_empty();

        let content = if matches!(load_state, SidebarLoadState::Loading) {
            div()
                .flex()
                .h_full()
                .items_center()
                .justify_center()
                .child(Spinner::new().small())
                .into_any_element()
        } else if let SidebarLoadState::Error(error) = load_state {
            div()
                .flex()
                .flex_col()
                .h_full()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .text_color(theme.muted_foreground)
                .child(Icon::new(IconName::TriangleAlert).size(px(20.0)))
                .child(
                    div()
                        .text_size(px(11.0))
                        .child("Could not load collections"),
                )
                .child(div().text_size(px(10.0)).child(error.to_string()))
                .into_any_element()
        } else if is_empty {
            Self::render_empty_state(
                &theme,
                callbacks.on_new_collection.as_ref().map(Rc::clone),
                callbacks.on_import_collection.as_ref().map(Rc::clone),
            )
        } else if !has_tree_rows {
            Self::render_search_empty_state(&theme)
        } else {
            let list_theme = theme.clone();
            let tree_callbacks = callbacks.clone();
            tree(&tree_state, move |index, entry, selected, window, cx| {
                let row = tree_rows
                    .get(&entry.item().id)
                    .expect("collection tree rows stay synchronized with TreeState");
                ListItem::new(index)
                    .selected(selected)
                    .p_0()
                    .h(px(32.0))
                    .child(Self::render_tree_row(
                        window,
                        cx,
                        row,
                        entry.depth(),
                        &list_theme,
                        &tree_callbacks,
                    ))
            })
            .size_full()
            .into_any_element()
        };

        let mut actions = div().flex().flex_row().items_center().gap(px(2.0));
        if let Some(handler) = callbacks.on_import_collection {
            actions = actions.child(
                Button::new("import-collection")
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(IconName::FileUp).size(px(14.0)))
                    .tooltip("Import Collection")
                    .on_click(move |_, window, cx| handler(window, cx)),
            );
        }
        if let Some(handler) = callbacks.on_new_collection {
            actions = actions.child(
                Button::new("new-collection")
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(IconName::FolderPlus).size(px(14.0)))
                    .tooltip("New Collection")
                    .on_click(move |_, window, cx| handler(window, cx)),
            );
        }

        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(theme.sidebar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(12.0))
                    .py(px(8.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                Icon::new(IconName::Folder)
                                    .size(px(14.0))
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_color(theme.foreground)
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_size(px(13.0))
                                    .child("Collections"),
                            ),
                    )
                    .child(actions),
            )
            .child(
                div().px(px(12.0)).pb(px(6.0)).child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(8.0))
                        .py(px(5.0))
                        .bg(theme.border.opacity(0.5))
                        .rounded(px(6.0))
                        .child(
                            Icon::new(IconName::Search)
                                .size(px(14.0))
                                .text_color(theme.muted_foreground),
                        )
                        .child(Input::new(&self.search_input).appearance(false).xsmall()),
                ),
            )
            .child(
                div()
                    .id("collections-scroll-container")
                    .flex_1()
                    .overflow_x_hidden()
                    .pl(px(6.0))
                    .pr(px(4.0))
                    .pb(px(8.0))
                    .child(content),
            )
    }
}
