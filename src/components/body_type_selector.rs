use gpui::prelude::*;
use gpui::{
    div, px, App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    SharedString, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::select::{Select, SelectEvent, SelectItem, SelectState};
use gpui_component::{ActiveTheme, Sizable};

use crate::icons::IconName;

/// Body content type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyType {
    #[default]
    None,
    Json,
    Text,
    FormUrlEncoded,
    FormData,
    Xml,
    Html,
}

impl BodyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BodyType::None => "none",
            BodyType::Json => "JSON",
            BodyType::Text => "Text",
            BodyType::FormUrlEncoded => "x-www-form-urlencoded",
            BodyType::FormData => "form-data",
            BodyType::Xml => "XML",
            BodyType::Html => "HTML",
        }
    }

    pub fn content_type(&self) -> Option<&'static str> {
        match self {
            BodyType::None => None,
            BodyType::Json => Some("application/json"),
            BodyType::Text => Some("text/plain"),
            BodyType::FormUrlEncoded => Some("application/x-www-form-urlencoded"),
            BodyType::FormData => Some("multipart/form-data"),
            BodyType::Xml => Some("application/xml"),
            BodyType::Html => Some("text/html"),
        }
    }

    pub fn syntax_language(&self) -> &'static str {
        match self {
            BodyType::Json => "json",
            BodyType::Xml => "xml",
            BodyType::Html => "html",
            _ => "text",
        }
    }

    pub fn all() -> &'static [BodyType] {
        &[
            BodyType::None,
            BodyType::Json,
            BodyType::Text,
            BodyType::FormUrlEncoded,
            BodyType::FormData,
            BodyType::Xml,
            BodyType::Html,
        ]
    }
}

/// Implement SelectItem for BodyType
impl SelectItem for BodyType {
    type Value = BodyType;

    fn title(&self) -> SharedString {
        self.as_str().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

/// Events emitted by the body type selector
#[derive(Clone, Debug)]
pub enum BodyTypeSelectorEvent {
    TypeChanged(BodyType),
    ImportRequested,
    BeautifyRequested,
    ClearRequested,
}

impl EventEmitter<BodyTypeSelectorEvent> for BodyTypeSelector {}

/// Body type selector
pub struct BodyTypeSelector {
    selected: BodyType,
    select_state: Entity<SelectState<Vec<BodyType>>>,
    focus_handle: FocusHandle,
}

#[allow(dead_code)]
impl BodyTypeSelector {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let items: Vec<BodyType> = BodyType::all().to_vec();
        let select_state = cx
            .new(|cx| SelectState::new(items, Some(gpui_component::IndexPath::new(0)), window, cx));

        // Subscribe to selection changes
        cx.subscribe(
            &select_state,
            |this, _, event: &SelectEvent<Vec<BodyType>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.selected = *value;
                    cx.emit(BodyTypeSelectorEvent::TypeChanged(*value));
                    cx.notify();
                }
            },
        )
        .detach();

        Self {
            selected: BodyType::None,
            select_state,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set selected type
    pub fn set_type(&mut self, body_type: BodyType, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != body_type {
            self.selected = body_type;
            self.select_state.update(cx, |state, cx| {
                state.set_selected_value(&body_type, window, cx);
            });
            cx.emit(BodyTypeSelectorEvent::TypeChanged(body_type));
            cx.notify();
        }
    }

    /// Get selected type
    pub fn selected(&self) -> BodyType {
        self.selected
    }
}

impl Focusable for BodyTypeSelector {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for BodyTypeSelector {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let this = cx.entity().clone();
        let this_for_beautify = cx.entity().clone();
        let this_for_clear = cx.entity().clone();

        // Only show import button for body types that support file import
        let show_import = matches!(
            self.selected,
            BodyType::Json | BodyType::Text | BodyType::Xml | BodyType::Html
        );

        // Only show beautify button for JSON
        let show_beautify = self.selected == BodyType::Json;

        // Show clear button for text-based body types (JSON, XML, HTML, Text)
        let show_clear = matches!(
            self.selected,
            BodyType::Json | BodyType::Xml | BodyType::Html | BodyType::Text
        );

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .py(px(6.0))
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Content Type"),
                    )
                    .child(
                        Select::new(&self.select_state)
                            .small()
                            .menu_width(px(200.0)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .when(show_beautify, |el| {
                        el.child(
                            Button::new("beautify-body")
                                .icon(IconName::Sparkles)
                                .ghost()
                                .xsmall()
                                .tooltip("Beautify JSON")
                                .on_click(move |_, _, cx| {
                                    this_for_beautify.update(cx, |_, cx| {
                                        cx.emit(BodyTypeSelectorEvent::BeautifyRequested);
                                    });
                                }),
                        )
                    })
                    .when(show_clear, |el| {
                        el.child(
                            Button::new("clear-body")
                                .icon(IconName::Trash)
                                .ghost()
                                .xsmall()
                                .tooltip("Clear body content")
                                .on_click(move |_, _, cx| {
                                    this_for_clear.update(cx, |_, cx| {
                                        cx.emit(BodyTypeSelectorEvent::ClearRequested);
                                    });
                                }),
                        )
                    })
                    .when(show_import, |el| {
                        el.child(
                            Button::new("import-body")
                                .icon(IconName::FileUp)
                                .ghost()
                                .xsmall()
                                .tooltip("Import from file")
                                .on_click(move |_, _, cx| {
                                    this.update(cx, |_, cx| {
                                        cx.emit(BodyTypeSelectorEvent::ImportRequested);
                                    });
                                }),
                        )
                    }),
            )
    }
}
