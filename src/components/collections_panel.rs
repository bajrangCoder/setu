use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Entity, IntoElement, Styled, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Icon, Sizable};
use std::rc::Rc;
use uuid::Uuid;

use crate::entities::CollectionsEntity;
use crate::icons::IconName;
use crate::theme::method_color;

#[derive(IntoElement)]
pub struct CollectionsPanel {
    collections: Entity<CollectionsEntity>,
    search_input: Entity<InputState>,
    on_load_request: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_collection: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_item: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_new_collection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_toggle_expand: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
}

impl CollectionsPanel {
    pub fn new(collections: Entity<CollectionsEntity>, search_input: Entity<InputState>) -> Self {
        Self {
            collections,
            search_input,
            on_load_request: None,
            on_delete_collection: None,
            on_delete_item: None,
            on_new_collection: None,
            on_toggle_expand: None,
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

    pub fn on_delete_item(
        mut self,
        f: impl Fn(Uuid, Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete_item = Some(Rc::new(f));
        self
    }

    pub fn on_new_collection(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_collection = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_expand(mut self, f: impl Fn(Uuid, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_expand = Some(Rc::new(f));
        self
    }

    fn render_empty_state(
        theme: &gpui_component::theme::ThemeColor,
        on_new: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
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

        container.into_any_element()
    }
}

impl RenderOnce for CollectionsPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let collections_data = self.collections.read(cx);
        let is_empty = collections_data.is_empty();

        let search_query = self.search_input.read(cx).text().to_string().to_lowercase();

        let on_new_clone: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>> =
            self.on_new_collection.as_ref().map(Rc::clone);

        let on_toggle = self.on_toggle_expand.clone();
        let on_delete_collection = self.on_delete_collection.clone();
        let on_load = self.on_load_request.clone();
        let on_delete_item = self.on_delete_item.clone();

        let list_hover = theme.list_hover;

        let content = if is_empty {
            Self::render_empty_state(&theme, on_new_clone)
        } else {
            let mut items: Vec<AnyElement> = Vec::new();
            let mut collection_idx = 0;

            for collection in &collections_data.collections {
                let matches_search = search_query.is_empty()
                    || collection.name.to_lowercase().contains(&search_query)
                    || collection.items.iter().any(|i| {
                        i.request.url.to_lowercase().contains(&search_query)
                            || i.request.name.to_lowercase().contains(&search_query)
                    });

                if !matches_search {
                    continue;
                }

                let collection_id = collection.id;
                let is_expanded = collection.expanded;
                let item_count = collection.items.len();
                let on_toggle_clone = on_toggle.clone();
                let on_delete_clone = on_delete_collection.clone();

                items.push(
                    div()
                        .id(("collection-header", collection_idx))
                        .group("collection-header")
                        .flex()
                        .flex_row()
                        .items_center()
                        .w_full()
                        .gap(px(8.0))
                        .px(px(12.0))
                        .py(px(10.0))
                        .cursor_pointer()
                        .rounded(px(6.0))
                        .hover(|el| el.bg(list_hover))
                        .child(
                            Icon::new(if is_expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .size(px(14.0))
                            .text_color(theme.muted_foreground),
                        )
                        .child(
                            Icon::new(if is_expanded {
                                IconName::FolderOpen
                            } else {
                                IconName::Folder
                            })
                            .size(px(16.0))
                            .text_color(theme.warning),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(theme.foreground)
                                .text_size(px(13.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(collection.name.clone()),
                        )
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .text_size(px(11.0))
                                .child(format!("{}", item_count)),
                        )
                        .child({
                            let del_handler = on_delete_clone.clone();
                            div()
                                .opacity(0.0)
                                .group_hover("collection-header", |s| s.opacity(1.0))
                                .when_some(del_handler, |el, handler| {
                                    el.child(
                                        Button::new(("delete-collection", collection_idx))
                                            .ghost()
                                            .xsmall()
                                            .icon(Icon::new(IconName::Trash).size(px(12.0)))
                                            .on_click(move |_, window, cx| {
                                                cx.stop_propagation();
                                                handler(collection_id, window, cx);
                                            }),
                                    )
                                })
                        })
                        .on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_toggle_clone {
                                handler(collection_id, window, cx);
                            }
                        })
                        .into_any_element(),
                );

                if is_expanded {
                    for (item_idx, item) in collection.items.iter().enumerate() {
                        let item_matches = search_query.is_empty()
                            || item.request.url.to_lowercase().contains(&search_query)
                            || item.request.name.to_lowercase().contains(&search_query);

                        if !item_matches && !search_query.is_empty() {
                            continue;
                        }

                        let method = item.request.method;
                        let method_str = method.as_str().to_string();
                        let m_color = method_color(&method, cx);
                        let display_name = item.display_name();
                        let item_id = item.id;
                        let on_load_clone = on_load.clone();
                        let on_delete_clone = on_delete_item.clone();

                        items.push(
                            div()
                                .id(("collection-item", collection_idx * 1000 + item_idx))
                                .group("collection-item")
                                .flex()
                                .flex_row()
                                .items_center()
                                .w_full()
                                .gap(px(10.0))
                                .pl(px(48.0))
                                .pr(px(12.0))
                                .py(px(8.0))
                                .cursor_pointer()
                                .rounded(px(6.0))
                                .hover(|el| el.bg(list_hover))
                                .child(
                                    div()
                                        .min_w(px(42.0))
                                        .px(px(8.0))
                                        .py(px(3.0))
                                        .bg(m_color.opacity(0.15))
                                        .rounded(px(4.0))
                                        .text_color(m_color)
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_size(px(10.0))
                                        .text_center()
                                        .child(method_str),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(theme.foreground)
                                        .text_size(px(12.0))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(display_name),
                                )
                                .child({
                                    let del_handler = on_delete_clone.clone();
                                    div()
                                        .opacity(0.0)
                                        .group_hover("collection-item", |s| s.opacity(1.0))
                                        .when_some(del_handler, |el, handler| {
                                            el.child(
                                                Button::new(("delete-item", item_idx))
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(Icon::new(IconName::Trash).size(px(12.0)))
                                                    .on_click(move |_, window, cx| {
                                                        cx.stop_propagation();
                                                        handler(collection_id, item_id, window, cx);
                                                    }),
                                            )
                                        })
                                })
                                .on_click(move |_, window, cx| {
                                    if let Some(ref handler) = on_load_clone {
                                        handler(collection_id, item_id, window, cx);
                                    }
                                })
                                .into_any_element(),
                        );
                    }
                }

                collection_idx += 1;
            }

            div().flex().flex_col().children(items).into_any_element()
        };

        let mut new_btn_wrapper = div();
        if let Some(handler) = self.on_new_collection {
            new_btn_wrapper = new_btn_wrapper.child(
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
                            )
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(theme.warning.opacity(0.15))
                                    .text_color(theme.warning)
                                    .text_size(px(9.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("WIP"),
                            ),
                    )
                    .child(new_btn_wrapper),
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
                    .px(px(4.0))
                    .child(content),
            )
    }
}
