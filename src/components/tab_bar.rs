use gpui::prelude::*;
use gpui::{App, Entity, Hsla, IntoElement, ScrollHandle, SharedString, Styled, Window, div, px};
use gpui_component::ActiveTheme;
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};

use crate::entities::HttpMethod;
use crate::icons::IconName;
use crate::theme::method_color;

#[derive(Clone, Debug)]
pub enum TabIcon {
    Method(HttpMethod),
    Icon(IconName),
}

#[derive(Clone)]
pub struct TabInfo {
    pub id: usize,
    pub index: usize,
    pub name: SharedString,
    pub icon: TabIcon,
    pub is_active: bool,
}

impl TabInfo {
    pub fn new(id: usize, index: usize, name: impl Into<SharedString>, icon: TabIcon) -> Self {
        Self {
            id,
            index,
            name: name.into(),
            icon,
            is_active: false,
        }
    }

    #[allow(dead_code)]
    pub fn with_method(
        id: usize,
        index: usize,
        name: impl Into<SharedString>,
        method: HttpMethod,
    ) -> Self {
        Self::new(id, index, name, TabIcon::Method(method))
    }

    pub fn active(mut self) -> Self {
        self.is_active = true;
        self
    }
}

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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let main_view = self.main_view;
        let main_view_for_new = main_view.clone();

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(36.0))
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
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

                        RequestTab::new(tab, main_view_for_context, cx)
                            .on_click(move |_, _, cx| {
                                main_view_for_click
                                    .update(cx, |view, cx| view.switch_tab(index, cx));
                            })
                            .on_close(move |_, _, cx| {
                                main_view_for_close
                                    .update(cx, |view, cx| view.close_tab(index, cx));
                            })
                    })),
            )
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
                    .text_color(theme.muted_foreground)
                    .text_size(px(14.0))
                    .hover(|style| style.bg(theme.muted).text_color(theme.secondary_foreground))
                    .on_click(move |_, _, cx| {
                        main_view_for_new.update(cx, |view, cx| view.new_tab(cx));
                    })
                    .child(IconName::Plus),
            )
    }
}

#[derive(IntoElement)]
struct RequestTab {
    info: TabInfo,
    main_view: Entity<crate::views::MainView>,
    bg_active: Hsla,
    bg_hover: Hsla,
    text_active: Hsla,
    text_inactive: Hsla,
    on_click: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
    on_close: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl RequestTab {
    fn new(info: TabInfo, main_view: Entity<crate::views::MainView>, cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            info,
            main_view,
            bg_active: theme.muted,
            bg_hover: theme.accent,
            text_active: theme.foreground,
            text_inactive: theme.muted_foreground,
            on_click: None,
            on_close: None,
        }
    }

    fn on_click(
        mut self,
        callback: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(callback));
        self
    }

    fn on_close(
        mut self,
        callback: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(callback));
        self
    }
}

impl RenderOnce for RequestTab {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let is_active = self.info.is_active;
        let tab_id = self.info.id;
        let tab_index = self.info.index;
        let tab_name = self.info.name.clone();
        let main_view_for_rename = self.main_view.clone();
        let main_view_for_close = self.main_view.clone();
        let main_view_for_close_others = self.main_view;

        let icon_badge = match &self.info.icon {
            TabIcon::Method(method) => {
                let m_color = method_color(method, cx);
                div()
                    .text_color(m_color)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(9.0))
                    .child(method.as_str())
                    .into_any_element()
            }
            TabIcon::Icon(icon) => div()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    gpui_component::Icon::new(*icon)
                        .size(px(12.0))
                        .text_color(theme.primary),
                )
                .into_any_element(),
        };

        let text_color = if is_active {
            self.text_active
        } else {
            self.text_inactive
        };

        div()
            .id(("tab", tab_id))
            .group("tab")
            .flex()
            .items_center()
            .gap(px(6.0))
            .h(px(28.0))
            .px(px(10.0))
            .mx(px(2.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .when(is_active, |style| style.bg(self.bg_active))
            .when(!is_active, |style| {
                style.hover(|hover| hover.bg(self.bg_hover.opacity(0.3)))
            })
            .when_some(self.on_click, |element, callback| {
                element.on_click(move |event, window, cx| callback(event, window, cx))
            })
            .child(icon_badge)
            .child(
                div()
                    .max_w(px(180.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_color(text_color)
                    .text_size(px(11.0))
                    .child(self.info.name),
            )
            .when_some(self.on_close, |element, on_close| {
                element.child(
                    div()
                        .id(("tab-close", tab_id))
                        .ml(px(4.0))
                        .w(px(14.0))
                        .h(px(14.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(2.0))
                        .text_size(px(10.0))
                        .cursor_pointer()
                        .text_color(text_color)
                        .when(!is_active, |style| {
                            style
                                .invisible()
                                .group_hover("tab", |hover| hover.visible())
                        })
                        .on_click(move |event, window, cx| {
                            cx.stop_propagation();
                            on_close(event, window, cx);
                        })
                        .child(IconName::Close),
                )
            })
            .context_menu(move |menu, _, _| {
                let tab_name = tab_name.to_string();
                let main_view_for_rename = main_view_for_rename.clone();
                let main_view_for_close = main_view_for_close.clone();
                let main_view_for_close_others = main_view_for_close_others.clone();

                menu.item(
                    PopupMenuItem::new("Rename")
                        .icon(IconName::FilePen)
                        .on_click(move |_, window, cx| {
                            main_view_for_rename.update(cx, |view, cx| {
                                view.show_rename_dialog(tab_index, tab_name.clone(), window, cx);
                            });
                        }),
                )
                .separator()
                .item(PopupMenuItem::new("Close").icon(IconName::Close).on_click(
                    move |_, _, cx| {
                        main_view_for_close.update(cx, |view, cx| view.close_tab(tab_index, cx));
                    },
                ))
                .item(
                    PopupMenuItem::new("Close Others")
                        .icon(IconName::CircleX)
                        .on_click(move |_, _, cx| {
                            main_view_for_close_others
                                .update(cx, |view, cx| view.close_other_tabs(tab_index, cx));
                        }),
                )
            })
    }
}
