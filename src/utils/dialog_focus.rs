use gpui::{App, FocusHandle, Global, Window};
use gpui_component::WindowExt;
use gpui_component::dialog::Dialog;

struct AppFocusHandle(FocusHandle);

impl Global for AppFocusHandle {}

pub fn set_app_focus_handle(focus_handle: FocusHandle, cx: &mut App) {
    cx.set_global(AppFocusHandle(focus_handle));
}

pub fn restore_app_focus(window: &mut Window, cx: &mut App) {
    if let Some(app_focus) = cx.try_global::<AppFocusHandle>() {
        app_focus.0.clone().focus(window, cx);
    }
}

pub fn close_dialog(window: &mut Window, cx: &mut App) {
    window.close_dialog(cx);
    restore_app_focus(window, cx);
}

pub fn open_dialog(
    window: &mut Window,
    cx: &mut App,
    build: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
) {
    window.open_dialog(cx, move |dialog, window, cx| {
        build(dialog, window, cx).on_close(|_, window, cx| {
            restore_app_focus(window, cx);
        })
    });
}
