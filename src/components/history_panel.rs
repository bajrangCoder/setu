use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Entity, Hsla, IntoElement, Styled, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme, Icon, Selectable, Sizable};
use std::rc::Rc;
use uuid::Uuid;

use crate::entities::{HistoryEntity, HistoryEntry, TimeGroup};
use crate::icons::IconName;
use crate::theme::method_color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryFilter {
    #[default]
    All,
    Starred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryGroupBy {
    #[default]
    Time,
    Url,
}

#[derive(IntoElement)]
pub struct HistoryPanel {
    history: Entity<HistoryEntity>,
    search_input: Entity<InputState>,
    filter: HistoryFilter,
    group_by: HistoryGroupBy,
    on_load_request: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_delete_entry: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_toggle_star: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_clear: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_filter_change: Option<Rc<dyn Fn(HistoryFilter, &mut Window, &mut App) + 'static>>,
    on_group_by_change: Option<Rc<dyn Fn(HistoryGroupBy, &mut Window, &mut App) + 'static>>,
}

impl HistoryPanel {
    pub fn new(history: Entity<HistoryEntity>, search_input: Entity<InputState>) -> Self {
        Self {
            history,
            search_input,
            filter: HistoryFilter::All,
            group_by: HistoryGroupBy::Time,
            on_load_request: None,
            on_delete_entry: None,
            on_toggle_star: None,
            on_clear: None,
            on_filter_change: None,
            on_group_by_change: None,
        }
    }

    pub fn filter(mut self, filter: HistoryFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn group_by(mut self, group_by: HistoryGroupBy) -> Self {
        self.group_by = group_by;
        self
    }

    pub fn on_load_request(mut self, f: impl Fn(Uuid, &mut Window, &mut App) + 'static) -> Self {
        self.on_load_request = Some(Rc::new(f));
        self
    }

    pub fn on_delete_entry(mut self, f: impl Fn(Uuid, &mut Window, &mut App) + 'static) -> Self {
        self.on_delete_entry = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_star(mut self, f: impl Fn(Uuid, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_star = Some(Rc::new(f));
        self
    }

    pub fn on_clear(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(Rc::new(f));
        self
    }

    pub fn on_filter_change(
        mut self,
        f: impl Fn(HistoryFilter, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_filter_change = Some(Rc::new(f));
        self
    }

    pub fn on_group_by_change(
        mut self,
        f: impl Fn(HistoryGroupBy, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_group_by_change = Some(Rc::new(f));
        self
    }

    fn render_empty_state(
        theme: &gpui_component::theme::ThemeColor,
        has_query: bool,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h_full()
            .py(px(48.0))
            .gap(px(12.0))
            .child(
                Icon::new(IconName::History)
                    .size(px(40.0))
                    .text_color(theme.muted_foreground.opacity(0.5)),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(13.0))
                    .child(if has_query {
                        "No matching requests"
                    } else {
                        "No history yet"
                    }),
            )
            .into_any_element()
    }

    fn render_group_header(
        group: TimeGroup,
        is_collapsed: bool,
        count: usize,
        theme: &gpui_component::theme::ThemeColor,
        history: Entity<HistoryEntity>,
    ) -> AnyElement {
        let chevron_icon = if is_collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };

        let list_hover = theme.list_hover;

        div()
            .id(gpui::SharedString::from(format!(
                "history-group-{}",
                group.label()
            )))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(6.0))
            .mt(px(8.0))
            .px(px(12.0))
            .py(px(8.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(move |s| s.bg(list_hover))
            .child(
                Icon::new(chevron_icon)
                    .size(px(14.0))
                    .text_color(theme.muted_foreground),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(group.label().to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground.opacity(0.5))
                    .text_size(px(11.0))
                    .child(format!("({})", count)),
            )
            .on_click(move |_, _window, cx| {
                history.update(cx, |h, cx| {
                    h.toggle_group_collapsed(group, cx);
                });
            })
            .into_any_element()
    }

    fn render_url_group_header(
        domain: &str,
        is_collapsed: bool,
        count: usize,
        theme: &gpui_component::theme::ThemeColor,
    ) -> AnyElement {
        let chevron_icon = if is_collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };

        let list_hover = theme.list_hover;

        div()
            .id(gpui::SharedString::from(format!("url-group-{}", domain)))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(6.0))
            .mt(px(8.0))
            .px(px(12.0))
            .py(px(8.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(move |s| s.bg(list_hover))
            .child(
                Icon::new(chevron_icon)
                    .size(px(14.0))
                    .text_color(theme.muted_foreground),
            )
            .child(
                Icon::new(IconName::Link)
                    .size(px(12.0))
                    .text_color(theme.muted_foreground),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(domain.to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground.opacity(0.5))
                    .text_size(px(11.0))
                    .child(format!("({})", count)),
            )
            .into_any_element()
    }

    fn render_history_item(
        ix: usize,
        entry: &HistoryEntry,
        theme: &gpui_component::theme::ThemeColor,
        m_color: Hsla,
        on_load: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
        on_delete: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
        on_star: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    ) -> AnyElement {
        let method_str = entry.request.method.as_str().to_string();
        let is_starred = entry.starred;
        let full_timestamp = entry.timestamp.format("%b %d, %Y at %H:%M:%S").to_string();
        let url_display: String = if entry.request.url.is_empty() {
            "No URL".to_string()
        } else {
            entry.request.url.clone()
        };
        let entry_id = entry.id;

        let star_icon = if is_starred {
            IconName::StarFilled
        } else {
            IconName::Star
        };

        let on_load_clone = on_load.clone();
        let on_delete_clone = on_delete.clone();
        let on_star_clone = on_star.clone();

        let warning_color = theme.warning;
        let danger_color = theme.danger;
        let foreground = theme.foreground;
        let list_hover = theme.list_hover;

        div()
            .id(("history-item", ix))
            .group("history-item")
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(10.0))
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(|el| el.bg(list_hover))
            .child(
                div()
                    .min_w(px(42.0))
                    .flex_shrink_0()
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
                    .id(("url-tooltip", ix))
                    .flex_1()
                    .overflow_hidden()
                    .tooltip(move |window, cx| {
                        Tooltip::new(full_timestamp.clone()).build(window, cx)
                    })
                    .child(
                        div()
                            .text_color(foreground)
                            .text_size(px(12.0))
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(url_display),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(8.0))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .px(px(4.0))
                    .opacity(0.0)
                    .group_hover("history-item", |s| s.opacity(1.0).bg(list_hover))
                    .child({
                        let handler = on_star_clone.clone();
                        let mut btn = Button::new(("star", ix))
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(star_icon).size(px(14.0)))
                            .tooltip(if is_starred {
                                "Remove from favorites"
                            } else {
                                "Add to favorites"
                            });
                        if is_starred {
                            btn = btn.text_color(warning_color);
                        }
                        if let Some(h) = handler {
                            btn = btn.on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                h(entry_id, window, cx);
                            });
                        }
                        btn
                    })
                    .child({
                        let handler = on_delete_clone.clone();
                        let mut btn = Button::new(("delete", ix))
                            .ghost()
                            .xsmall()
                            .icon(
                                Icon::new(IconName::Trash)
                                    .size(px(14.0))
                                    .text_color(danger_color),
                            )
                            .tooltip("Delete");
                        if let Some(h) = handler {
                            btn = btn.on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                h(entry_id, window, cx);
                            });
                        }
                        btn
                    }),
            )
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = on_load_clone {
                    handler(entry_id, window, cx);
                }
            })
            .into_any_element()
    }
}

impl RenderOnce for HistoryPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let history = self.history.read(cx);
        let search_query = self.search_input.read(cx).text().to_string();

        let filtered_entries: Vec<&HistoryEntry> = if search_query.is_empty() {
            match self.filter {
                HistoryFilter::All => history.entries.iter().collect(),
                HistoryFilter::Starred => history.starred(),
            }
        } else {
            let searched = history.search(&search_query);
            match self.filter {
                HistoryFilter::All => searched,
                HistoryFilter::Starred => searched.into_iter().filter(|e| e.starred).collect(),
            }
        };

        let is_empty = filtered_entries.is_empty();
        let has_query = !search_query.is_empty();

        let entry_colors: Vec<(Uuid, Hsla)> = filtered_entries
            .iter()
            .map(|e| (e.id, method_color(&e.request.method, cx)))
            .collect();

        let theme = cx.theme();

        let on_load = self.on_load_request.clone();
        let on_delete = self.on_delete_entry.clone();
        let on_star = self.on_toggle_star.clone();

        let current_filter = self.filter;
        let current_group_by = self.group_by;

        let content = if is_empty {
            Self::render_empty_state(&theme, has_query)
        } else {
            let mut items: Vec<AnyElement> = Vec::new();

            match self.group_by {
                HistoryGroupBy::Time => {
                    let grouped = history.grouped_entries();
                    for (group, group_entries) in grouped {
                        let matching_entries: Vec<_> = group_entries
                            .iter()
                            .filter(|e| filtered_entries.iter().any(|fe| fe.id == e.id))
                            .collect();

                        if matching_entries.is_empty() {
                            continue;
                        }

                        let is_collapsed = history.is_group_collapsed(&group);
                        items.push(Self::render_group_header(
                            group,
                            is_collapsed,
                            matching_entries.len(),
                            &theme,
                            self.history.clone(),
                        ));

                        if !is_collapsed {
                            for (idx, entry) in matching_entries.iter().enumerate() {
                                let m_color = entry_colors
                                    .iter()
                                    .find(|(id, _)| *id == entry.id)
                                    .map(|(_, c)| *c)
                                    .unwrap_or(theme.foreground);

                                items.push(Self::render_history_item(
                                    idx,
                                    entry,
                                    &theme,
                                    m_color,
                                    on_load.clone(),
                                    on_delete.clone(),
                                    on_star.clone(),
                                ));
                            }
                        }
                    }
                }
                HistoryGroupBy::Url => {
                    let grouped = history.grouped_by_url();
                    for (domain, group_entries) in grouped {
                        let matching_entries: Vec<_> = group_entries
                            .iter()
                            .filter(|e| filtered_entries.iter().any(|fe| fe.id == e.id))
                            .collect();

                        if matching_entries.is_empty() {
                            continue;
                        }

                        items.push(Self::render_url_group_header(
                            &domain,
                            false,
                            matching_entries.len(),
                            &theme,
                        ));

                        for (idx, entry) in matching_entries.iter().enumerate() {
                            let m_color = entry_colors
                                .iter()
                                .find(|(id, _)| *id == entry.id)
                                .map(|(_, c)| *c)
                                .unwrap_or(theme.foreground);

                            items.push(Self::render_history_item(
                                idx,
                                entry,
                                &theme,
                                m_color,
                                on_load.clone(),
                                on_delete.clone(),
                                on_star.clone(),
                            ));
                        }
                    }
                }
            }

            div().flex().flex_col().children(items).into_any_element()
        };

        let mut clear_btn = div();
        if let Some(on_clear) = self.on_clear {
            clear_btn = clear_btn.child(
                Button::new("clear-history")
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(IconName::Trash).size(px(14.0)))
                    .tooltip("Clear History")
                    .on_click(move |_, window, cx| on_clear(window, cx)),
            );
        }

        let filter_active =
            current_filter != HistoryFilter::All || current_group_by != HistoryGroupBy::Time;

        let on_filter_change = self.on_filter_change.clone();
        let on_group_by_change = self.on_group_by_change.clone();

        let filter_button = {
            let on_filter_all = on_filter_change.clone();
            let on_filter_starred = on_filter_change.clone();
            let on_group_time = on_group_by_change.clone();
            let on_group_url = on_group_by_change.clone();

            Button::new("filter-funnel")
                .ghost()
                .xsmall()
                .icon(Icon::new(IconName::Funnel).size(px(14.0)))
                .tooltip("Filter & Group")
                .when(filter_active, |btn| btn.selected(true))
                .dropdown_menu(move |menu, _window, _cx| {
                    let on_filter_all = on_filter_all.clone();
                    let on_filter_starred = on_filter_starred.clone();
                    let on_group_time = on_group_time.clone();
                    let on_group_url = on_group_url.clone();

                    menu.label("Filter")
                        .item({
                            let handler = on_filter_all.clone();
                            let mut item = PopupMenuItem::new("All");
                            if current_filter == HistoryFilter::All {
                                item = item.icon(IconName::Check);
                            }
                            item.on_click(move |_event, window, cx| {
                                if let Some(ref f) = handler {
                                    f(HistoryFilter::All, window, cx);
                                }
                            })
                        })
                        .item({
                            let handler = on_filter_starred.clone();
                            let mut item = PopupMenuItem::new("Starred");
                            if current_filter == HistoryFilter::Starred {
                                item = PopupMenuItem::new("Starred").icon(IconName::Check);
                            }
                            item.on_click(move |_event, window, cx| {
                                if let Some(ref f) = handler {
                                    f(HistoryFilter::Starred, window, cx);
                                }
                            })
                        })
                        .separator()
                        .label("Group by")
                        .item({
                            let handler = on_group_time.clone();
                            let mut item = PopupMenuItem::new("Time");
                            if current_group_by == HistoryGroupBy::Time {
                                item = item.icon(IconName::Check);
                            }
                            item.on_click(move |_event, window, cx| {
                                if let Some(ref f) = handler {
                                    f(HistoryGroupBy::Time, window, cx);
                                }
                            })
                        })
                        .item({
                            let handler = on_group_url.clone();
                            let mut item = PopupMenuItem::new("URL");
                            if current_group_by == HistoryGroupBy::Url {
                                item = item.icon(IconName::Check);
                            }
                            item.on_click(move |_event, window, cx| {
                                if let Some(ref f) = handler {
                                    f(HistoryGroupBy::Url, window, cx);
                                }
                            })
                        })
                })
        };

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
                                Icon::new(IconName::History)
                                    .size(px(16.0))
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_color(theme.foreground)
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .child("History"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .child(filter_button)
                            .child(clear_btn),
                    ),
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
                    .id("history-scroll-container")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(4.0))
                    .child(content),
            )
    }
}
