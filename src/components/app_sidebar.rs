use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Entity, IntoElement, Styled, Window};
use gpui_component::input::InputState;
use gpui_component::{ActiveTheme, Icon};
use std::rc::Rc;
use uuid::Uuid;

use crate::entities::{CollectionsEntity, HistoryEntity};
use crate::icons::IconName;

use super::collections_panel::CollectionsPanel;
use super::history_panel::{HistoryFilter, HistoryGroupBy, HistoryPanel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarTab {
    #[default]
    History,
    Collections,
}

#[derive(IntoElement)]
pub struct AppSidebar {
    active_tab: SidebarTab,
    history: Entity<HistoryEntity>,
    collections: Entity<CollectionsEntity>,
    history_search: Entity<InputState>,
    collections_search: Entity<InputState>,
    history_filter: HistoryFilter,
    history_group_by: HistoryGroupBy,
    on_tab_change: Option<Rc<dyn Fn(SidebarTab, &mut Window, &mut App) + 'static>>,
    on_load_history_request: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_history_entry: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_toggle_star: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_clear_history: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_load_collection_request: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_collection: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_collection_item: Option<Rc<dyn Fn(Uuid, Uuid, &mut Window, &mut App) + 'static>>,
    on_new_collection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_toggle_collection_expand: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
}

impl AppSidebar {
    pub fn new(
        history: Entity<HistoryEntity>,
        collections: Entity<CollectionsEntity>,
        history_search: Entity<InputState>,
        collections_search: Entity<InputState>,
    ) -> Self {
        Self {
            active_tab: SidebarTab::History,
            history,
            collections,
            history_search,
            collections_search,
            history_filter: HistoryFilter::All,
            history_group_by: HistoryGroupBy::Time,
            on_tab_change: None,
            on_load_history_request: None,
            on_delete_history_entry: None,
            on_toggle_star: None,
            on_clear_history: None,
            on_load_collection_request: None,
            on_delete_collection: None,
            on_delete_collection_item: None,
            on_new_collection: None,
            on_toggle_collection_expand: None,
        }
    }

    pub fn active_tab(mut self, tab: SidebarTab) -> Self {
        self.active_tab = tab;
        self
    }

    pub fn on_tab_change(
        mut self,
        f: impl Fn(SidebarTab, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_tab_change = Some(Rc::new(f));
        self
    }

    pub fn on_load_history_request(
        mut self,
        f: impl Fn(Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_load_history_request = Some(Rc::new(f));
        self
    }

    pub fn on_delete_history_entry(
        mut self,
        f: impl Fn(Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete_history_entry = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_star(mut self, f: impl Fn(Uuid, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_star = Some(Rc::new(f));
        self
    }

    pub fn on_clear_history(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear_history = Some(Rc::new(f));
        self
    }

    pub fn on_load_collection_request(
        mut self,
        f: impl Fn(Uuid, Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_load_collection_request = Some(Rc::new(f));
        self
    }

    pub fn on_delete_collection(
        mut self,
        f: impl Fn(Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete_collection = Some(Rc::new(f));
        self
    }

    pub fn on_delete_collection_item(
        mut self,
        f: impl Fn(Uuid, Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete_collection_item = Some(Rc::new(f));
        self
    }

    pub fn on_new_collection(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_collection = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_collection_expand(
        mut self,
        f: impl Fn(Uuid, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_collection_expand = Some(Rc::new(f));
        self
    }

    fn build_history_panel(&self) -> HistoryPanel {
        let mut panel = HistoryPanel::new(self.history.clone(), self.history_search.clone())
            .filter(self.history_filter)
            .group_by(self.history_group_by);

        if let Some(ref f) = self.on_load_history_request {
            let f = Rc::clone(f);
            panel = panel.on_load_request(move |id, window, cx| f(id, window, cx));
        }

        if let Some(ref f) = self.on_delete_history_entry {
            let f = Rc::clone(f);
            panel = panel.on_delete_entry(move |id, window, cx| f(id, window, cx));
        }

        if let Some(ref f) = self.on_toggle_star {
            let f = Rc::clone(f);
            panel = panel.on_toggle_star(move |id, window, cx| f(id, window, cx));
        }

        if let Some(ref f) = self.on_clear_history {
            let f = Rc::clone(f);
            panel = panel.on_clear(move |window, cx| f(window, cx));
        }

        panel
    }

    fn build_collections_panel(&self) -> CollectionsPanel {
        let mut panel =
            CollectionsPanel::new(self.collections.clone(), self.collections_search.clone());

        if let Some(ref f) = self.on_load_collection_request {
            let f = Rc::clone(f);
            panel = panel.on_load_request(move |coll_id, item_id, window, cx| {
                f(coll_id, item_id, window, cx)
            });
        }

        if let Some(ref f) = self.on_delete_collection {
            let f = Rc::clone(f);
            panel = panel.on_delete_collection(move |id, window, cx| f(id, window, cx));
        }

        if let Some(ref f) = self.on_delete_collection_item {
            let f = Rc::clone(f);
            panel = panel.on_delete_item(move |coll_id, item_id, window, cx| {
                f(coll_id, item_id, window, cx)
            });
        }

        if let Some(ref f) = self.on_new_collection {
            let f = Rc::clone(f);
            panel = panel.on_new_collection(move |window, cx| f(window, cx));
        }

        if let Some(ref f) = self.on_toggle_collection_expand {
            let f = Rc::clone(f);
            panel = panel.on_toggle_expand(move |id, window, cx| f(id, window, cx));
        }

        panel
    }

    fn render_icon_button(
        &self,
        tab: SidebarTab,
        icon: IconName,
        theme: &gpui_component::theme::ThemeColor,
    ) -> AnyElement {
        let is_active = self.active_tab == tab;
        let handler = self.on_tab_change.clone();

        div()
            .id(match tab {
                SidebarTab::History => "history-tab-btn",
                SidebarTab::Collections => "collections-tab-btn",
            })
            .flex()
            .items_center()
            .justify_center()
            .size(px(40.0))
            .cursor_pointer()
            .rounded(px(8.0))
            .when(is_active, |el| el.bg(theme.primary.opacity(0.15)))
            .hover(|el| el.bg(theme.muted))
            .child(Icon::new(icon).size(px(20.0)).text_color(if is_active {
                theme.primary
            } else {
                theme.muted_foreground
            }))
            .on_click(move |_, window, cx| {
                if let Some(ref f) = handler {
                    f(tab, window, cx);
                }
            })
            .into_any_element()
    }
}

impl RenderOnce for AppSidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let panel = match self.active_tab {
            SidebarTab::History => self.build_history_panel().into_any_element(),
            SidebarTab::Collections => self.build_collections_panel().into_any_element(),
        };

        div()
            .flex()
            .flex_row()
            .h_full()
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .w(px(52.0))
                    .h_full()
                    .py(px(12.0))
                    .gap(px(8.0))
                    .border_r_1()
                    .border_color(theme.border)
                    .child(self.render_icon_button(SidebarTab::History, IconName::History, &theme))
                    .child(self.render_icon_button(
                        SidebarTab::Collections,
                        IconName::Folder,
                        &theme,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .bg(theme.sidebar)
                    .border_r_1()
                    .border_color(theme.border)
                    .child(panel),
            )
    }
}
