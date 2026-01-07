use gpui::prelude::*;
use gpui::{div, px, App, FocusHandle, Focusable, IntoElement, Render, Styled, Window};

use gpui_component::ActiveTheme;

/// A command in the palette
#[derive(Debug, Clone)]
pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub icon: &'static str,
}

impl Command {
    pub const fn new(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            shortcut: None,
            icon: "⚡", // TODO: Replace with proper SVG icon
        }
    }

    pub const fn with_shortcut(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    pub const fn with_icon(mut self, icon: &'static str) -> Self {
        self.icon = icon;
        self
    }
}

/// Default commands available in the palette
pub fn default_commands() -> Vec<Command> {
    vec![
        // TODO: Replace emoji icons with proper SVG icons
        Command::new("send_request", "Send Request")
            .with_shortcut("⌘↵")
            .with_icon("🚀"),
        Command::new("new_request", "New Request")
            .with_shortcut("⌘N")
            .with_icon("➕"),
        Command::new("toggle_sidebar", "Toggle Sidebar")
            .with_shortcut("⌘B")
            .with_icon("📋"),
        Command::new("clear_history", "Clear History").with_icon("🗑️"),
        Command::new("focus_url", "Focus URL Bar")
            .with_shortcut("⌘L")
            .with_icon("🔗"),
        Command::new("set_get", "Set Method: GET").with_icon("🟢"),
        Command::new("set_post", "Set Method: POST").with_icon("🟠"),
        Command::new("set_put", "Set Method: PUT").with_icon("🔵"),
        Command::new("set_delete", "Set Method: DELETE").with_icon("🔴"),
    ]
}

/// Command Palette View
pub struct CommandPaletteView {
    is_open: bool,
    query: String,
    commands: Vec<Command>,
    selected_index: usize,
    focus_handle: FocusHandle,
}

impl CommandPaletteView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            is_open: false,
            query: String::new(),
            commands: default_commands(),
            selected_index: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.is_open = !self.is_open;
        if self.is_open {
            self.query.clear();
            self.selected_index = 0;
        }
        cx.notify();
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.is_open = true;
        self.query.clear();
        self.selected_index = 0;
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.is_open = false;
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    fn filtered_commands(&self) -> Vec<&Command> {
        if self.query.is_empty() {
            self.commands.iter().collect()
        } else {
            let query_lower = self.query.to_lowercase();
            self.commands
                .iter()
                .filter(|cmd| cmd.label.to_lowercase().contains(&query_lower))
                .collect()
        }
    }

    pub fn select_next(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_commands();
        if !filtered.is_empty() {
            self.selected_index = (self.selected_index + 1) % filtered.len();
            cx.notify();
        }
    }

    pub fn select_prev(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_commands();
        if !filtered.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                filtered.len() - 1
            } else {
                self.selected_index - 1
            };
            cx.notify();
        }
    }
}

impl Focusable for CommandPaletteView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CommandPaletteView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        if !self.is_open {
            return div();
        }

        let filtered = self.filtered_commands();
        let selected_index = self.selected_index;

        // Overlay backdrop
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(100.0))
            .bg(theme.overlay)
            .child(
                // Palette container
                div()
                    .track_focus(&self.focus_handle)
                    .flex()
                    .flex_col()
                    .w(px(500.0))
                    .max_h(px(400.0))
                    .bg(theme.popover)
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .overflow_hidden()
                    // Search input
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .px(px(16.0))
                            .h(px(52.0))
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                // TODO: Add proper SVG search icon
                                div().text_size(px(18.0)).child("🔍"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(if self.query.is_empty() {
                                        theme.muted_foreground
                                    } else {
                                        theme.foreground
                                    })
                                    .text_size(px(15.0))
                                    // TODO: Replace with gpui-component Input when needed
                                    .child(if self.query.is_empty() {
                                        "Type a command...".to_string()
                                    } else {
                                        self.query.clone()
                                    }),
                            ),
                    )
                    // Commands list
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .py(px(8.0))
                            .children(filtered.into_iter().enumerate().map(|(i, cmd)| {
                                let is_selected = i == selected_index;
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .px(px(16.0))
                                    .py(px(10.0))
                                    .cursor_pointer()
                                    .when(is_selected, |s| s.bg(theme.primary.opacity(0.15)))
                                    .hover(|s| s.bg(theme.muted))
                                    // Icon
                                    .child(div().text_size(px(16.0)).child(cmd.icon))
                                    // Label
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_color(theme.foreground)
                                            .text_size(px(14.0))
                                            .child(cmd.label),
                                    )
                                    // Shortcut
                                    .when_some(cmd.shortcut, |el, shortcut| {
                                        el.child(
                                            div()
                                                .px(px(8.0))
                                                .py(px(2.0))
                                                .bg(theme.secondary)
                                                .rounded(px(4.0))
                                                .text_color(theme.muted_foreground)
                                                .text_size(px(11.0))
                                                .child(shortcut),
                                        )
                                    })
                            })),
                    ),
            )
    }
}
