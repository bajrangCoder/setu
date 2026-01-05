use gpui::{px, Pixels};

/// Spacing and layout tokens
#[derive(Clone)]
pub struct Spacing {
    // Base spacing scale (4px base)
    pub px_0: Pixels,
    pub px_1: Pixels,  // 4px
    pub px_2: Pixels,  // 8px
    pub px_3: Pixels,  // 12px
    pub px_4: Pixels,  // 16px
    pub px_5: Pixels,  // 20px
    pub px_6: Pixels,  // 24px
    pub px_8: Pixels,  // 32px
    pub px_10: Pixels, // 40px
    pub px_12: Pixels, // 48px
    pub px_16: Pixels, // 64px

    // Border radius
    pub radius_none: Pixels,
    pub radius_sm: Pixels,
    pub radius_md: Pixels,
    pub radius_lg: Pixels,
    pub radius_xl: Pixels,
    pub radius_full: Pixels,

    // Common widths
    pub sidebar_width: Pixels,
    pub header_height: Pixels,
    pub input_height: Pixels,
    pub button_height: Pixels,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            // Base scale
            px_0: px(0.0),
            px_1: px(4.0),
            px_2: px(8.0),
            px_3: px(12.0),
            px_4: px(16.0),
            px_5: px(20.0),
            px_6: px(24.0),
            px_8: px(32.0),
            px_10: px(40.0),
            px_12: px(48.0),
            px_16: px(64.0),

            // Border radius
            radius_none: px(0.0),
            radius_sm: px(4.0),
            radius_md: px(6.0),
            radius_lg: px(8.0),
            radius_xl: px(12.0),
            radius_full: px(9999.0),

            // Layout widths
            sidebar_width: px(280.0),
            header_height: px(48.0),
            input_height: px(36.0),
            button_height: px(32.0),
        }
    }
}
