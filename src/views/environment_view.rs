use gpui::prelude::*;
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, Render, SharedString,
    Styled, Window, div, px,
};
use gpui_component::ActiveTheme;
use gpui_component::Icon;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use std::collections::HashMap;
use uuid::Uuid;

use crate::completion::{
    CompletionContext, CompletionEngine, CompletionInput, configure_completion,
};
use crate::entities::{
    CollectionsEntity, EnvironmentColor, EnvironmentEvent, EnvironmentScope, EnvironmentVariable,
    EnvironmentsEntity,
};
use crate::icons::IconName;

struct VariableRow {
    id: Uuid,
    key_input: Entity<InputState>,
    value_input: Entity<InputState>,
    enabled: bool,
    secret: bool,
}

pub struct EnvironmentView {
    environments: Entity<EnvironmentsEntity>,
    collections: Entity<CollectionsEntity>,
    collection_id: Option<Uuid>,
    environment_id: Uuid,
    search_input: Entity<InputState>,
    loaded_signature: Vec<(Uuid, bool)>,
    rows: Vec<VariableRow>,
    color_picker: Option<Entity<ColorPickerState>>,
    color_picker_environment_id: Option<Uuid>,
    color_picker_color: Option<EnvironmentColor>,
    completion_engine: CompletionEngine,
    focus_handle: FocusHandle,
}

impl EnvironmentView {
    pub fn new(
        environment_id: Uuid,
        environments: Entity<EnvironmentsEntity>,
        collections: Entity<CollectionsEntity>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let completion_engine = CompletionEngine::for_environments(environments.clone());
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter variables..."));

        cx.subscribe(
            &environments,
            |_this, _environments, _event: &EnvironmentEvent, cx| {
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
            environment_id,
            search_input,
            loaded_signature: Vec::new(),
            rows: Vec::new(),
            color_picker: None,
            color_picker_environment_id: None,
            color_picker_color: None,
            completion_engine,
            focus_handle: cx.focus_handle(),
        }
    }

    #[allow(dead_code)]
    pub fn environment_id(&self) -> Uuid {
        self.environment_id
    }

    #[allow(dead_code)]
    pub fn set_collection_context(&mut self, collection_id: Option<Uuid>) {
        self.collection_id = collection_id;
    }

    fn sync_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let environment_id = self.environment_id;
        self.sync_color_picker(Some(environment_id), window, cx);
        let variables = self
            .environments
            .read(cx)
            .get(environment_id)
            .map(|environment| environment.variables.clone())
            .unwrap_or_default();
        let signature: Vec<_> = variables
            .iter()
            .map(|variable| (variable.id, variable.secret))
            .collect();

        if self.loaded_signature == signature {
            for (row, variable) in self.rows.iter_mut().zip(variables) {
                row.enabled = variable.enabled;
                row.secret = variable.secret;
            }
            return;
        }

        self.rows.clear();
        self.loaded_signature = signature;
        for variable in variables {
            self.rows
                .push(self.build_row(environment_id, variable, window, cx));
        }
    }

    fn ensure_color_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.color_picker.is_some() {
            return;
        }
        let picker = cx.new(|cx| {
            ColorPickerState::new(window, cx).default_value(EnvironmentColor::default().accent())
        });
        let environment_id = self.environment_id;
        cx.subscribe(&picker, move |this, _, event: &ColorPickerEvent, cx| {
            let ColorPickerEvent::Change(Some(color)) = event else {
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
                .placeholder("Key")
                .default_value(&variable.key)
        });
        let value_input = cx.new(|cx| {
            configure_completion(
                InputState::new(window, cx)
                    .placeholder(if variable.secret {
                        "Secret value"
                    } else {
                        "Value"
                    })
                    .default_value(&variable.value)
                    .masked(variable.secret),
                Some(&self.completion_engine),
                CompletionContext::EnvironmentValue,
            )
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
            EnvironmentScope::Global => "Global · All workspaces".to_string(),
            EnvironmentScope::Workspace => "Workspace base".to_string(),
            EnvironmentScope::Project(collection_id) => self
                .collection_name(collection_id, cx)
                .map(|name| format!("Project · {name}"))
                .unwrap_or_else(|| "Unlinked project".to_string()),
        }
    }

    fn clear_all_variables(&mut self, cx: &mut Context<Self>) {
        let environment_id = self.environment_id;
        self.environments.update(cx, |environments, cx| {
            environments.clear_variables(environment_id, cx);
        });
    }
}

impl Focusable for EnvironmentView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EnvironmentView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.completion_engine.set_collection_id(self.collection_id);
        self.ensure_color_picker(window, cx);
        self.sync_rows(window, cx);
        let theme = cx.theme();
        let environment_id = self.environment_id;

        let environment = self.environments.read(cx).get(environment_id).cloned();

        let Some(environment) = environment else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child("Environment not found")
                .into_any_element();
        };

        let active = self.environments.read(cx).is_active(environment_id);
        let color = environment.color.accent();
        let scope_label = self.scope_label(environment.scope, cx);
        let var_count = environment.variables.len();
        let color_picker = self.color_picker.clone();
        let mut key_counts = HashMap::new();
        for variable in &environment.variables {
            let key = variable.key.trim();
            if !key.is_empty() {
                *key_counts.entry(key.to_ascii_lowercase()).or_insert(0usize) += 1;
            }
        }

        // Single unified Header bar
        let environments_for_menu = self.environments.clone();
        let environments_for_activate = self.environments.clone();
        let collection_id = self.collection_id;
        let this = cx.entity().clone();

        let header = div()
            .h(px(40.0))
            .px(px(16.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    // Color Dot
                    .child(div().size(px(10.0)).rounded_full().bg(color))
                    // Environment Title
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(environment.name.clone()),
                    )
                    // Bullet separator
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground.opacity(0.4))
                            .child("•"),
                    )
                    // Scope subtitle
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(scope_label),
                    )
                    // Minimal Active Badge
                    .when(active, |element| {
                        element.child(
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded(px(4.0))
                                .bg(color.opacity(0.15))
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(color)
                                .child("Active"),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    // Variable Count
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "{var_count} variable{}",
                                if var_count == 1 { "" } else { "s" }
                            )),
                    )
                    // Search box with search icon
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(8.0))
                            .py(px(3.0))
                            .bg(theme.muted)
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(theme.border.opacity(0.5))
                            .child(
                                Icon::new(IconName::Search)
                                    .size(px(13.0))
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div().w(px(160.0)).child(
                                    Input::new(&self.search_input).appearance(false).small(),
                                ),
                            ),
                    )
                    // Color picker
                    .when_some(color_picker, |element, picker| {
                        element.child(
                            ColorPicker::new(&picker)
                                .featured_colors(
                                    EnvironmentColor::ALL
                                        .iter()
                                        .map(EnvironmentColor::accent)
                                        .collect(),
                                )
                                .small(),
                        )
                    })
                    // Clear All Trash Button
                    .child(
                        Button::new("clear-all-env-vars-btn")
                            .icon(IconName::Trash)
                            .ghost()
                            .small()
                            .tooltip("Clear All")
                            .on_click({
                                let this = this.clone();
                                move |_, _, cx| {
                                    this.update(cx, |view, cx| {
                                        view.clear_all_variables(cx);
                                    });
                                }
                            }),
                    )
                    // Add New Variable Button
                    .child(
                        Button::new("add-env-var-btn")
                            .icon(IconName::Plus)
                            .ghost()
                            .small()
                            .tooltip("Add Variable")
                            .on_click({
                                let envs = self.environments.clone();
                                move |_, _, cx| {
                                    envs.update(cx, |environments, cx| {
                                        environments.add_variable(environment_id, cx);
                                    });
                                }
                            }),
                    )
                    // Dropdown menu
                    .child(
                        Button::new(SharedString::from(format!("env-actions-{environment_id}")))
                            .icon(IconName::Ellipsis)
                            .ghost()
                            .small()
                            .tooltip("Actions")
                            .dropdown_menu(move |mut menu, _window, _cx| {
                                let envs = environments_for_menu.clone();
                                let envs_act = environments_for_activate.clone();
                                if !active {
                                    menu = menu.item(
                                        PopupMenuItem::new("Set Active")
                                            .icon(IconName::Check)
                                            .on_click(move |_, _, cx| {
                                                envs_act.update(cx, |e, cx| {
                                                    e.set_active(
                                                        collection_id,
                                                        Some(environment_id),
                                                        cx,
                                                    );
                                                });
                                            }),
                                    );
                                }
                                menu = menu.item(
                                    PopupMenuItem::new("Duplicate")
                                        .icon(IconName::CopyPlus)
                                        .on_click(move |_, _, cx| {
                                            envs.update(cx, |e, cx| {
                                                e.duplicate_environment(environment_id, cx);
                                            });
                                        }),
                                );
                                menu
                            }),
                    ),
            );

        // Filter rows based on search input
        let search_query = self.search_input.read(cx).text().to_string().to_lowercase();
        let filtered_rows: Vec<_> = self
            .rows
            .iter()
            .filter(|row| {
                if search_query.is_empty() {
                    return true;
                }
                let key = row.key_input.read(cx).text().to_string().to_lowercase();
                let value = row.value_input.read(cx).text().to_string().to_lowercase();
                key.contains(&search_query) || value.contains(&search_query)
            })
            .collect();

        // Main table content (rows or empty state)
        let content = if filtered_rows.is_empty() {
            if environment.variables.is_empty() {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.0))
                    .px(px(24.0))
                    .py(px(48.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(48.0))
                            .rounded_full()
                            .bg(theme.secondary)
                            .border_1()
                            .border_color(theme.border)
                            .child(IconName::Variable),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("No variables in this environment"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Add variables to store key-value pairs and reuse them across requests with {{variable_name}} syntax."),
                    )
                    .child(
                        Button::new("empty-add-var-btn")
                            .label("Add variable")
                            .icon(IconName::Plus)
                            .primary()
                            .small()
                            .on_click({
                                let envs = self.environments.clone();
                                move |_, _, cx| {
                                    envs.update(cx, |environments, cx| {
                                        environments.add_variable(environment_id, cx);
                                    });
                                }
                            }),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .py(px(48.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .child(format!("No variables match \"{}\"", search_query)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .child("Try adjusting your search query."),
                    )
                    .into_any_element()
            }
        } else {
            div()
                .id("env-var-rows-scroll")
                .flex_1()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .children(filtered_rows.iter().map(|row| {
                    let variable_id = row.id;
                    let key_text = row.key_input.read(cx).text().to_string();
                    let key_trimmed = key_text.trim().to_ascii_lowercase();
                    let is_duplicate = !key_trimmed.is_empty()
                        && key_counts.get(&key_trimmed).copied().unwrap_or(0) > 1;
                    let environments_for_toggle = self.environments.clone();
                    let environments_for_secret = self.environments.clone();
                    let environments_for_remove = self.environments.clone();
                    let secret = row.secret;
                    let enabled = row.enabled;

                    div()
                        .id(SharedString::from(format!("env-row-{variable_id}")))
                        .flex()
                        .flex_row()
                        .items_center()
                        .min_h(px(36.0))
                        .px(px(16.0))
                        .border_b_1()
                        .border_color(theme.border.opacity(0.3))
                        .hover(|s| s.bg(theme.secondary.opacity(0.3)))
                        .when(!enabled, |el| el.opacity(0.5))
                        .child(
                            div().w(px(32.0)).flex().items_center().child(
                                Checkbox::new(SharedString::from(format!(
                                    "env-check-{variable_id}"
                                )))
                                .checked(row.enabled)
                                .on_click(move |_, _, cx| {
                                    environments_for_toggle.update(cx, |environments, cx| {
                                        environments.toggle_variable(
                                            environment_id,
                                            variable_id,
                                            cx,
                                        );
                                    });
                                }),
                            ),
                        )
                        // Key input
                        .child(
                            div()
                                .w(px(220.0))
                                .min_w(px(220.0))
                                .when(is_duplicate, |d| d.text_color(theme.danger))
                                .child(Input::new(&row.key_input).appearance(false).small()),
                        )
                        // Value input
                        .child(
                            div().flex_1().min_w_0().child(CompletionInput::new(
                                &row.value_input,
                                Input::new(&row.value_input)
                                    .appearance(false)
                                    .small()
                                    .when(row.secret, |input| input.mask_toggle()),
                            )),
                        )
                        // Secret badge/toggle button
                        .child(
                            div().w(px(80.0)).flex().items_center().child(
                                Button::new(SharedString::from(format!(
                                    "env-secret-toggle-{variable_id}"
                                )))
                                .icon(if secret {
                                    IconName::Lock
                                } else {
                                    IconName::Unlock
                                })
                                .ghost()
                                .small()
                                .tooltip(if secret {
                                    "Secret value"
                                } else {
                                    "Regular value"
                                })
                                .on_click(move |_, _, cx| {
                                    environments_for_secret.update(cx, |environments, cx| {
                                        environments.toggle_secret(environment_id, variable_id, cx);
                                    });
                                }),
                            ),
                        )
                        // Delete button
                        .child(
                            div().w(px(40.0)).flex().items_center().justify_end().child(
                                Button::new(SharedString::from(format!(
                                    "env-del-btn-{variable_id}"
                                )))
                                .icon(IconName::Trash)
                                .ghost()
                                .small()
                                .tooltip("Delete")
                                .on_click(move |_, _, cx| {
                                    environments_for_remove.update(cx, |environments, cx| {
                                        environments.remove_variable(
                                            environment_id,
                                            variable_id,
                                            cx,
                                        );
                                    });
                                }),
                            ),
                        )
                }))
                .into_any_element()
        };

        // Main table container
        let table_container = div()
            .id("env-variables-table-container")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .bg(theme.muted)
            // Table column header row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(28.0))
                    .px(px(16.0))
                    .border_b_1()
                    .border_color(theme.border.opacity(0.5))
                    .bg(theme.secondary.opacity(0.5))
                    .child(div().w(px(32.0)))
                    .child(
                        div()
                            .w(px(220.0))
                            .min_w(px(220.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Key"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Value"),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_color(theme.muted_foreground.opacity(0.7))
                            .text_size(px(10.0))
                            .child("Secret"),
                    )
                    .child(div().w(px(40.0))),
            )
            // Scrollable table rows or empty state
            .child(content);

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .child(table_container)
            .into_any_element()
    }
}
