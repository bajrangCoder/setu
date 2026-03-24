use gpui::prelude::*;
use gpui::{div, hsla, px, App, ClickEvent, Entity, IntoElement, Styled, Window};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants, DropdownButton};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::ActiveTheme;
use std::rc::Rc;

use crate::components::{MethodDropdown, MethodDropdownState};
use crate::entities::RequestEntity;
use crate::icons::IconName;

/// Callback type for Send button
pub type OnSendCallback = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
pub type OnCancelCallback = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
pub type OnSaveToCollectionCallback = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// URL Bar component
#[derive(IntoElement)]
pub struct UrlBar {
    input_state: Entity<InputState>,
    method_dropdown: Option<Entity<MethodDropdownState>>,
    request: Option<Entity<RequestEntity>>,
    is_loading: bool,
    on_send: Option<OnSendCallback>,
    on_cancel: Option<OnCancelCallback>,
    on_save_to_collection: Option<OnSaveToCollectionCallback>,
}

impl UrlBar {
    pub fn new(input_state: Entity<InputState>) -> Self {
        Self {
            input_state,
            method_dropdown: None,
            request: None,
            is_loading: false,
            on_send: None,
            on_cancel: None,
            on_save_to_collection: None,
        }
    }

    pub fn method_dropdown(
        mut self,
        dropdown_state: Entity<MethodDropdownState>,
        request: Entity<RequestEntity>,
    ) -> Self {
        self.method_dropdown = Some(dropdown_state);
        self.request = Some(request);
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.is_loading = loading;
        self
    }

    pub fn on_send(
        mut self,
        callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_send = Some(Rc::new(callback));
        self
    }

    pub fn on_cancel(
        mut self,
        callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Rc::new(callback));
        self
    }

    pub fn on_save_to_collection(
        mut self,
        callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_save_to_collection = Some(Rc::new(callback));
        self
    }
}

impl RenderOnce for UrlBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let is_loading = self.is_loading;
        let on_send = self.on_send;
        let on_cancel = self.on_cancel;
        let on_save_to_collection = self.on_save_to_collection;
        let send_variant = ButtonCustomVariant::new(cx)
            .color(hsla(168.0 / 360.0, 0.67, 0.47, 1.0))
            .foreground(theme.background)
            .border(hsla(168.0 / 360.0, 0.67, 0.47, 1.0))
            .hover(hsla(168.0 / 360.0, 0.67, 0.44, 1.0))
            .active(hsla(168.0 / 360.0, 0.67, 0.41, 1.0));

        let primary_button = Button::new("send-request-primary")
            .label(if is_loading { "Cancel" } else { "Send" })
            .compact()
            .on_click(move |event, window, cx| {
                if is_loading {
                    if let Some(ref callback) = on_cancel {
                        callback(event, window, cx);
                    }
                } else if let Some(ref callback) = on_send {
                    callback(event, window, cx);
                }
            });

        let split_button = DropdownButton::new("send-request-split").button(primary_button);

        let split_button = if is_loading {
            split_button.danger()
        } else {
            split_button.custom(send_variant)
        }
        .dropdown_menu(move |menu: PopupMenu, _window, _cx| {
            let mut menu = menu;
            if let Some(callback) = on_save_to_collection.clone() {
                menu = menu.item(
                    PopupMenuItem::new("Save to Collection")
                        .icon(IconName::FilePlus)
                        .on_click(move |event, window, cx| {
                            callback(event, window, cx);
                        }),
                );
            }
            menu
        });

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .w_full()
            .h(px(40.0))
            .bg(theme.muted)
            .rounded(px(6.0))
            // Method dropdown trigger
            .when_some(
                self.method_dropdown.zip(self.request),
                |el, (dropdown_state, request)| {
                    el.child(
                        div()
                            .ml(px(4.0))
                            .child(MethodDropdown::new(dropdown_state, request)),
                    )
                },
            )
            // Divider
            .child(div().w(px(1.0)).h(px(20.0)).bg(theme.border))
            // URL input using gpui-component
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .h_full()
                    .px(px(8.0))
                    .child(
                        Input::new(&self.input_state)
                            .appearance(false) // Remove default styling
                            .size_full(),
                    ),
            )
            .child(div().mr(px(4.0)).child(split_button))
    }
}
