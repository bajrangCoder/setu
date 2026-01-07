use gpui::{hsla, App, Hsla};
use gpui_component::{highlighter::HighlightTheme, Theme as GpuiTheme};

pub fn init_theme(cx: &mut App) {
    apply_setu_teal_theme(cx);
}

/// Setu Teal theme color palette
struct SetuColors {
    // Backgrounds
    bg_primary: Hsla,
    bg_secondary: Hsla,
    bg_tertiary: Hsla,
    bg_elevated: Hsla,
    bg_overlay: Hsla,

    // Text
    text_primary: Hsla,
    text_secondary: Hsla,
    text_muted: Hsla,

    // Borders
    border_primary: Hsla,
    border_secondary: Hsla,
    border_focus: Hsla,

    // Accent
    accent: Hsla,
    accent_hover: Hsla,
    accent_muted: Hsla,

    // Semantic
    success: Hsla,
    warning: Hsla,
    error: Hsla,
    info: Hsla,

    // HTTP Methods
    method_get: Hsla,
    method_post: Hsla,
    method_put: Hsla,
    method_delete: Hsla,
    method_patch: Hsla,
    method_head: Hsla,
    method_options: Hsla,

    // Status codes
    status_1xx: Hsla,
    status_2xx: Hsla,
    status_3xx: Hsla,
    status_4xx: Hsla,
    status_5xx: Hsla,
}

impl SetuColors {
    fn teal() -> Self {
        Self {
            // Backgrounds
            bg_primary: hsla(240.0 / 360.0, 0.10, 0.08, 1.0), // #111318
            bg_secondary: hsla(240.0 / 360.0, 0.08, 0.10, 1.0), // #16181d
            bg_tertiary: hsla(240.0 / 360.0, 0.08, 0.12, 1.0), // #1c1e24
            bg_elevated: hsla(240.0 / 360.0, 0.10, 0.14, 1.0), // #21242b
            bg_overlay: hsla(0.0, 0.0, 0.0, 0.6),

            // Foregrounds
            text_primary: hsla(0.0, 0.0, 0.93, 1.0), // #ededed
            text_secondary: hsla(240.0 / 360.0, 0.05, 0.65, 1.0), // #a0a4ad
            text_muted: hsla(240.0 / 360.0, 0.04, 0.45, 1.0), // #6e7179

            // Borders
            border_primary: hsla(240.0 / 360.0, 0.06, 0.18, 1.0), // #2a2d35
            border_secondary: hsla(240.0 / 360.0, 0.05, 0.22, 1.0), // #353840
            border_focus: hsla(165.0 / 360.0, 0.80, 0.50, 1.0),   // teal accent

            // Accent - teal/cyan
            accent: hsla(165.0 / 360.0, 0.80, 0.45, 1.0), // #1db883
            accent_hover: hsla(165.0 / 360.0, 0.80, 0.52, 1.0),
            accent_muted: hsla(165.0 / 360.0, 0.40, 0.25, 1.0),

            // Semantic
            success: hsla(145.0 / 360.0, 0.70, 0.45, 1.0),
            warning: hsla(40.0 / 360.0, 0.95, 0.55, 1.0),
            error: hsla(0.0 / 360.0, 0.75, 0.55, 1.0),
            info: hsla(200.0 / 360.0, 0.80, 0.55, 1.0),

            // HTTP Methods
            method_get: hsla(145.0 / 360.0, 0.70, 0.50, 1.0), // green
            method_post: hsla(280.0 / 360.0, 0.65, 0.60, 1.0), // purple/magenta
            method_put: hsla(200.0 / 360.0, 0.75, 0.55, 1.0), // blue
            method_delete: hsla(0.0 / 360.0, 0.75, 0.55, 1.0), // red
            method_patch: hsla(35.0 / 360.0, 0.90, 0.55, 1.0), // orange
            method_head: hsla(180.0 / 360.0, 0.60, 0.45, 1.0), // cyan
            method_options: hsla(320.0 / 360.0, 0.60, 0.55, 1.0), // pink

            // Status codes
            status_1xx: hsla(200.0 / 360.0, 0.75, 0.55, 1.0),
            status_2xx: hsla(145.0 / 360.0, 0.70, 0.50, 1.0),
            status_3xx: hsla(35.0 / 360.0, 0.85, 0.55, 1.0),
            status_4xx: hsla(35.0 / 360.0, 0.90, 0.55, 1.0),
            status_5xx: hsla(0.0 / 360.0, 0.75, 0.55, 1.0),
        }
    }
}

fn apply_setu_teal_theme(cx: &mut App) {
    let colors = SetuColors::teal();
    let theme = GpuiTheme::global_mut(cx);

    // Background colors
    theme.background = colors.bg_primary;
    theme.secondary = colors.bg_secondary;
    theme.muted = colors.bg_tertiary;

    // Foreground/text colors
    theme.foreground = colors.text_primary;
    theme.secondary_foreground = colors.text_secondary;
    theme.muted_foreground = colors.text_muted;

    // Border colors
    theme.border = colors.border_primary;
    theme.input = colors.border_secondary;
    theme.ring = colors.border_focus;

    // Accent/primary colors
    theme.primary = colors.accent;
    theme.primary_hover = colors.accent_hover;
    theme.primary_foreground = colors.text_primary;
    theme.accent = colors.accent_muted;
    theme.accent_foreground = colors.text_primary;

    // Semantic colors
    theme.danger = colors.error;
    theme.danger_foreground = colors.text_primary;
    theme.danger_hover = colors.error;
    theme.success = colors.success;
    theme.success_foreground = colors.text_primary;
    theme.warning = colors.warning;
    theme.warning_foreground = colors.text_primary;
    theme.info = colors.info;
    theme.info_foreground = colors.text_primary;

    // Sidebar specific colors
    theme.sidebar = colors.bg_secondary;
    theme.sidebar_foreground = colors.text_primary;
    theme.sidebar_border = colors.border_primary;
    theme.sidebar_accent = colors.bg_tertiary;
    theme.sidebar_accent_foreground = colors.text_primary;
    theme.sidebar_primary = colors.accent;
    theme.sidebar_primary_foreground = colors.text_primary;

    // Tab bar colors
    theme.tab_bar = colors.bg_secondary;
    theme.tab = colors.bg_tertiary;
    theme.tab_foreground = colors.text_secondary;
    theme.tab_active = colors.bg_tertiary;
    theme.tab_active_foreground = colors.text_primary;

    // Title bar
    theme.title_bar = colors.bg_secondary;
    theme.title_bar_border = colors.border_primary;

    // Scrollbar
    theme.scrollbar = colors.bg_tertiary;
    theme.scrollbar_thumb = colors.border_secondary;
    theme.scrollbar_thumb_hover = colors.text_muted;

    // Selection
    theme.selection = colors.accent_muted;

    // Link
    theme.link = colors.accent;
    theme.link_hover = colors.accent_hover;
    theme.link_active = colors.accent;

    // Popover/dropdown
    theme.popover = colors.bg_elevated;
    theme.popover_foreground = colors.text_primary;

    // List colors
    theme.list = colors.bg_secondary;
    theme.list_hover = colors.bg_tertiary;
    theme.list_active = colors.accent_muted;
    theme.list_active_border = colors.accent;
    theme.list_head = colors.bg_secondary;
    theme.list_even = colors.bg_secondary;

    // Table colors
    theme.table = colors.bg_secondary;
    theme.table_hover = colors.bg_tertiary;
    theme.table_head = colors.bg_tertiary;
    theme.table_head_foreground = colors.text_secondary;

    // Accordion
    theme.accordion = colors.bg_secondary;
    theme.accordion_hover = colors.bg_tertiary;

    // Progress bar
    theme.progress_bar = colors.accent;

    // Overlay
    theme.overlay = colors.bg_overlay;

    // Caret
    theme.caret = colors.accent;

    // Highlight theme for code editor
    theme.highlight_theme = HighlightTheme::default_dark();
    let highlight_theme = std::sync::Arc::make_mut(&mut theme.highlight_theme);
    highlight_theme.style.editor_background = Some(colors.bg_tertiary);
    highlight_theme.style.editor_foreground = Some(colors.text_primary);
    highlight_theme.style.editor_active_line = Some(colors.bg_elevated);
    highlight_theme.style.editor_line_number = Some(colors.text_muted);
    highlight_theme.style.editor_active_line_number = Some(colors.accent);
}

/// Get HTTP method color
pub fn method_color(method: &crate::entities::HttpMethod, cx: &App) -> Hsla {
    use crate::entities::HttpMethod;

    let colors = SetuColors::teal();
    match method {
        HttpMethod::Get => colors.method_get,
        HttpMethod::Post => colors.method_post,
        HttpMethod::Put => colors.method_put,
        HttpMethod::Delete => colors.method_delete,
        HttpMethod::Patch => colors.method_patch,
        HttpMethod::Head => colors.method_head,
        HttpMethod::Options => colors.method_options,
    }
}

/// Get status code color
pub fn status_color(status_code: u16, cx: &App) -> Hsla {
    let colors = SetuColors::teal();
    match status_code / 100 {
        1 => colors.status_1xx,
        2 => colors.status_2xx,
        3 => colors.status_3xx,
        4 => colors.status_4xx,
        5 => colors.status_5xx,
        _ => colors.text_muted,
    }
}
