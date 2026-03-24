use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, App, Entity, IntoElement, MouseButton, SharedString, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_component::{ActiveTheme, Icon, IconName as ComponentIconName, Sizable};
use std::rc::Rc;
use uuid::Uuid;

use crate::entities::{Collection, CollectionNode, CollectionsEntity};
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

    fn row_height() -> gpui::Pixels {
        px(32.0)
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
            .items_center()
            .invisible()
            .group_hover(group_id, |this| this.visible())
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
            .w(px(32.0))
            .flex_shrink_0()
            .text_color(method_color)
            .font_weight(gpui::FontWeight::BOLD)
            .text_size(px(10.0))
            .child(method_str)
            .into_any_element()
    }

    fn render_collection_row(
        collection: &Collection,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
    ) -> AnyElement {
        let group_id = SharedString::from(format!("collection-row-{}", collection.id));
        let action_id = SharedString::from(format!("collection-actions-{}", collection.id));
        let row_name = collection.name.clone();
        let callbacks_for_toggle = callbacks.clone();
        let callbacks_for_menu = callbacks.clone();
        let callbacks_for_action = callbacks.clone();
        let collection_id = collection.id;
        let action_name = collection.name.clone();
        let action_collection_id = collection.id;
        let is_expanded = collection.expanded;

        div()
            .id(SharedString::from(format!(
                "collection-row-{}",
                collection.id
            )))
            .group(group_id.clone())
            .relative()
            .flex()
            .items_center()
            .w_full()
            .h(Self::row_height())
            .gap(px(6.0))
            .pl(px(8.0))
            .pr(px(8.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(|this| this.bg(theme.muted.opacity(0.25)))
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = callbacks_for_toggle.on_toggle_collection_expand {
                    handler(collection_id, window, cx);
                }
            })
            .context_menu(move |menu, _window, _cx| {
                Self::build_collection_popup_menu(
                    menu,
                    &callbacks_for_menu,
                    collection_id,
                    row_name.clone(),
                )
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
                    .text_size(px(12.5))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_ellipsis()
                    .child(collection.name.clone()),
            )
            .child(Self::render_count_text(theme, collection.request_count()))
            .child(Self::render_action_button(
                theme,
                group_id,
                action_id,
                "Collection actions",
                px(6.0),
                move |menu, _window, _cx| {
                    Self::build_collection_popup_menu(
                        menu,
                        &callbacks_for_action,
                        action_collection_id,
                        action_name.clone(),
                    )
                },
            ))
            .into_any_element()
    }

    fn render_folder_row(
        collection_id: Uuid,
        folder: &crate::entities::CollectionFolderNode,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
    ) -> AnyElement {
        let has_children = !folder.children.is_empty();
        let group_id = SharedString::from(format!("folder-row-{}-{}", collection_id, folder.id));
        let action_id =
            SharedString::from(format!("folder-actions-{}-{}", collection_id, folder.id));
        let row_name = folder.name.clone();
        let node_id = folder.id;
        let callbacks_for_toggle = callbacks.clone();
        let callbacks_for_menu = callbacks.clone();
        let callbacks_for_action = callbacks.clone();
        let action_name = folder.name.clone();
        let action_node_id = folder.id;
        let is_expanded = folder.expanded;

        div()
            .id(SharedString::from(format!(
                "folder-row-{}-{}",
                collection_id, folder.id
            )))
            .group(group_id.clone())
            .relative()
            .flex()
            .items_center()
            .w_full()
            .h(Self::row_height())
            .gap(px(6.0))
            .pl(px(8.0))
            .pr(px(8.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(|this| this.bg(theme.muted.opacity(0.25)))
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = callbacks_for_toggle.on_toggle_node_expand {
                    handler(collection_id, node_id, window, cx);
                }
            })
            .context_menu(move |menu, _window, _cx| {
                Self::build_folder_popup_menu(
                    menu,
                    &callbacks_for_menu,
                    collection_id,
                    node_id,
                    row_name.clone(),
                )
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
                    .text_size(px(12.5))
                    .text_ellipsis()
                    .child(folder.name.clone()),
            )
            .child(Self::render_action_button(
                theme,
                group_id,
                action_id,
                "Folder actions",
                px(6.0),
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
            .into_any_element()
    }

    fn render_request_row(
        collection_id: Uuid,
        request: &crate::entities::CollectionRequestNode,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
        app: &App,
    ) -> AnyElement {
        let group_id = SharedString::from(format!("request-row-{}-{}", collection_id, request.id));
        let action_id =
            SharedString::from(format!("request-actions-{}-{}", collection_id, request.id));
        let m_color = method_color(&request.request.method, app);
        let request_id = request.id;
        let callbacks_for_load = callbacks.clone();
        let callbacks_for_menu = callbacks.clone();
        let callbacks_for_action = callbacks.clone();
        let context_name = request.display_name();
        let action_name = request.display_name();
        let method_str = request.request.method.as_str();

        div()
            .id(SharedString::from(format!(
                "request-row-{}-{}",
                collection_id, request.id
            )))
            .group(group_id.clone())
            .relative()
            .flex()
            .items_center()
            .w_full()
            .h(Self::row_height())
            .gap(px(8.0))
            .pl(px(18.0))
            .pr(px(8.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(|this| this.bg(theme.muted.opacity(0.18)))
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = callbacks_for_load.on_load_request {
                    handler(collection_id, request_id, window, cx);
                }
            })
            .context_menu(move |menu, _window, _cx| {
                Self::build_request_popup_menu(
                    menu,
                    &callbacks_for_menu,
                    collection_id,
                    request_id,
                    context_name.clone(),
                )
            })
            .child(Self::render_method_badge(method_str, m_color))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_color(theme.foreground.opacity(0.9))
                    .text_size(px(12.5))
                    .text_ellipsis()
                    .child(request.display_name()),
            )
            .child(Self::render_action_button(
                theme,
                group_id,
                action_id,
                "Request actions",
                px(6.0),
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
            .into_any_element()
    }

    fn render_node_tree(
        collection_id: Uuid,
        node: &CollectionNode,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
        app: &App,
    ) -> AnyElement {
        match node {
            CollectionNode::Folder(folder) => {
                let mut branch = div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .child(Self::render_folder_row(
                        collection_id,
                        folder,
                        theme,
                        callbacks,
                    ));

                if folder.expanded && !folder.children.is_empty() {
                    branch = branch.child(
                        div()
                            .ml(px(12.0))
                            .pl(px(10.0))
                            .border_l_1()
                            .border_color(theme.border.opacity(0.2))
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .children(folder.children.iter().map(|child| {
                                Self::render_node_tree(collection_id, child, theme, callbacks, app)
                            })),
                    );
                }

                branch.into_any_element()
            }
            CollectionNode::Request(request) => {
                Self::render_request_row(collection_id, request, theme, callbacks, app)
            }
        }
    }

    fn render_collection_tree(
        collection: &Collection,
        theme: &gpui_component::theme::ThemeColor,
        callbacks: &PanelCallbacks,
        app: &App,
    ) -> AnyElement {
        let mut tree = div()
            .flex()
            .flex_col()
            .w_full()
            .child(Self::render_collection_row(collection, theme, callbacks));

        if collection.expanded && !collection.nodes.is_empty() {
            tree = tree.child(
                div()
                    .ml(px(12.0))
                    .pl(px(10.0))
                    .border_l_1()
                    .border_color(theme.border.opacity(0.2))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .children(collection.nodes.iter().map(|node| {
                        Self::render_node_tree(collection.id, node, theme, callbacks, app)
                    })),
            );
        }

        tree.into_any_element()
    }
}

impl RenderOnce for CollectionsPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let search_query = self.search_input.read(cx).text().to_string();
        let collections_data = self.collections.read(cx);
        let filtered_collections = collections_data.filtered_collections(&search_query);
        let is_empty = collections_data.is_empty();
        let callbacks = self.callbacks();

        let content = if is_empty {
            Self::render_empty_state(
                &theme,
                callbacks.on_new_collection.as_ref().map(Rc::clone),
                callbacks.on_import_collection.as_ref().map(Rc::clone),
            )
        } else if filtered_collections.is_empty() {
            Self::render_search_empty_state(&theme)
        } else {
            let rows = filtered_collections
                .iter()
                .map(|collection| Self::render_collection_tree(collection, &theme, &callbacks, cx))
                .collect::<Vec<_>>();

            div()
                .flex()
                .flex_col()
                .w_full()
                .gap(px(4.0))
                .children(rows)
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
                    .py(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                Icon::new(IconName::Folder)
                                    .size(px(16.0))
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_color(theme.foreground)
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .child("Collections"),
                            ),
                    )
                    .child(actions),
            )
            .child(
                div().px(px(12.0)).pb(px(8.0)).child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(10.0))
                        .py(px(6.0))
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
                    .overflow_y_scroll()
                    .overflow_x_hidden()
                    .pl(px(6.0))
                    .pr(px(4.0))
                    .pb(px(8.0))
                    .child(content),
            )
    }
}
