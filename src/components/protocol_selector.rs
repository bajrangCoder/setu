use gpui::prelude::*;
use gpui::{div, px, App, IntoElement, Styled, Window};

use gpui_component::ActiveTheme;

/// Available protocol types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtocolType {
    #[default]
    Rest,
    WebSocket,
    GraphQL,
    Sse,
}

impl ProtocolType {
    pub fn label(&self) -> &'static str {
        match self {
            ProtocolType::Rest => "REST",
            ProtocolType::WebSocket => "WebSocket",
            ProtocolType::GraphQL => "GraphQL",
            ProtocolType::Sse => "SSE",
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, ProtocolType::Rest)
    }
}

/// Protocol selector component
#[derive(IntoElement)]
pub struct ProtocolSelector {
    selected: ProtocolType,
}

impl ProtocolSelector {
    pub fn new(selected: ProtocolType) -> Self {
        Self { selected }
    }
}

impl RenderOnce for ProtocolSelector {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let protocols = [
            ProtocolType::Rest,
            ProtocolType::WebSocket,
            ProtocolType::GraphQL,
            ProtocolType::Sse,
        ];

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .p(px(2.0))
            .bg(theme.muted)
            .rounded(px(6.0))
            .children(protocols.into_iter().map(|protocol| {
                let is_selected = protocol == self.selected;
                let is_available = protocol.is_available();

                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .cursor(if is_available {
                        gpui::CursorStyle::PointingHand
                    } else {
                        gpui::CursorStyle::Arrow
                    })
                    .when(is_selected, |s| {
                        s.bg(theme.popover).text_color(theme.foreground)
                    })
                    .when(!is_selected && is_available, |s| {
                        s.text_color(theme.muted_foreground)
                            .hover(|s| s.text_color(theme.secondary_foreground))
                    })
                    .when(!is_available, |s| {
                        s.text_color(theme.muted_foreground).opacity(0.6)
                    })
                    .text_size(px(11.0))
                    .font_weight(if is_selected {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .child(protocol.label())
                    // "Soon" badge for unavailable protocols
                    .when(!is_available, |s| {
                        s.child(
                            div()
                                .px(px(4.0))
                                .py(px(1.0))
                                .bg(theme.primary.opacity(0.2))
                                .rounded(px(3.0))
                                .text_color(theme.primary)
                                .text_size(px(8.0))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child("SOON"),
                        )
                    })
            }))
    }
}
