use gpui::prelude::*;
use gpui::{
    App, ElementId, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    ScrollHandle, SharedString, Styled, Window, div, px,
};

use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, WindowExt};

use crate::icons::IconName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    SendRequest,
    CancelRequest,
    NewRequest,
    DuplicateRequest,
    ToggleSidebar,
    ToggleRequestResponseLayout,
    FocusUrlBar,
    ClearHistory,
    SaveToCollection,
    ImportCollection,
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
}

impl Command {
    pub const fn new(id: CommandId, label: &'static str, _icon: IconName) -> Self {
        Self {
            id,
            label,
            shortcut: None,
        }
    }

    pub const fn with_shortcut(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    fn namespace(&self) -> &'static str {
        match self.id {
            CommandId::CloseTab
            | CommandId::CloseAllTabs
            | CommandId::CloseOtherTabs
            | CommandId::NextTab
            | CommandId::PreviousTab
            | CommandId::GoToTab1
            | CommandId::GoToTab2
            | CommandId::GoToTab3
            | CommandId::GoToTab4
            | CommandId::GoToTab5
            | CommandId::GoToTab6
            | CommandId::GoToTab7
            | CommandId::GoToTab8
            | CommandId::GoToLastTab => "tabs",
            CommandId::ToggleSidebar | CommandId::ToggleRequestResponseLayout => "view",
            CommandId::ClearHistory => "history",
            CommandId::ImportCollection | CommandId::SaveToCollection => "collections",
            CommandId::SwitchToResponseBody | CommandId::SwitchToResponseHeaders => "response",
            _ => "request",
        }
    }

    fn palette_label(&self) -> String {
        let action = self.label.to_ascii_lowercase().replace(": ", " ");
        format!("{}: {action}", self.namespace())
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
        Command::new(
            CommandId::ToggleRequestResponseLayout,
            "Toggle Request/Response Layout",
            IconName::LayoutSplit,
        ),
        Command::new(CommandId::FocusUrlBar, "Focus URL Bar", IconName::Link).with_shortcut("⌘L"),
        Command::new(
            CommandId::SaveToCollection,
            "Save to Collection",
            IconName::FilePlus,
        ),
        Command::new(
            CommandId::ImportCollection,
            "Import Collection",
            IconName::FileUp,
        ),
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
    command_labels: Vec<SharedString>,
    command_labels_lower: Vec<String>,
    filtered_indices: Vec<usize>,
    selected_index: usize,
    focus_handle: FocusHandle,
    app_focus_handle: FocusHandle,
    input_state: Option<Entity<InputState>>,
    query: String,
    scroll_handle: ScrollHandle,
}

impl CommandPaletteView {
    pub fn new(app_focus_handle: FocusHandle, cx: &mut Context<Self>) -> Self {
        let commands = default_commands();
        let command_labels = commands
            .iter()
            .map(|command| SharedString::from(command.palette_label()))
            .collect::<Vec<_>>();
        let command_labels_lower = command_labels
            .iter()
            .map(|label| label.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let filtered_indices = (0..commands.len()).collect::<Vec<_>>();

        Self {
            is_open: false,
            commands,
            command_labels,
            command_labels_lower,
            filtered_indices,
            selected_index: 0,
            focus_handle: cx.focus_handle(),
            app_focus_handle,
            input_state: None,
            query: String::new(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    fn ensure_input_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input_state.is_none() {
            let input_state =
                cx.new(|cx| InputState::new(window, cx).placeholder("Execute a command…"));

            cx.subscribe_in(
                &input_state,
                window,
                |this, state, event, window, cx| match event {
                    InputEvent::Change => {
                        this.query = state.read(cx).text().to_string();
                        this.selected_index = 0;
                        this.refresh_filtered_indices();
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        this.execute_selected(window, cx);
                    }
                    _ => {}
                },
            )
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
                });
            }
        }
        cx.notify();
    }

    pub fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.input_state {
            input.update(cx, |state, cx| state.focus(window, cx));
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.is_open = false;
        cx.notify();
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
        window.close_dialog(cx);
        self.app_focus_handle.focus(window, cx);
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

    fn execute_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(command_index) = self.filtered_indices.get(self.selected_index) {
            let cmd_id = self.commands[*command_index].id;
            self.is_open = false;
            cx.emit(CommandPaletteEvent::ExecuteCommand(cmd_id));
            cx.notify();
            self.close_dialog(window, cx);
        }
    }

    fn execute_command(
        &mut self,
        command_id: CommandId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_open = false;
        cx.emit(CommandPaletteEvent::ExecuteCommand(command_id));
        cx.notify();
        self.close_dialog(window, cx);
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
            .map(|input| Input::new(input).appearance(false));

        let hover_bg = theme.list_hover;
        let selected_bg = theme.list_active;
        let is_empty = self.filtered_indices.is_empty();

        div()
            .id("command-palette-container")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                match key {
                    "escape" => {
                        this.close_dialog(window, cx);
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
                        this.execute_selected(window, cx);
                    }
                    _ => {}
                }
            }))
            .flex()
            .flex_col()
            .w_full()
            .max_h(px(460.0))
            .overflow_hidden()
            .font_family(theme.font_family.clone())
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .h(px(46.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().flex_1().w_full().min_w_0().children(input_element)),
            )
            .child(
                div()
                    .id("command-list")
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .py(px(5.0))
                    .max_h(px(380.0))
                    .children(
                        self.filtered_indices
                            .iter()
                            .enumerate()
                            .map(|(i, cmd_index)| {
                                let cmd = &self.commands[*cmd_index];
                                let command_label = self.command_labels[*cmd_index].clone();
                                let is_selected = i == selected_index;
                                let cmd_id = cmd.id;
                                let item_id: ElementId =
                                    SharedString::from(format!("cmd-{}", i)).into();

                                div()
                                    .id(item_id)
                                    .flex()
                                    .items_center()
                                    .h(px(38.0))
                                    .px(px(12.0))
                                    .mx(px(6.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .when(is_selected, |style| style.bg(selected_bg))
                                    .hover(|style| style.bg(hover_bg))
                                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                                        if this.selected_index != i {
                                            this.selected_index = i;
                                            cx.notify();
                                        }
                                    }))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.execute_command(cmd_id, window, cx);
                                    }))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .text_color(if is_selected {
                                                theme.foreground
                                            } else {
                                                theme.secondary_foreground
                                            })
                                            .text_size(px(13.0))
                                            .font_weight(if is_selected {
                                                gpui::FontWeight::MEDIUM
                                            } else {
                                                gpui::FontWeight::NORMAL
                                            })
                                            .child(command_label),
                                    )
                                    .when_some(cmd.shortcut, |el, shortcut| {
                                        el.child(
                                            div()
                                                .ml(px(12.0))
                                                .text_color(theme.muted_foreground)
                                                .text_size(px(11.0))
                                                .child(shortcut),
                                        )
                                    })
                            }),
                    )
                    .when(is_empty, |list| {
                        list.child(
                            div()
                                .h(px(156.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.muted_foreground)
                                .child(div().text_size(px(13.0)).child("No matching commands")),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .h(px(34.0))
                    .px(px(12.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14.0))
                            .text_size(px(11.0))
                            .text_color(theme.muted_foreground)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child("Close")
                                    .child("esc"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .text_color(theme.secondary_foreground)
                                    .child("Run")
                                    .child("↵"),
                            ),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandId, default_commands};

    #[test]
    fn includes_collection_commands() {
        let commands = default_commands();
        assert!(
            commands
                .iter()
                .any(|command| matches!(command.id, CommandId::SaveToCollection))
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command.id, CommandId::ImportCollection))
        );
    }

    #[test]
    fn labels_commands_with_searchable_namespaces() {
        let commands = default_commands();
        let send = commands
            .iter()
            .find(|command| command.id == CommandId::SendRequest)
            .expect("send command");
        let close = commands
            .iter()
            .find(|command| command.id == CommandId::CloseTab)
            .expect("close command");

        assert_eq!(send.palette_label(), "request: send request");
        assert_eq!(close.palette_label(), "tabs: close tab");
    }
}
