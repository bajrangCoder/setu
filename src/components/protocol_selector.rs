use gpui::prelude::*;
use gpui::{App, IntoElement, Styled, Window, div, px};
use gpui_component::ActiveTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtocolType {
    #[default]
    Rest,
    WebSocket,
    GraphQL,
    Sse,
}

impl ProtocolType {
    fn label(self) -> &'static str {
        match self {
            Self::Rest => "REST",
            Self::WebSocket => "WebSocket",
            Self::GraphQL => "GraphQL",
            Self::Sse => "SSE",
        }
    }

    fn is_available(self) -> bool {
        matches!(self, Self::Rest)
    }
}

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
                    .when(is_selected, |style| {
                        style.bg(theme.popover).text_color(theme.foreground)
                    })
                    .when(!is_selected && is_available, |style| {
                        style
                            .text_color(theme.muted_foreground)
                            .hover(|hover| hover.text_color(theme.secondary_foreground))
                    })
                    .when(!is_available, |style| {
                        style.text_color(theme.muted_foreground).opacity(0.6)
                    })
                    .text_size(px(11.0))
                    .font_weight(if is_selected {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .child(protocol.label())
                    .when(!is_available, |style| {
                        style.child(
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
