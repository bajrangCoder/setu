use gpui::{hsla, Hsla};

/// Color palette for the application
#[derive(Clone)]
pub struct Colors {
    // Backgrounds - subtle gradations
    pub bg_primary: Hsla,   // Main background
    pub bg_secondary: Hsla, // Sidebar, panels
    pub bg_tertiary: Hsla,  // Elevated cards, inputs
    pub bg_elevated: Hsla,  // Modals, dropdowns
    pub bg_overlay: Hsla,   // Backdrop

    // Foregrounds
    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_muted: Hsla,
    pub text_placeholder: Hsla,

    // Borders - very subtle
    pub border_primary: Hsla,
    pub border_secondary: Hsla,
    pub border_focus: Hsla,

    // Accent - vibrant but not overwhelming
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_muted: Hsla,

    // Semantic
    pub success: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
    pub info: Hsla,

    // HTTP Method colors - distinctive but harmonious
    pub method_get: Hsla,
    pub method_post: Hsla,
    pub method_put: Hsla,
    pub method_delete: Hsla,
    pub method_patch: Hsla,
    pub method_head: Hsla,
    pub method_options: Hsla,

    // Status code colors
    pub status_1xx: Hsla,
    pub status_2xx: Hsla,
    pub status_3xx: Hsla,
    pub status_4xx: Hsla,
    pub status_5xx: Hsla,
}

impl Colors {
    /// Dark theme - default, inspired by modern dev tools
    pub fn dark() -> Self {
        Self {
            // Backgrounds - deep, minimal contrast between levels
            bg_primary: hsla(240.0 / 360.0, 0.10, 0.08, 1.0), // #111318
            bg_secondary: hsla(240.0 / 360.0, 0.08, 0.10, 1.0), // #16181d
            bg_tertiary: hsla(240.0 / 360.0, 0.08, 0.12, 1.0), // #1c1e24
            bg_elevated: hsla(240.0 / 360.0, 0.10, 0.14, 1.0), // #21242b
            bg_overlay: hsla(0.0, 0.0, 0.0, 0.6),

            // Foregrounds - high contrast text
            text_primary: hsla(0.0, 0.0, 0.93, 1.0), // #ededed
            text_secondary: hsla(240.0 / 360.0, 0.05, 0.65, 1.0), // #a0a4ad
            text_muted: hsla(240.0 / 360.0, 0.04, 0.45, 1.0), // #6e7179
            text_placeholder: hsla(240.0 / 360.0, 0.03, 0.35, 1.0), // #555962

            // Borders - very subtle
            border_primary: hsla(240.0 / 360.0, 0.06, 0.18, 1.0), // #2a2d35
            border_secondary: hsla(240.0 / 360.0, 0.05, 0.22, 1.0), // #353840
            border_focus: hsla(165.0 / 360.0, 0.80, 0.50, 1.0),   // teal accent

            // Accent - teal/cyan like Hoppscotch but unique
            accent: hsla(165.0 / 360.0, 0.80, 0.45, 1.0), // #1db883 - vibrant teal
            accent_hover: hsla(165.0 / 360.0, 0.80, 0.52, 1.0),
            accent_muted: hsla(165.0 / 360.0, 0.40, 0.25, 1.0),

            // Semantic
            success: hsla(145.0 / 360.0, 0.70, 0.45, 1.0),
            warning: hsla(40.0 / 360.0, 0.95, 0.55, 1.0),
            error: hsla(0.0 / 360.0, 0.75, 0.55, 1.0),
            info: hsla(200.0 / 360.0, 0.80, 0.55, 1.0),

            // HTTP Methods - vibrant, distinct
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

    /// Light theme (for future use)
    pub fn light() -> Self {
        Self {
            bg_primary: hsla(0.0, 0.0, 0.98, 1.0),
            bg_secondary: hsla(240.0 / 360.0, 0.05, 0.96, 1.0),
            bg_tertiary: hsla(240.0 / 360.0, 0.05, 0.94, 1.0),
            bg_elevated: hsla(0.0, 0.0, 1.0, 1.0),
            bg_overlay: hsla(0.0, 0.0, 0.0, 0.4),

            text_primary: hsla(240.0 / 360.0, 0.10, 0.10, 1.0),
            text_secondary: hsla(240.0 / 360.0, 0.05, 0.40, 1.0),
            text_muted: hsla(240.0 / 360.0, 0.04, 0.55, 1.0),
            text_placeholder: hsla(240.0 / 360.0, 0.03, 0.65, 1.0),

            border_primary: hsla(240.0 / 360.0, 0.05, 0.88, 1.0),
            border_secondary: hsla(240.0 / 360.0, 0.05, 0.85, 1.0),
            border_focus: hsla(165.0 / 360.0, 0.80, 0.40, 1.0),

            accent: hsla(165.0 / 360.0, 0.80, 0.40, 1.0),
            accent_hover: hsla(165.0 / 360.0, 0.80, 0.35, 1.0),
            accent_muted: hsla(165.0 / 360.0, 0.40, 0.90, 1.0),

            success: hsla(145.0 / 360.0, 0.70, 0.40, 1.0),
            warning: hsla(40.0 / 360.0, 0.95, 0.50, 1.0),
            error: hsla(0.0 / 360.0, 0.75, 0.50, 1.0),
            info: hsla(200.0 / 360.0, 0.80, 0.50, 1.0),

            method_get: hsla(145.0 / 360.0, 0.70, 0.40, 1.0),
            method_post: hsla(280.0 / 360.0, 0.65, 0.50, 1.0),
            method_put: hsla(200.0 / 360.0, 0.75, 0.45, 1.0),
            method_delete: hsla(0.0 / 360.0, 0.75, 0.50, 1.0),
            method_patch: hsla(35.0 / 360.0, 0.90, 0.50, 1.0),
            method_head: hsla(180.0 / 360.0, 0.60, 0.40, 1.0),
            method_options: hsla(320.0 / 360.0, 0.60, 0.50, 1.0),

            status_1xx: hsla(200.0 / 360.0, 0.75, 0.50, 1.0),
            status_2xx: hsla(145.0 / 360.0, 0.70, 0.40, 1.0),
            status_3xx: hsla(35.0 / 360.0, 0.85, 0.50, 1.0),
            status_4xx: hsla(35.0 / 360.0, 0.90, 0.50, 1.0),
            status_5xx: hsla(0.0 / 360.0, 0.75, 0.50, 1.0),
        }
    }
}
