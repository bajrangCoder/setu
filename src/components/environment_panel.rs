use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, Render,
    SharedString, Styled, Window, div, px,
};
use gpui_component::ActiveTheme;
use gpui_component::Selectable;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::scroll::ScrollableElement;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;

use crate::completion::CompletionEngine;
use crate::entities::{
    CollectionsEntity, Environment, EnvironmentColor, EnvironmentEvent, EnvironmentScope,
    EnvironmentsEntity,
};
use crate::icons::IconName;

type NewEnvironmentCallback = Rc<dyn Fn(Option<Uuid>, &mut Window, &mut App) + 'static>;
type ImportEnvironmentCallback = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type DeleteEnvironmentCallback = Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>;
type RenameEnvironmentCallback = Rc<dyn Fn(Uuid, String, &mut Window, &mut App) + 'static>;
type OpenEnvironmentCallback = Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>;

pub struct EnvironmentPanel {
    environments: Entity<EnvironmentsEntity>,
    collections: Entity<CollectionsEntity>,
    collection_id: Option<Uuid>,
    selected_environment_id: Option<Uuid>,
    color_picker: Option<Entity<ColorPickerState>>,
    color_picker_environment_id: Option<Uuid>,
    color_picker_color: Option<EnvironmentColor>,
    completion_engine: CompletionEngine,
    on_new_environment: Option<NewEnvironmentCallback>,
    on_import_environment: Option<ImportEnvironmentCallback>,
    on_delete_environment: Option<DeleteEnvironmentCallback>,
    on_rename_environment: Option<RenameEnvironmentCallback>,
    on_open_environment: Option<OpenEnvironmentCallback>,
    focus_handle: FocusHandle,
}

impl EnvironmentPanel {
    pub fn new(
        environments: Entity<EnvironmentsEntity>,
        collections: Entity<CollectionsEntity>,
        cx: &mut Context<Self>,
    ) -> Self {
        let completion_engine = CompletionEngine::for_environments(environments.clone());
        cx.subscribe(
            &environments,
            |this, environments, event: &EnvironmentEvent, cx| {
                if matches!(event, EnvironmentEvent::ActiveChanged) {
                    this.selected_environment_id = environments
                        .read(cx)
                        .active_environment_id(this.collection_id);
                }
                cx.notify();
            },
        )
        .detach();
        cx.subscribe(&collections, |_this, _, _, cx| cx.notify())
            .detach();

        Self {
            environments,
            collections,
            collection_id: None,
            selected_environment_id: None,
            color_picker: None,
            color_picker_environment_id: None,
            color_picker_color: None,
            completion_engine,
            on_new_environment: None,
            on_import_environment: None,
            on_delete_environment: None,
            on_rename_environment: None,
            on_open_environment: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_collection_context(&mut self, collection_id: Option<Uuid>, cx: &mut Context<Self>) {
        if self.collection_id == collection_id {
            return;
        }
        self.collection_id = collection_id;
        self.selected_environment_id = self
            .environments
            .read(cx)
            .active_environment_id(collection_id);
        cx.notify();
    }

    pub fn on_new_environment(
        &mut self,
        callback: impl Fn(Option<Uuid>, &mut Window, &mut App) + 'static,
    ) {
        self.on_new_environment = Some(Rc::new(callback));
    }

    pub fn on_import_environment(&mut self, callback: impl Fn(&mut Window, &mut App) + 'static) {
        self.on_import_environment = Some(Rc::new(callback));
    }

    pub fn on_delete_environment(
        &mut self,
        callback: impl Fn(Uuid, &mut Window, &mut App) + 'static,
    ) {
        self.on_delete_environment = Some(Rc::new(callback));
    }

    pub fn on_rename_environment(
        &mut self,
        callback: impl Fn(Uuid, String, &mut Window, &mut App) + 'static,
    ) {
        self.on_rename_environment = Some(Rc::new(callback));
    }

    pub fn on_open_environment(
        &mut self,
        callback: impl Fn(Uuid, &mut Window, &mut App) + 'static,
    ) {
        self.on_open_environment = Some(Rc::new(callback));
    }

    pub fn select_environment(&mut self, environment_id: Uuid, cx: &mut Context<Self>) {
        if self.selected_environment_id != Some(environment_id) {
            self.selected_environment_id = Some(environment_id);
            cx.notify();
        }
    }

    fn ensure_selection(&mut self, cx: &mut Context<Self>) {
        let environments = self.environments.read(cx);
        if self
            .selected_environment_id
            .is_some_and(|id| environments.get(id).is_some())
        {
            return;
        }
        self.selected_environment_id = environments
            .active_environment_id(self.collection_id)
            .or_else(|| {
                environments
                    .environments()
                    .first()
                    .map(|environment| environment.id)
            });
    }

    fn sync_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_selection(cx);
        let selected_id = self.selected_environment_id;
        self.sync_color_picker(selected_id, window, cx);
    }

    fn ensure_color_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.color_picker.is_some() {
            return;
        }
        let picker = cx.new(|cx| {
            ColorPickerState::new(window, cx).default_value(EnvironmentColor::default().accent())
        });
        cx.subscribe(&picker, |this, _, event: &ColorPickerEvent, cx| {
            let ColorPickerEvent::Change(Some(color)) = event else {
                return;
            };
            let Some(environment_id) = this.selected_environment_id else {
                return;
            };
            this.environments.update(cx, |environments, cx| {
                environments.set_environment_color(
                    environment_id,
                    EnvironmentColor::custom(*color),
                    cx,
                );
            });
        })
        .detach();
        self.color_picker = Some(picker);
    }

    fn sync_color_picker(
        &mut self,
        environment_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected_color = environment_id
            .and_then(|id| self.environments.read(cx).get(id))
            .map(|environment| environment.color.clone());
        if self.color_picker_environment_id == environment_id
            && self.color_picker_color == selected_color
        {
            return;
        }
        let Some(selected_color) = selected_color else {
            self.color_picker_environment_id = environment_id;
            self.color_picker_color = None;
            return;
        };
        if let Some(picker) = &self.color_picker {
            let color = selected_color.accent();
            picker.update(cx, |picker, cx| picker.set_value(color, window, cx));
        }
        self.color_picker_environment_id = environment_id;
        self.color_picker_color = Some(selected_color);
    }

    fn render_environment_menu(
        &self,
        environment: &Environment,
        selected: bool,
        this: Entity<Self>,
    ) -> impl IntoElement {
        let environment_id = environment.id;
        let environment_scope = environment.scope;
        let environment_name = environment.name.clone();
        let environment_color = environment.color.clone();
        let environments_for_activate = self.environments.clone();
        let environments_for_duplicate = self.environments.clone();
        let environments_for_color = self.environments.clone();
        let on_rename = self.on_rename_environment.clone();
        let on_delete = self.on_delete_environment.clone();

        Button::new(SharedString::from(format!(
            "environment-more-{environment_id}"
        )))
        .icon(IconName::Ellipsis)
        .ghost()
        .xsmall()
        .selected(selected)
        .tooltip("Environment actions")
        .dropdown_menu(move |mut menu, _window, _cx| {
            let activate_label = match environment_scope {
                EnvironmentScope::Global => "Use globally",
                EnvironmentScope::Workspace => "Use as workspace base",
                EnvironmentScope::Project(_) => "Use for this project",
            };
            let environments = environments_for_activate.clone();
            menu = menu.item(
                PopupMenuItem::new(activate_label)
                    .icon(IconName::CircleCheck)
                    .on_click(move |_, _, cx| {
                        environments.update(cx, |environments, cx| match environment_scope {
                            EnvironmentScope::Global => {
                                environments.set_active(None, Some(environment_id), cx);
                            }
                            EnvironmentScope::Workspace => {
                                environments.set_active(None, Some(environment_id), cx);
                            }
                            EnvironmentScope::Project(project_id) => {
                                environments.set_active(Some(project_id), Some(environment_id), cx);
                            }
                        });
                    }),
            );

            let environments = environments_for_duplicate.clone();
            let this_for_duplicate = this.clone();
            menu = menu.item(
                PopupMenuItem::new("Duplicate")
                    .icon(IconName::CopyPlus)
                    .on_click(move |_, _, cx| {
                        if let Some(id) = environments.update(cx, |environments, cx| {
                            environments.duplicate_environment(environment_id, cx)
                        }) {
                            this_for_duplicate.update(cx, |panel, cx| {
                                panel.select_environment(id, cx);
                            });
                        }
                    }),
            );

            if let Some(callback) = on_rename.clone() {
                let name = environment_name.clone();
                menu = menu.item(
                    PopupMenuItem::new("Rename")
                        .icon(IconName::FilePen)
                        .on_click(move |_, window, cx| {
                            callback(environment_id, name.clone(), window, cx);
                        }),
                );
            }

            menu = menu.separator().label("Color");
            for color in EnvironmentColor::ALL {
                let environments = environments_for_color.clone();
                let mut item = PopupMenuItem::new(color.label());
                if color == environment_color {
                    item = item.icon(IconName::Check);
                }
                menu = menu.item(item.on_click(move |_, _, cx| {
                    let color = color.clone();
                    environments.update(cx, |environments, cx| {
                        environments.set_environment_color(environment_id, color, cx);
                    });
                }));
            }

            if let Some(callback) = on_delete.clone() {
                menu = menu.separator().item(
                    PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
                        move |_, window, cx| {
                            callback(environment_id, window, cx);
                        },
                    ),
                );
            }
            menu
        })
    }

    fn render_environment_row(
        &self,
        environment: &Environment,
        this: Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        let theme = cx.theme();
        let id = environment.id;
        let selected = self.selected_environment_id == Some(id);
        let active = self.environments.read(cx).is_active(id);
        let color = environment.color.accent();
        let this_for_select = this.clone();
        let on_open = self.on_open_environment.clone();

        div()
            .id(SharedString::from(format!("environment-row-{id}")))
            .h(px(32.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .rounded(px(5.0))
            .cursor_pointer()
            .when(selected, |element| element.bg(theme.sidebar_accent))
            .when(!selected, |element| {
                element.hover(|element| element.bg(theme.sidebar_accent.opacity(0.55)))
            })
            .on_click(move |_, window, cx| {
                this_for_select.update(cx, |panel, cx| {
                    panel.select_environment(id, cx);
                });
                if let Some(ref cb) = on_open {
                    cb(id, window, cx);
                }
            })
            .child(
                div()
                    .size(if active { px(8.0) } else { px(7.0) })
                    .rounded_full()
                    .bg(color)
                    .when(active, |element| {
                        element
                            .border_1()
                            .border_color(theme.foreground.opacity(0.6))
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_xs()
                    .truncate()
                    .text_color(if selected {
                        theme.foreground
                    } else {
                        theme.secondary_foreground
                    })
                    .child(environment.name.clone()),
            )
            .child(self.render_environment_menu(environment, selected, this))
            .into_any_element()
    }

    fn render_active_stack(&self, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let (global, workspace, project) = {
            let environments = self.environments.read(cx);
            let global = environments
                .active_global_environment_id()
                .and_then(|id| environments.get(id))
                .cloned();
            let workspace = environments
                .active_workspace_environment_id()
                .and_then(|id| environments.get(id))
                .cloned();
            let project = self
                .collection_id
                .and_then(|id| environments.active_project_environment_id(id))
                .and_then(|id| environments.get(id))
                .cloned();
            (global, workspace, project)
        };

        let mut active_items = Vec::new();
        if let Some(env) = global {
            active_items.push(env);
        }
        if let Some(env) = workspace {
            active_items.push(env);
        }
        if let Some(env) = project {
            active_items.push(env);
        }

        if active_items.is_empty() {
            return div().into_any_element();
        }

        let mut stack_children = Vec::new();
        for (i, env) in active_items.into_iter().enumerate() {
            if i > 0 {
                stack_children.push(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme.muted_foreground)
                        .child("→")
                        .into_any_element(),
                );
            }
            stack_children.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .min_w_0()
                    .child(div().size(px(6.0)).rounded_full().bg(env.color.accent()))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(10.0))
                            .text_color(theme.secondary_foreground)
                            .child(env.name),
                    )
                    .into_any_element(),
            );
        }

        div()
            .px(px(10.0))
            .py(px(4.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child("Active:"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .children(stack_children),
            )
            .into_any_element()
    }

    fn has_layered_precedence(&self, cx: &App) -> bool {
        let environments = self.environments.read(cx);
        let active_layers = usize::from(environments.active_global_environment_id().is_some())
            + usize::from(environments.active_workspace_environment_id().is_some())
            + usize::from(
                self.collection_id
                    .and_then(|id| environments.active_project_environment_id(id))
                    .is_some(),
            );
        active_layers > 1
    }

    fn render_empty_state(&self, theme: &gpui_component::theme::ThemeColor) -> AnyElement {
        let on_new = self.on_new_environment.clone();
        let collection_id = self.collection_id;
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px(px(28.0))
            .gap(px(9.0))
            .child(
                div()
                    .size(px(36.0))
                    .rounded(px(9.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.primary.opacity(0.11))
                    .child(
                        gpui_component::Icon::new(IconName::Package)
                            .size(px(17.0))
                            .text_color(theme.primary),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Create your first environment"),
            )
            .child(
                div()
                    .text_xs()
                    .text_center()
                    .text_color(theme.muted_foreground)
                    .child("Keep base URLs and credentials reusable across requests."),
            )
            .child(
                Button::new("environment-empty-new")
                    .label("New environment")
                    .icon(IconName::Plus)
                    .primary()
                    .small()
                    .on_click(move |_, window, cx| {
                        if let Some(ref callback) = on_new {
                            callback(collection_id, window, cx);
                        }
                    }),
            )
            .into_any_element()
    }
}

impl Focusable for EnvironmentPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EnvironmentPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.completion_engine.set_collection_id(self.collection_id);
        self.ensure_color_picker(window, cx);
        self.sync_rows(window, cx);
        let theme = cx.theme();
        let this = cx.entity().clone();
        let environments = self.environments.read(cx).environments().to_vec();
        let _selected = self
            .selected_environment_id
            .and_then(|id| environments.iter().find(|environment| environment.id == id))
            .cloned();
        let on_new = self.on_new_environment.clone();
        let on_import = self.on_import_environment.clone();
        let collection_id = self.collection_id;
        let has_layered_precedence = self.has_layered_precedence(cx);
        let _color_picker = self.color_picker.clone();

        let mut project_groups: HashMap<Uuid, Vec<Environment>> = HashMap::new();
        let mut global_environments = Vec::new();
        let mut workspace_environments = Vec::new();
        for environment in &environments {
            match environment.scope {
                EnvironmentScope::Global => global_environments.push(environment.clone()),
                EnvironmentScope::Workspace => workspace_environments.push(environment.clone()),
                EnvironmentScope::Project(project_id) => project_groups
                    .entry(project_id)
                    .or_default()
                    .push(environment.clone()),
            }
        }

        let mut navigator_groups = Vec::new();
        if !global_environments.is_empty() {
            let rows = global_environments
                .iter()
                .map(|environment| self.render_environment_row(environment, this.clone(), cx))
                .collect::<Vec<_>>();
            navigator_groups.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .px(px(8.0))
                            .pt(px(5.0))
                            .pb(px(3.0))
                            .text_size(px(9.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child("GLOBAL"),
                    )
                    .children(rows)
                    .into_any_element(),
            );
        }
        if !workspace_environments.is_empty() {
            let rows = workspace_environments
                .iter()
                .map(|environment| self.render_environment_row(environment, this.clone(), cx))
                .collect::<Vec<_>>();
            navigator_groups.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .px(px(8.0))
                            .pt(px(5.0))
                            .pb(px(3.0))
                            .text_size(px(9.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child("WORKSPACE"),
                    )
                    .children(rows)
                    .into_any_element(),
            );
        }

        let collections = self.collections.read(cx).collections.clone();
        for collection in collections {
            let Some(group) = project_groups.remove(&collection.id) else {
                continue;
            };
            let rows = group
                .iter()
                .map(|environment| self.render_environment_row(environment, this.clone(), cx))
                .collect::<Vec<_>>();
            navigator_groups.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .px(px(8.0))
                            .pt(px(8.0))
                            .pb(px(3.0))
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .text_size(px(9.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child(gpui_component::Icon::new(IconName::Folder).size(px(10.0)))
                            .child(collection.name.to_uppercase()),
                    )
                    .children(rows)
                    .into_any_element(),
            );
        }

        for (_project_id, group) in project_groups {
            let rows = group
                .iter()
                .map(|environment| self.render_environment_row(environment, this.clone(), cx))
                .collect::<Vec<_>>();
            navigator_groups.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .px(px(8.0))
                            .pt(px(8.0))
                            .pb(px(3.0))
                            .text_size(px(9.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child("UNLINKED PROJECT"),
                    )
                    .children(rows)
                    .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .min_h_0()
            .bg(theme.sidebar)
            .child(
                div()
                    .h(px(52.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .size(px(28.0))
                                    .rounded(px(7.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme.primary.opacity(0.11))
                                    .child(
                                        gpui_component::Icon::new(IconName::Package)
                                            .size(px(14.0))
                                            .text_color(theme.primary),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Environments"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.0))
                                            .text_color(theme.muted_foreground)
                                            .child("Workspace and project values"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .child(
                                Button::new("environment-import")
                                    .icon(IconName::FileUp)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Import Postman data")
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref callback) = on_import {
                                            callback(window, cx);
                                        }
                                    }),
                            )
                            .child(
                                Button::new("environment-new")
                                    .icon(IconName::Plus)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("New environment")
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref callback) = on_new {
                                            callback(collection_id, window, cx);
                                        }
                                    }),
                            ),
                    ),
            )
            .when(!environments.is_empty(), |element| {
                element
                    .when(has_layered_precedence, |element| {
                        element.child(self.render_active_stack(cx))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .px(px(7.0))
                            .pt(px(4.0))
                            .pb(px(6.0))
                            .flex()
                            .flex_col()
                            .gap(px(3.0))
                            .children(navigator_groups),
                    )
            })
            .when(environments.is_empty(), |element| {
                element.child(self.render_empty_state(&theme))
            })
    }
}
