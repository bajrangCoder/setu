use gpui::App;
use gpui_component::Theme as GpuiTheme;

use super::Colors;

/// Apply our custom colors to gpui-component's global theme
pub fn apply_setu_theme(cx: &mut App) {
    let colors = Colors::dark();

    // Get mutable access to the global theme
    // Theme derefs to ThemeColor, so we can set colors directly
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

    // List colors (for sidebar items, dropdown menus)
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

    // Caret (cursor in inputs)
    theme.caret = colors.accent;
}

/// Light theme variant (for future use)
#[allow(dead_code)]
pub fn apply_setu_light_theme(cx: &mut App) {
    let colors = Colors::light();
    let theme = GpuiTheme::global_mut(cx);

    theme.background = colors.bg_primary;
    theme.secondary = colors.bg_secondary;
    theme.muted = colors.bg_tertiary;
    theme.foreground = colors.text_primary;
    theme.secondary_foreground = colors.text_secondary;
    theme.muted_foreground = colors.text_muted;
    theme.border = colors.border_primary;
    theme.primary = colors.accent;
    theme.primary_foreground = colors.text_primary;
    theme.sidebar = colors.bg_secondary;
    theme.sidebar_foreground = colors.text_primary;
    theme.sidebar_border = colors.border_primary;
}
