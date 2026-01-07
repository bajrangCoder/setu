use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, ScrollHandle, SharedString, Styled, Window};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::IconName;

use crate::entities::HttpMethod;
use crate::theme::Theme;

/// A single tab in the tab bar
#[derive(Clone)]
pub struct TabInfo {
    pub id: usize,
    pub index: usize, // Position in tabs array
    pub name: SharedString,
    pub method: HttpMethod,
    pub is_active: bool,
}

impl TabInfo {
    pub fn new(id: usize, index: usize, name: impl Into<SharedString>, method: HttpMethod) -> Self {
        Self {
            id,
            index,
            name: name.into(),
            method,
            is_active: false,
        }
    }

    pub fn active(mut self) -> Self {
        self.is_active = true;
        self
    }
}

/// Tab bar component - original custom implementation with colored method badges
#[derive(IntoElement)]
pub struct TabBar {
    tabs: Vec<TabInfo>,
    main_view: Entity<crate::views::MainView>,
    scroll_handle: ScrollHandle,
}

impl TabBar {
    pub fn new(
        tabs: Vec<TabInfo>,
        main_view: Entity<crate::views::MainView>,
        scroll_handle: ScrollHandle,
    ) -> Self {
        Self {
            tabs,
            main_view,
            scroll_handle,
        }
    }
}

impl RenderOnce for TabBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::dark();
        let main_view = self.main_view;
        let main_view_for_new = main_view.clone();

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(36.0))
            .bg(theme.colors.bg_secondary)
            .border_b_1()
            .border_color(theme.colors.border_primary)
            // Tabs - horizontally scrollable
            .child(
                div()
                    .id("tab-scroll")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .flex_1()
                    .overflow_x_scroll()
                    .track_scroll(&self.scroll_handle)
                    .children(self.tabs.into_iter().map(|tab| {
                        let index = tab.index;
                        let main_view_for_click = main_view.clone();
                        let main_view_for_close = main_view.clone();
                        let main_view_for_context = main_view.clone();

                        Tab::new(tab, main_view_for_context)
                            .on_click(move |_event, _window, cx| {
                                main_view_for_click.update(cx, |view, cx| {
                                    view.switch_tab(index, cx);
                                });
                            })
                            .on_close(move |_event, _window, cx| {
                                main_view_for_close.update(cx, |view, cx| {
                                    view.close_tab(index, cx);
                                });
                            })
                    })),
            )
            // New tab button
            .child(
                div()
                    .id("new-tab-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(32.0))
                    .h(px(28.0))
                    .mx(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_color(theme.colors.text_muted)
                    .text_size(px(14.0))
                    .hover(|s| {
                        s.bg(theme.colors.bg_tertiary)
                            .text_color(theme.colors.text_secondary)
                    })
                    .on_click(move |_event, _window, cx| {
                        main_view_for_new.update(cx, |view, cx| {
                            view.new_tab(cx);
                        });
                    })
                    .child(IconName::Plus),
            )
    }
}

/// Single tab component with click handler
pub struct Tab {
    info: TabInfo,
    main_view: Entity<crate::views::MainView>,
    on_click: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
    on_close: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl Tab {
    pub fn new(info: TabInfo, main_view: Entity<crate::views::MainView>) -> Self {
        Self {
            info,
            main_view,
            on_click: None,
            on_close: None,
        }
    }

    pub fn on_click(
        mut self,
        callback: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(callback));
        self
    }

    pub fn on_close(
        mut self,
        callback: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(callback));
        self
    }

    fn method_color(&self, theme: &Theme) -> gpui::Hsla {
        match self.info.method {
            HttpMethod::Get => theme.colors.method_get,
            HttpMethod::Post => theme.colors.method_post,
            HttpMethod::Put => theme.colors.method_put,
            HttpMethod::Delete => theme.colors.method_delete,
            HttpMethod::Patch => theme.colors.method_patch,
            HttpMethod::Head => theme.colors.method_head,
            HttpMethod::Options => theme.colors.method_options,
        }
    }
}

impl IntoElement for Tab {
    type Element = gpui::AnyElement;

    fn into_element(self) -> Self::Element {
        let theme = Theme::dark();
        let method_color = self.method_color(&theme);
        let is_active = self.info.is_active;
        let tab_id = self.info.id;
        let tab_index = self.info.index;
        let tab_name = self.info.name.clone();
        let main_view = self.main_view.clone();

        // Clone main_view for context menu closures
        let main_view_for_rename = main_view.clone();
        let main_view_for_close = main_view.clone();
        let main_view_for_close_others = main_view.clone();

        div()
            .id(("tab", tab_id))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(28.0))
            .px(px(10.0))
            .mx(px(2.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .when(is_active, |s| s.bg(theme.colors.bg_tertiary))
            .when(!is_active, |s| {
                s.hover(|s| s.bg(theme.colors.bg_tertiary.opacity(0.5)))
            })
            .when_some(self.on_click, |el, callback| {
                el.on_click(move |event, window, cx| {
                    callback(event, window, cx);
                })
            })
            // Method badge
            .child(
                div()
                    .text_color(method_color)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(9.0))
                    .child(self.info.method.as_str()),
            )
            // Tab name
            .child(
                div()
                    .text_color(if is_active {
                        theme.colors.text_primary
                    } else {
                        theme.colors.text_muted
                    })
                    .text_size(px(11.0))
                    .child(self.info.name),
            )
            // Close button - use group for hover
            .group("tab")
            .when_some(self.on_close, |el, on_close| {
                el.child(
                    div()
                        .id(("tab-close", tab_id))
                        .ml(px(4.0))
                        .w(px(14.0))
                        .h(px(14.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(2.0))
                        .text_color(theme.colors.text_muted)
                        .text_size(px(10.0))
                        .cursor_pointer()
                        // Hide by default for inactive tabs, show on group hover
                        .when(!is_active, |s| {
                            s.invisible().group_hover("tab", |s| s.visible())
                        })
                        .hover(|s| {
                            s.bg(theme.colors.bg_secondary)
                                .text_color(theme.colors.text_secondary)
                        })
                        .on_click(move |event, window, cx| {
                            // Stop propagation to prevent tab switch
                            cx.stop_propagation();
                            on_close(event, window, cx);
                        })
                        .child(IconName::Close),
                )
            })
            // Context menu for right-click
            .context_menu(move |menu, _window, _cx| {
                let tab_name_for_rename = tab_name.to_string();
                let main_view_rename = main_view_for_rename.clone();
                let main_view_close = main_view_for_close.clone();
                let main_view_close_others = main_view_for_close_others.clone();

                menu.item(PopupMenuItem::new("Rename").icon(IconName::Menu).on_click(
                    move |_event, window, cx| {
                        let current_name = tab_name_for_rename.clone();
                        main_view_rename.update(cx, |view, cx| {
                            view.show_rename_dialog(tab_index, current_name, window, cx);
                        });
                    },
                ))
                .separator()
                .item(PopupMenuItem::new("Close").icon(IconName::Close).on_click(
                    move |_event, _window, cx| {
                        main_view_close.update(cx, |view, cx| {
                            view.close_tab(tab_index, cx);
                        });
                    },
                ))
                .item(
                    PopupMenuItem::new("Close Others")
                        .icon(IconName::CircleX)
                        .on_click(move |_event, _window, cx| {
                            main_view_close_others.update(cx, |view, cx| {
                                view.close_other_tabs(tab_index, cx);
                            });
                        }),
                )
            })
            .into_any_element()
    }
}
