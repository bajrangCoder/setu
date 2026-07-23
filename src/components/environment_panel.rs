use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, Render,
    SharedString, Styled, Window, div, px,
};
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{ActiveTheme, Selectable};
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;

use crate::entities::{
    CollectionsEntity, Environment, EnvironmentColor, EnvironmentEvent, EnvironmentScope,
    EnvironmentVariable, EnvironmentsEntity,
};
use crate::icons::IconName;

type NewEnvironmentCallback = Rc<dyn Fn(Option<Uuid>, &mut Window, &mut App) + 'static>;
type ImportEnvironmentCallback = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type DeleteEnvironmentCallback = Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>;
type RenameEnvironmentCallback = Rc<dyn Fn(Uuid, String, &mut Window, &mut App) + 'static>;

struct VariableRow {
    id: Uuid,
    key_input: Entity<InputState>,
    value_input: Entity<InputState>,
    enabled: bool,
    secret: bool,
}

pub struct EnvironmentPanel {
    environments: Entity<EnvironmentsEntity>,
    collections: Entity<CollectionsEntity>,
    collection_id: Option<Uuid>,
    selected_environment_id: Option<Uuid>,
    loaded_environment_id: Option<Uuid>,
    loaded_signature: Vec<(Uuid, bool)>,
    rows: Vec<VariableRow>,
    on_new_environment: Option<NewEnvironmentCallback>,
    on_import_environment: Option<ImportEnvironmentCallback>,
    on_delete_environment: Option<DeleteEnvironmentCallback>,
    on_rename_environment: Option<RenameEnvironmentCallback>,
    focus_handle: FocusHandle,
}

impl EnvironmentPanel {
    pub fn new(
        environments: Entity<EnvironmentsEntity>,
        collections: Entity<CollectionsEntity>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(
            &environments,
            |this, environments, event: &EnvironmentEvent, cx| {
                if matches!(event, EnvironmentEvent::ActiveChanged) {
                    this.selected_environment_id = environments
                        .read(cx)
                        .active_environment_id(this.collection_id);
                    this.loaded_environment_id = None;
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
            loaded_environment_id: None,
            loaded_signature: Vec::new(),
            rows: Vec::new(),
            on_new_environment: None,
            on_import_environment: None,
            on_delete_environment: None,
            on_rename_environment: None,
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
        self.loaded_environment_id = None;
        self.loaded_signature.clear();
        self.rows.clear();
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

    pub fn select_environment(&mut self, environment_id: Uuid, cx: &mut Context<Self>) {
        if self.selected_environment_id != Some(environment_id) {
            self.selected_environment_id = Some(environment_id);
            self.loaded_environment_id = None;
            self.loaded_signature.clear();
            self.rows.clear();
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
        let variables = selected_id
            .and_then(|id| self.environments.read(cx).get(id))
            .map(|environment| environment.variables.clone())
            .unwrap_or_default();
        let signature: Vec<_> = variables
            .iter()
            .map(|variable| (variable.id, variable.secret))
            .collect();

        if self.loaded_environment_id == selected_id && self.loaded_signature == signature {
            for (row, variable) in self.rows.iter_mut().zip(variables) {
                row.enabled = variable.enabled;
                row.secret = variable.secret;
            }
            return;
        }

        self.rows.clear();
        self.loaded_environment_id = selected_id;
        self.loaded_signature = signature;
        let Some(environment_id) = selected_id else {
            return;
        };
        for variable in variables {
            self.rows
                .push(self.build_row(environment_id, variable, window, cx));
        }
    }

    fn build_row(
        &self,
        environment_id: Uuid,
        variable: EnvironmentVariable,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> VariableRow {
        let variable_id = variable.id;
        let key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("VARIABLE_NAME")
                .default_value(&variable.key)
        });
        let value_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(if variable.secret {
                    "Secret value"
                } else {
                    "Value"
                })
                .default_value(&variable.value)
                .masked(variable.secret)
        });

        let environments_for_key = self.environments.clone();
        cx.subscribe(&key_input, move |_, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                environments_for_key.update(cx, |environments, cx| {
                    environments.update_variable(
                        environment_id,
                        variable_id,
                        Some(input.read(cx).text().to_string()),
                        None,
                        cx,
                    );
                });
            }
        })
        .detach();

        let environments_for_value = self.environments.clone();
        cx.subscribe(&value_input, move |_, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                environments_for_value.update(cx, |environments, cx| {
                    environments.update_variable(
                        environment_id,
                        variable_id,
                        None,
                        Some(input.read(cx).text().to_string()),
                        cx,
                    );
                });
            }
        })
        .detach();

        VariableRow {
            id: variable.id,
            key_input,
            value_input,
            enabled: variable.enabled,
            secret: variable.secret,
        }
    }

    fn collection_name(&self, id: Uuid, cx: &App) -> Option<String> {
        self.collections
            .read(cx)
            .collections
            .iter()
            .find(|collection| collection.id == id)
            .map(|collection| collection.name.clone())
    }

    fn scope_label(&self, scope: EnvironmentScope, cx: &App) -> String {
        match scope {
            EnvironmentScope::Workspace => "Workspace base".to_string(),
            EnvironmentScope::Project(collection_id) => self
                .collection_name(collection_id, cx)
                .map(|name| format!("Project · {name}"))
                .unwrap_or_else(|| "Unlinked project".to_string()),
        }
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
        let environment_color = environment.color;
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
                EnvironmentScope::Workspace => "Use as workspace base",
                EnvironmentScope::Project(_) => "Use for this project",
            };
            let environments = environments_for_activate.clone();
            menu = menu.item(
                PopupMenuItem::new(activate_label)
                    .icon(IconName::CircleCheck)
                    .on_click(move |_, _, cx| {
                        environments.update(cx, |environments, cx| match environment_scope {
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
        let variable_count = environment.variables.len();
        let this_for_select = this.clone();

        div()
            .id(SharedString::from(format!("environment-row-{id}")))
            .h(px(40.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .when(selected, |element| {
                element
                    .bg(theme.sidebar_accent)
                    .border_1()
                    .border_color(theme.border.opacity(0.9))
            })
            .when(!selected, |element| {
                element.hover(|element| element.bg(theme.sidebar_accent.opacity(0.55)))
            })
            .on_click(move |_, _, cx| {
                this_for_select.update(cx, |panel, cx| {
                    panel.select_environment(id, cx);
                });
            })
            .child(
                div()
                    .size(px(9.0))
                    .rounded_full()
                    .bg(color)
                    .when(active, |element| {
                        element.border_2().border_color(theme.sidebar).shadow_sm()
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .min_w_0()
                            .text_sm()
                            .truncate()
                            .text_color(if selected {
                                theme.foreground
                            } else {
                                theme.secondary_foreground
                            })
                            .child(environment.name.clone()),
                    )
                    .when(active, |element| {
                        element.child(
                            div()
                                .px(px(5.0))
                                .py(px(1.0))
                                .rounded(px(3.0))
                                .bg(color.opacity(0.13))
                                .text_size(px(9.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(color)
                                .child("ACTIVE"),
                        )
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(variable_count.to_string()),
            )
            .child(self.render_environment_menu(environment, selected, this))
            .into_any_element()
    }

    fn render_active_stack(&self, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let (workspace, project) = {
            let environments = self.environments.read(cx);
            let workspace = environments
                .active_workspace_environment_id()
                .and_then(|id| environments.get(id))
                .cloned();
            let project = self
                .collection_id
                .and_then(|id| environments.active_project_environment_id(id))
                .and_then(|id| environments.get(id))
                .cloned();
            (workspace, project)
        };

        let layer = |environment: &Environment, suffix: &str| {
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .min_w_0()
                .child(
                    div()
                        .size(px(7.0))
                        .rounded_full()
                        .bg(environment.color.accent()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(theme.secondary_foreground)
                        .child(environment.name.clone()),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(9.0))
                        .text_color(theme.muted_foreground)
                        .child(suffix.to_string()),
                )
        };

        div()
            .mx(px(10.0))
            .mt(px(8.0))
            .mb(px(6.0))
            .p(px(9.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.background.opacity(0.42))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(9.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child("ACTIVE VARIABLE STACK"),
            )
            .when_some(workspace.as_ref(), |element, environment| {
                element.child(layer(environment, "workspace"))
            })
            .when_some(project.as_ref(), |element, environment| {
                element
                    .child(div().ml(px(2.0)).h(px(7.0)).w(px(1.0)).bg(theme.border))
                    .child(layer(environment, "overrides"))
            })
            .when(workspace.is_none() && project.is_none(), |element| {
                element.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("No environment is active"),
                )
            })
            .into_any_element()
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
        self.sync_rows(window, cx);
        let theme = cx.theme();
        let this = cx.entity().clone();
        let environments = self.environments.read(cx).environments().to_vec();
        let selected = self
            .selected_environment_id
            .and_then(|id| environments.iter().find(|environment| environment.id == id))
            .cloned();
        let on_new = self.on_new_environment.clone();
        let on_import = self.on_import_environment.clone();
        let collection_id = self.collection_id;

        let mut project_groups: HashMap<Uuid, Vec<Environment>> = HashMap::new();
        let mut workspace_environments = Vec::new();
        for environment in &environments {
            match environment.scope {
                EnvironmentScope::Workspace => workspace_environments.push(environment.clone()),
                EnvironmentScope::Project(project_id) => project_groups
                    .entry(project_id)
                    .or_default()
                    .push(environment.clone()),
            }
        }

        let mut navigator_groups = Vec::new();
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

        let editor = if let Some(environment) = selected {
            let environment_id = environment.id;
            let color = environment.color.accent();
            let scope_label = self.scope_label(environment.scope, cx);
            let mut key_counts = HashMap::new();
            for variable in &environment.variables {
                let key = variable.key.trim();
                if !key.is_empty() {
                    *key_counts.entry(key.to_ascii_lowercase()).or_insert(0usize) += 1;
                }
            }
            let duplicate_key_count = key_counts.values().filter(|count| **count > 1).count();
            let environments_for_add = self.environments.clone();
            let environments_for_header_add = self.environments.clone();
            let rows = self
                .rows
                .iter()
                .map(|row| {
                    let variable_id = row.id;
                    let environments_for_toggle = self.environments.clone();
                    let environments_for_secret = self.environments.clone();
                    let environments_for_duplicate = self.environments.clone();
                    let environments_for_remove = self.environments.clone();
                    let secret = row.secret;

                    div()
                        .id(SharedString::from(format!(
                            "environment-variable-{variable_id}"
                        )))
                        .px(px(10.0))
                        .py(px(8.0))
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .border_b_1()
                        .border_color(theme.border.opacity(0.72))
                        .when(!row.enabled, |element| element.opacity(0.5))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .child(
                                    Checkbox::new(SharedString::from(format!(
                                        "environment-enabled-{variable_id}"
                                    )))
                                    .checked(row.enabled)
                                    .on_click(
                                        move |_, _, cx| {
                                            environments_for_toggle.update(
                                                cx,
                                                |environments, cx| {
                                                    environments.toggle_variable(
                                                        environment_id,
                                                        variable_id,
                                                        cx,
                                                    );
                                                },
                                            );
                                        },
                                    ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .h(px(29.0))
                                        .rounded(px(5.0))
                                        .bg(theme.muted.opacity(0.7))
                                        .px(px(7.0))
                                        .child(
                                            Input::new(&row.key_input).appearance(false).xsmall(),
                                        ),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "environment-variable-more-{variable_id}"
                                    )))
                                    .icon(IconName::Ellipsis)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Variable actions")
                                    .dropdown_menu(
                                        move |menu, _window, _cx| {
                                            let environments = environments_for_secret.clone();
                                            let secret_item = PopupMenuItem::new(if secret {
                                                "Make regular value"
                                            } else {
                                                "Mark as secret"
                                            })
                                            .icon(if secret {
                                                IconName::Unlock
                                            } else {
                                                IconName::Lock
                                            })
                                            .on_click(move |_, _, cx| {
                                                environments.update(cx, |environments, cx| {
                                                    environments.toggle_secret(
                                                        environment_id,
                                                        variable_id,
                                                        cx,
                                                    );
                                                });
                                            });
                                            let environments = environments_for_duplicate.clone();
                                            let duplicate_item = PopupMenuItem::new("Duplicate")
                                                .icon(IconName::CopyPlus)
                                                .on_click(move |_, _, cx| {
                                                    environments.update(cx, |environments, cx| {
                                                        environments.duplicate_variable(
                                                            environment_id,
                                                            variable_id,
                                                            cx,
                                                        );
                                                    });
                                                });
                                            let environments = environments_for_remove.clone();
                                            let delete_item = PopupMenuItem::new("Delete")
                                                .icon(IconName::Trash)
                                                .on_click(move |_, _, cx| {
                                                    environments.update(cx, |environments, cx| {
                                                        environments.remove_variable(
                                                            environment_id,
                                                            variable_id,
                                                            cx,
                                                        );
                                                    });
                                                });
                                            menu.item(secret_item)
                                                .item(duplicate_item)
                                                .separator()
                                                .item(delete_item)
                                        },
                                    ),
                                ),
                        )
                        .child(
                            div()
                                .ml(px(27.0))
                                .h(px(30.0))
                                .rounded(px(5.0))
                                .border_1()
                                .border_color(theme.input)
                                .bg(theme.background.opacity(0.55))
                                .px(px(7.0))
                                .child(
                                    Input::new(&row.value_input)
                                        .appearance(false)
                                        .xsmall()
                                        .when(row.secret, |input| input.mask_toggle()),
                                ),
                        )
                        .when(row.secret, |element| {
                            element.child(
                                div()
                                    .ml(px(29.0))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .text_size(px(9.0))
                                    .text_color(theme.muted_foreground)
                                    .child(gpui_component::Icon::new(IconName::Lock).size(px(9.0)))
                                    .child("Local secret · excluded from history"),
                            )
                        })
                })
                .collect::<Vec<_>>();

            div()
                .h(px(0.0))
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(theme.border)
                .child(
                    div()
                        .px(px(11.0))
                        .py(px(9.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .bg(theme.background.opacity(0.35))
                        .child(div().w(px(3.0)).h(px(30.0)).rounded_full().bg(color))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .truncate()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(environment.name),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(10.0))
                                        .text_color(theme.muted_foreground)
                                        .child(scope_label),
                                ),
                        )
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(theme.muted)
                                .text_size(px(9.0))
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "{} VAR{}",
                                    self.rows.len(),
                                    if self.rows.len() == 1 { "" } else { "S" }
                                )),
                        )
                        .when(duplicate_key_count > 0, |element| {
                            element.child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(theme.warning.opacity(0.12))
                                    .text_size(px(9.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.warning)
                                    .child("DUPLICATE KEYS"),
                            )
                        })
                        .child(
                            Button::new("environment-add-variable-header")
                                .icon(IconName::Plus)
                                .label("Add")
                                .ghost()
                                .xsmall()
                                .tooltip("Add a variable")
                                .on_click(move |_, _, cx| {
                                    environments_for_header_add.update(cx, |environments, cx| {
                                        environments.add_variable(environment_id, cx);
                                    });
                                }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .children(rows)
                        .when(self.rows.is_empty(), |element| {
                            element.child(
                                div()
                                    .h(px(110.0))
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(5.0))
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        gpui_component::Icon::new(IconName::Variable)
                                            .size(px(16.0)),
                                    )
                                    .child(
                                        div().text_xs().child("No variables in this environment"),
                                    ),
                            )
                        }),
                )
                .child(
                    div()
                        .p(px(9.0))
                        .border_t_1()
                        .border_color(theme.border)
                        .child(
                            Button::new("environment-add-variable")
                                .w_full()
                                .justify_center()
                                .label("New variable")
                                .icon(IconName::Plus)
                                .outline()
                                .small()
                                .on_click(move |_, _, cx| {
                                    environments_for_add.update(cx, |environments, cx| {
                                        environments.add_variable(environment_id, cx);
                                    });
                                }),
                        ),
                )
                .into_any_element()
        } else {
            self.render_empty_state(&theme)
        };

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
                                    .tooltip("Import a Postman environment")
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
                element.child(self.render_active_stack(cx)).child(
                    div()
                        // The scrollbar wrapper only inherits concrete sizes from its
                        // child, so this must be an explicit height rather than max_h.
                        .h(px(190.0))
                        .flex_shrink_0()
                        .overflow_y_scrollbar()
                        .px(px(7.0))
                        .pb(px(8.0))
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .children(navigator_groups),
                )
            })
            .child(
                div()
                    .h(px(0.0))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(editor),
            )
    }
}
