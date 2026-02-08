use gpui::prelude::*;
use gpui::{px, App, Application, Bounds, KeyBinding, Point, Size, WindowBounds, WindowOptions};
use gpui_component::Root;

use crate::actions::*;
use crate::assets::Assets;
use crate::theme::init_theme;
use crate::views::MainView;

/// Application configuration
pub struct SetuApp;

impl SetuApp {
    /// Initialize and run the application
    pub fn run() {
        env_logger::init();

        Application::new().with_assets(Assets).run(|cx: &mut App| {
            // Initialize gpui-component (must be called before using any gpui-component features)
            gpui_component::init(cx);

            // Apply our custom color theme to gpui-component
            init_theme(cx);

            // Register actions and keybindings
            Self::register_actions(cx);
            Self::register_keybindings(cx);

            // Create main window
            Self::create_main_window(cx);

            // Activate the application
            cx.activate(true);
        });
    }

    /// Register global actions
    fn register_actions(cx: &mut App) {
        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });
    }

    /// Register global keybindings
    fn register_keybindings(cx: &mut App) {
        cx.bind_keys([
            // ============ Application ============
            KeyBinding::new("cmd-q", Quit, None),
            // ============ Request Actions ============
            KeyBinding::new("cmd-enter", SendRequest, None),
            KeyBinding::new("ctrl-enter", SendRequest, None),
            KeyBinding::new("cmd-n", NewRequest, None),
            KeyBinding::new("cmd-d", DuplicateRequest, None),
            // ============ Tab Navigation ============
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("cmd-shift-]", NextTab, None),
            KeyBinding::new("alt-cmd-right", NextTab, None),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
            KeyBinding::new("cmd-shift-[", PreviousTab, None),
            KeyBinding::new("alt-cmd-left", PreviousTab, None),
            KeyBinding::new("cmd-w", CloseTab, None),
            KeyBinding::new("cmd-shift-w", CloseAllTabs, None),
            KeyBinding::new("cmd-alt-w", CloseOtherTabs, None),
            // Go to specific tab (like browsers/VSCode)
            KeyBinding::new("cmd-1", GoToTab1, None),
            KeyBinding::new("cmd-2", GoToTab2, None),
            KeyBinding::new("cmd-3", GoToTab3, None),
            KeyBinding::new("cmd-4", GoToTab4, None),
            KeyBinding::new("cmd-5", GoToTab5, None),
            KeyBinding::new("cmd-6", GoToTab6, None),
            KeyBinding::new("cmd-7", GoToTab7, None),
            KeyBinding::new("cmd-8", GoToTab8, None),
            KeyBinding::new("cmd-9", GoToLastTab, None),
            // ============ Focus & Navigation ============
            KeyBinding::new("cmd-l", FocusUrlBar, None),
            KeyBinding::new("cmd-u", FocusUrlBar, None),
            KeyBinding::new("cmd-shift-b", SwitchToBodyTab, None),
            KeyBinding::new("cmd-shift-p", SwitchToParamsTab, None),
            KeyBinding::new("cmd-shift-h", SwitchToHeadersTab, None),
            KeyBinding::new("cmd-shift-a", SwitchToAuthTab, None),
            KeyBinding::new("cmd-alt-b", SwitchToResponseBody, None),
            KeyBinding::new("cmd-alt-h", SwitchToResponseHeaders, None),
            // ============ UI Toggles ============
            KeyBinding::new("cmd-k", ToggleCommandPalette, None),
            KeyBinding::new("cmd-p", ToggleCommandPalette, None),
            KeyBinding::new("cmd-b", ToggleSidebar, None),
            KeyBinding::new("cmd-\\", ToggleSidebar, None),
            // ============ History ============
            KeyBinding::new("cmd-shift-delete", ClearHistory, None),
            // ============ HTTP Method Shortcuts ============
            // Alt + first letter (like Postman)
            KeyBinding::new("alt-g", SetMethodGet, None),
            KeyBinding::new("alt-p", SetMethodPost, None),
            KeyBinding::new("alt-u", SetMethodPut, None),
            KeyBinding::new("alt-d", SetMethodDelete, None),
            KeyBinding::new("alt-a", SetMethodPatch, None),
            KeyBinding::new("alt-h", SetMethodHead, None),
            KeyBinding::new("alt-o", SetMethodOptions, None),
        ]);
    }

    /// Create the main application window
    fn create_main_window(cx: &mut App) {
        let window_bounds = WindowBounds::Windowed(Bounds {
            origin: Point::default(),
            size: Size {
                width: px(1200.0),
                height: px(800.0),
            },
        });

        let options = WindowOptions {
            window_bounds: Some(window_bounds),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Setu".into()),
                appears_transparent: false,
                ..Default::default()
            }),
            ..Default::default()
        };

        // Wrap MainView in Root for gpui-component to work properly
        cx.open_window(options, |window, cx| {
            let main_view = cx.new(|cx| MainView::new(cx));
            cx.new(|cx| Root::new(main_view, window, cx))
        })
        .expect("Failed to open main window");
    }
}
