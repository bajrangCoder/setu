use gpui::prelude::*;
use gpui::{
    div, px, App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Styled,
};
use gpui_component::button::{Button, ButtonVariants, DropdownButton};
use gpui_component::menu::PopupMenuItem;
use gpui_component::Sizable;

use crate::theme::Theme;

/// Body content type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyType {
    None,
    #[default]
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

/// Event emitted when body type changes
#[derive(Clone, Debug)]
pub struct BodyTypeChanged(pub BodyType);

impl EventEmitter<BodyTypeChanged> for BodyTypeSelector {}

/// Body type selector
pub struct BodyTypeSelector {
    selected: BodyType,
    focus_handle: FocusHandle,
}

#[allow(dead_code)]
impl BodyTypeSelector {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            selected: BodyType::Json,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set selected type
    pub fn set_type(&mut self, body_type: BodyType, cx: &mut Context<Self>) {
        if self.selected != body_type {
            self.selected = body_type;
            cx.emit(BodyTypeChanged(body_type));
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
        let theme = Theme::dark();
        let this = cx.entity().clone();
        let selected_label: SharedString = self.selected.as_str().into();

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .py(px(6.0))
            .bg(theme.colors.bg_secondary)
            .border_b_1()
            .border_color(theme.colors.border_primary)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Content Type"),
                    )
                    .child(
                        DropdownButton::new("body-type-dropdown")
                            .button(
                                Button::new("body-type-btn")
                                    .label(selected_label)
                                    .small()
                                    .ghost(),
                            )
                            .small()
                            .dropdown_menu(move |menu, _window, _cx| {
                                let this = this.clone();
                                BodyType::all().iter().fold(menu, |menu, body_type| {
                                    let body_type = *body_type;
                                    let this = this.clone();
                                    menu.item(PopupMenuItem::new(body_type.as_str()).on_click(
                                        move |_, _, cx| {
                                            this.update(cx, |selector, cx| {
                                                selector.set_type(body_type, cx);
                                            });
                                        },
                                    ))
                                })
                            }),
                    ),
            )
    }
}
