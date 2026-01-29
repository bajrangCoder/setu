use gpui::{Entity, Keystroke, Window};
use gpui_component::input::InputState;

/// Triggers the search panel in an editor by focusing it and dispatching Cmd+F.
pub fn trigger_editor_search(editor: Option<Entity<InputState>>, window: &Window) {
    window.on_next_frame(move |window, cx| {
        if let Some(editor) = editor {
            editor.update(cx, |state, cx| {
                state.focus(window, cx);
            });
            if let Ok(keystroke) = Keystroke::parse("cmd-f") {
                window.dispatch_keystroke(keystroke, cx);
            }
        }
    });
}
