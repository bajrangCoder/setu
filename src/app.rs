use gpui::prelude::*;
use gpui::{px, App, Application, Bounds, KeyBinding, Point, Size, WindowBounds, WindowOptions};
use gpui_component::Root;

use crate::actions::*;
use crate::assets::Assets;
use crate::theme::apply_setu_theme;
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
            apply_setu_theme(cx);

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
            // Application
            KeyBinding::new("cmd-q", Quit, None),
            // Request actions
            KeyBinding::new("cmd-enter", SendRequest, None),
            KeyBinding::new("cmd-n", NewRequest, None),
            // UI
            KeyBinding::new("cmd-k", ToggleCommandPalette, None),
            KeyBinding::new("cmd-b", ToggleSidebar, None),
            KeyBinding::new("cmd-l", FocusUrlBar, None),
            // Method shortcuts (when URL bar focused)
            KeyBinding::new("alt-g", SetMethodGet, Some("UrlBar")),
            KeyBinding::new("alt-p", SetMethodPost, Some("UrlBar")),
            KeyBinding::new("alt-u", SetMethodPut, Some("UrlBar")),
            KeyBinding::new("alt-d", SetMethodDelete, Some("UrlBar")),
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
