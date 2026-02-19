use gpui::prelude::*;
use gpui::{
    div, px, App, ElementId, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    ScrollHandle, SharedString, Styled, Window,
};

use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, Icon, Sizable};

use crate::icons::IconName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    SendRequest,
    CancelRequest,
    NewRequest,
    DuplicateRequest,
    ToggleSidebar,
    FocusUrlBar,
    ClearHistory,
    SetMethodGet,
    SetMethodPost,
    SetMethodPut,
    SetMethodDelete,
    SetMethodPatch,
    SetMethodHead,
    SetMethodOptions,
    CloseTab,
    CloseAllTabs,
    CloseOtherTabs,
    NextTab,
    PreviousTab,
    GoToTab1,
    GoToTab2,
    GoToTab3,
    GoToTab4,
    GoToTab5,
    GoToTab6,
    GoToTab7,
    GoToTab8,
    GoToLastTab,
    SwitchToBodyTab,
    SwitchToParamsTab,
    SwitchToHeadersTab,
    SwitchToAuthTab,
    SwitchToResponseBody,
    SwitchToResponseHeaders,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub id: CommandId,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub icon: IconName,
}

impl Command {
    pub const fn new(id: CommandId, label: &'static str, icon: IconName) -> Self {
        Self {
            id,
            label,
            shortcut: None,
            icon,
        }
    }

    pub const fn with_shortcut(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }
}

pub fn default_commands() -> Vec<Command> {
    vec![
        Command::new(CommandId::SendRequest, "Send Request", IconName::Send).with_shortcut("⌘↵"),
        Command::new(CommandId::CancelRequest, "Cancel Request", IconName::Ban).with_shortcut("⌘."),
        Command::new(CommandId::NewRequest, "New Request", IconName::CopyPlus).with_shortcut("⌘N"),
        Command::new(
            CommandId::DuplicateRequest,
            "Duplicate Request",
            IconName::CopyPlus,
        )
        .with_shortcut("⌘D"),
        Command::new(CommandId::CloseTab, "Close Tab", IconName::Close).with_shortcut("⌘W"),
        Command::new(CommandId::CloseAllTabs, "Close All Tabs", IconName::Close)
            .with_shortcut("⌘⇧W"),
        Command::new(
            CommandId::CloseOtherTabs,
            "Close Other Tabs",
            IconName::Close,
        )
        .with_shortcut("⌘⌥W"),
        Command::new(CommandId::NextTab, "Next Tab", IconName::ChevronDown).with_shortcut("⌃⇥"),
        Command::new(CommandId::PreviousTab, "Previous Tab", IconName::ChevronUp)
            .with_shortcut("⌃⇧⇥"),
        Command::new(CommandId::GoToTab1, "Go to Tab 1", IconName::CircleDot).with_shortcut("⌘1"),
        Command::new(CommandId::GoToTab2, "Go to Tab 2", IconName::CircleDot).with_shortcut("⌘2"),
        Command::new(CommandId::GoToTab3, "Go to Tab 3", IconName::CircleDot).with_shortcut("⌘3"),
        Command::new(CommandId::GoToTab4, "Go to Tab 4", IconName::CircleDot).with_shortcut("⌘4"),
        Command::new(CommandId::GoToTab5, "Go to Tab 5", IconName::CircleDot).with_shortcut("⌘5"),
        Command::new(CommandId::GoToTab6, "Go to Tab 6", IconName::CircleDot).with_shortcut("⌘6"),
        Command::new(CommandId::GoToTab7, "Go to Tab 7", IconName::CircleDot).with_shortcut("⌘7"),
        Command::new(CommandId::GoToTab8, "Go to Tab 8", IconName::CircleDot).with_shortcut("⌘8"),
        Command::new(
            CommandId::GoToLastTab,
            "Go to Last Tab",
            IconName::ChevronDown,
        )
        .with_shortcut("⌘9"),
        Command::new(
            CommandId::ToggleSidebar,
            "Toggle Sidebar",
            IconName::PanelLeft,
        )
        .with_shortcut("⌘B"),
        Command::new(CommandId::FocusUrlBar, "Focus URL Bar", IconName::Link).with_shortcut("⌘L"),
        Command::new(
            CommandId::SwitchToBodyTab,
            "Switch to Body Tab",
            IconName::CircleDot,
        )
        .with_shortcut("⌘⇧B"),
        Command::new(
            CommandId::SwitchToParamsTab,
            "Switch to Params Tab",
            IconName::CircleDot,
        )
        .with_shortcut("⌘⇧P"),
        Command::new(
            CommandId::SwitchToHeadersTab,
            "Switch to Headers Tab",
            IconName::CircleDot,
        )
        .with_shortcut("⌘⇧H"),
        Command::new(
            CommandId::SwitchToAuthTab,
            "Switch to Auth Tab",
            IconName::CircleDot,
        )
        .with_shortcut("⌘⇧A"),
        Command::new(
            CommandId::SwitchToResponseBody,
            "Switch to Response Body",
            IconName::CircleDot,
        )
        .with_shortcut("⌘⌥B"),
        Command::new(
            CommandId::SwitchToResponseHeaders,
            "Switch to Response Headers",
            IconName::CircleDot,
        )
        .with_shortcut("⌘⌥H"),
        Command::new(CommandId::ClearHistory, "Clear History", IconName::Trash)
            .with_shortcut("⌘⇧⌫"),
        Command::new(
            CommandId::SetMethodGet,
            "Set Method: GET",
            IconName::CircleDot,
        )
        .with_shortcut("⌥G"),
        Command::new(
            CommandId::SetMethodPost,
            "Set Method: POST",
            IconName::CircleDot,
        )
        .with_shortcut("⌥P"),
        Command::new(
            CommandId::SetMethodPut,
            "Set Method: PUT",
            IconName::CircleDot,
        )
        .with_shortcut("⌥U"),
        Command::new(
            CommandId::SetMethodDelete,
            "Set Method: DELETE",
            IconName::CircleDot,
        )
        .with_shortcut("⌥D"),
        Command::new(
            CommandId::SetMethodPatch,
            "Set Method: PATCH",
            IconName::CircleDot,
        )
        .with_shortcut("⌥A"),
        Command::new(
            CommandId::SetMethodHead,
            "Set Method: HEAD",
            IconName::CircleDot,
        )
        .with_shortcut("⌥H"),
        Command::new(
            CommandId::SetMethodOptions,
            "Set Method: OPTIONS",
            IconName::CircleDot,
        )
        .with_shortcut("⌥O"),
    ]
}

#[derive(Clone)]
pub enum CommandPaletteEvent {
    ExecuteCommand(CommandId),
}

pub struct CommandPaletteView {
    is_open: bool,
    commands: Vec<Command>,
    command_labels_lower: Vec<String>,
    filtered_indices: Vec<usize>,
    selected_index: usize,
    focus_handle: FocusHandle,
    input_state: Option<Entity<InputState>>,
    query: String,
    scroll_handle: ScrollHandle,
}

impl CommandPaletteView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let commands = default_commands();
        let command_labels_lower = commands
            .iter()
            .map(|command| command.label.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let filtered_indices = (0..commands.len()).collect::<Vec<_>>();

        Self {
            is_open: false,
            commands,
            command_labels_lower,
            filtered_indices,
            selected_index: 0,
            focus_handle: cx.focus_handle(),
            input_state: None,
            query: String::new(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    fn ensure_input_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input_state.is_none() {
            let input_state =
                cx.new(|cx| InputState::new(window, cx).placeholder("Type a command..."));

            cx.subscribe_in(&input_state, window, |this, state, event, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.query = state.read(cx).text().to_string();
                    this.selected_index = 0;
                    this.refresh_filtered_indices();
                    cx.notify();
                }
            })
            .detach();

            self.input_state = Some(input_state);
        }
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_open = !self.is_open;
        if self.is_open {
            self.query.clear();
            self.selected_index = 0;
            self.refresh_filtered_indices();
            self.scroll_handle = ScrollHandle::new();
            self.ensure_input_state(window, cx);
            if let Some(ref input) = self.input_state {
                input.update(cx, |state, cx| {
                    state.set_value(String::new(), window, cx);
                    state.focus(window, cx);
                });
            }
        }
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.is_open = false;
        cx.notify();
    }

    fn refresh_filtered_indices(&mut self) {
        let query = self.query.trim();
        self.filtered_indices.clear();

        if query.is_empty() {
            self.filtered_indices.extend(0..self.commands.len());
            return;
        }

        let query_lower = query.to_ascii_lowercase();
        self.filtered_indices
            .extend(self.command_labels_lower.iter().enumerate().filter_map(
                |(index, label_lower)| {
                    if label_lower.contains(&query_lower) {
                        Some(index)
                    } else {
                        None
                    }
                },
            ));
    }

    pub fn select_next(&mut self) -> bool {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
            self.scroll_handle.scroll_to_item(self.selected_index);
            return true;
        }
        false
    }

    pub fn select_prev(&mut self) -> bool {
        if !self.filtered_indices.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.filtered_indices.len() - 1
            } else {
                self.selected_index - 1
            };
            self.scroll_handle.scroll_to_item(self.selected_index);
            return true;
        }
        false
    }

    fn execute_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(command_index) = self.filtered_indices.get(self.selected_index) {
            let cmd_id = self.commands[*command_index].id;
            self.is_open = false;
            cx.emit(CommandPaletteEvent::ExecuteCommand(cmd_id));
            cx.notify();
        }
    }

    fn execute_command(&mut self, command_id: CommandId, cx: &mut Context<Self>) {
        self.is_open = false;
        cx.emit(CommandPaletteEvent::ExecuteCommand(command_id));
        cx.notify();
    }
}

impl EventEmitter<CommandPaletteEvent> for CommandPaletteView {}

impl Focusable for CommandPaletteView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CommandPaletteView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.is_open {
            return div().into_any_element();
        }

        self.ensure_input_state(window, cx);

        let theme = cx.theme();

        let selected_index = self
            .selected_index
            .min(self.filtered_indices.len().saturating_sub(1));

        let input_element = self
            .input_state
            .as_ref()
            .map(|input| Input::new(input).xsmall().appearance(false));

        let item_bg = theme.secondary.opacity(0.4);

        div()
            .id("command-palette-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(80.0))
            .bg(theme.overlay)
            .on_click(cx.listener(|this, _, _, cx| {
                this.close(cx);
            }))
            .child(
                div()
                    .id("command-palette-container")
                    .track_focus(&self.focus_handle)
                    .on_click(|_, _, _| {})
                    .on_key_down(
                        cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                            let key = event.keystroke.key.as_str();
                            match key {
                                "escape" => {
                                    this.close(cx);
                                }
                                "down" => {
                                    if this.select_next() {
                                        cx.notify();
                                    }
                                }
                                "up" => {
                                    if this.select_prev() {
                                        cx.notify();
                                    }
                                }
                                "enter" => {
                                    this.execute_selected(cx);
                                }
                                _ => {}
                            }
                        }),
                    )
                    .flex()
                    .flex_col()
                    .w(px(520.0))
                    .max_h(px(450.0))
                    .bg(theme.popover)
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .overflow_hidden()
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
                                Icon::new(IconName::Search)
                                    .small()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(div().flex_1().w_full().min_w_0().children(input_element)),
                    )
                    .child(
                        div()
                            .id("command-list")
                            .flex()
                            .flex_col()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .py(px(6.0))
                            .max_h(px(350.0))
                            .children(self.filtered_indices.iter().enumerate().map(
                                |(i, cmd_index)| {
                                    let cmd = &self.commands[*cmd_index];
                                    let is_selected = i == selected_index;
                                    let cmd_id = cmd.id;
                                    let item_id: ElementId =
                                        SharedString::from(format!("cmd-{}", i)).into();

                                    div()
                                        .id(item_id)
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .px(px(14.0))
                                        .py(px(10.0))
                                        .mx(px(6.0))
                                        .rounded(px(8.0))
                                        .cursor_pointer()
                                        .when(is_selected, |s| s.bg(item_bg))
                                        .hover(|s| s.bg(item_bg))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.execute_command(cmd_id, cx);
                                        }))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .w(px(28.0))
                                                .h(px(28.0))
                                                .rounded(px(6.0))
                                                .bg(theme.secondary.opacity(0.5))
                                                .child(
                                                    Icon::new(cmd.icon)
                                                        .xsmall()
                                                        .text_color(theme.primary),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_color(theme.foreground)
                                                .text_size(px(14.0))
                                                .child(cmd.label),
                                        )
                                        .when_some(cmd.shortcut, |el, shortcut| {
                                            el.child(
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(4.0))
                                                    .bg(theme.secondary)
                                                    .rounded(px(5.0))
                                                    .text_color(theme.muted_foreground)
                                                    .text_size(px(11.0))
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .child(shortcut),
                                            )
                                        })
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(16.0))
                            .py(px(10.0))
                            .border_t_1()
                            .border_color(theme.border)
                            .bg(theme.muted.opacity(0.3))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(16.0))
                                    .text_size(px(11.0))
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .child("↑↓")
                                            .child("navigate"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .child("↵")
                                            .child("select"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .child("esc")
                                            .child("close"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(
                                        Icon::new(IconName::Command)
                                            .xsmall()
                                            .text_color(theme.muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(theme.muted_foreground)
                                            .child("Setu"),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
