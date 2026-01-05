use std::io::Write;

use gpui::prelude::*;
use gpui::{
    div, px, App, Context, FocusHandle, Focusable, IntoElement, Render, SharedString, Styled,
    Window,
};
use gpui_component::button::{Button, ButtonVariants, DropdownButton};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::PopupMenuItem;
use gpui_component::Sizable;

use crate::theme::Theme;

/// Authentication type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthType {
    #[default]
    None,
    Basic,
    Bearer,
    ApiKey,
}

impl AuthType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::None => "No Auth",
            AuthType::Basic => "Basic Auth",
            AuthType::Bearer => "Bearer Token",
            AuthType::ApiKey => "API Key",
        }
    }

    pub fn all() -> &'static [AuthType] {
        &[
            AuthType::None,
            AuthType::Basic,
            AuthType::Bearer,
            AuthType::ApiKey,
        ]
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    pub auth_type: AuthType,
    pub username: String,
    pub password: String,
    pub token: String,
    pub api_key_name: String,
    pub api_key_value: String,
    pub api_key_in_header: bool, // true = header, false = query param
}

impl AuthConfig {
    /// Generate Authorization header if applicable
    pub fn to_header(&self) -> Option<(String, String)> {
        match self.auth_type {
            AuthType::None => None,
            AuthType::Basic => {
                let credentials = format!("{}:{}", self.username, self.password);
                let encoded = base64_encode(&credentials);
                Some(("Authorization".to_string(), format!("Basic {}", encoded)))
            }
            AuthType::Bearer => {
                if self.token.is_empty() {
                    None
                } else {
                    Some((
                        "Authorization".to_string(),
                        format!("Bearer {}", self.token),
                    ))
                }
            }
            AuthType::ApiKey => {
                if self.api_key_in_header && !self.api_key_name.is_empty() {
                    Some((self.api_key_name.clone(), self.api_key_value.clone()))
                } else {
                    None
                }
            }
        }
    }
}

/// Simple base64 encoding (ASCII only for HTTP auth)
fn base64_encode(input: &str) -> String {
    let mut buf = Vec::new();
    {
        let mut encoder = Base64Encoder::new(&mut buf);
        encoder.write_all(input.as_bytes()).ok();
    }
    String::from_utf8(buf).unwrap_or_default()
}

struct Base64Encoder<'a> {
    output: &'a mut Vec<u8>,
    buffer: [u8; 3],
    buffer_len: usize,
}

impl<'a> Base64Encoder<'a> {
    fn new(output: &'a mut Vec<u8>) -> Self {
        Self {
            output,
            buffer: [0; 3],
            buffer_len: 0,
        }
    }
}

impl<'a> Write for Base64Encoder<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        for &byte in buf {
            self.buffer[self.buffer_len] = byte;
            self.buffer_len += 1;

            if self.buffer_len == 3 {
                self.output.push(CHARS[(self.buffer[0] >> 2) as usize]);
                self.output
                    .push(CHARS[((self.buffer[0] & 0x03) << 4 | self.buffer[1] >> 4) as usize]);
                self.output
                    .push(CHARS[((self.buffer[1] & 0x0f) << 2 | self.buffer[2] >> 6) as usize]);
                self.output.push(CHARS[(self.buffer[2] & 0x3f) as usize]);
                self.buffer_len = 0;
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        if self.buffer_len > 0 {
            self.output.push(CHARS[(self.buffer[0] >> 2) as usize]);
            if self.buffer_len == 1 {
                self.output
                    .push(CHARS[((self.buffer[0] & 0x03) << 4) as usize]);
                self.output.push(b'=');
                self.output.push(b'=');
            } else {
                self.output
                    .push(CHARS[((self.buffer[0] & 0x03) << 4 | self.buffer[1] >> 4) as usize]);
                self.output
                    .push(CHARS[((self.buffer[1] & 0x0f) << 2) as usize]);
                self.output.push(b'=');
            }
            self.buffer_len = 0;
        }
        Ok(())
    }
}

impl<'a> Drop for Base64Encoder<'a> {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

/// Authentication editor component
pub struct AuthEditor {
    auth_type: AuthType,
    // Basic auth
    username_input: Option<gpui::Entity<InputState>>,
    password_input: Option<gpui::Entity<InputState>>,
    // Bearer token
    token_input: Option<gpui::Entity<InputState>>,
    // API Key
    api_key_name_input: Option<gpui::Entity<InputState>>,
    api_key_value_input: Option<gpui::Entity<InputState>>,
    api_key_in_header: bool,

    focus_handle: FocusHandle,
}

impl AuthEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            auth_type: AuthType::None,
            username_input: None,
            password_input: None,
            token_input: None,
            api_key_name_input: None,
            api_key_value_input: None,
            api_key_in_header: true,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Initialize inputs lazily
    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.username_input.is_none() {
            self.username_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Username")));
            self.password_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Password")));
            self.token_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Bearer token")));
            self.api_key_name_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Key name")));
            self.api_key_value_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Key value")));
        }
    }

    /// Set auth type
    pub fn set_auth_type(&mut self, auth_type: AuthType, cx: &mut Context<Self>) {
        self.auth_type = auth_type;
        cx.notify();
    }

    /// Get current auth config
    pub fn get_config(&self, cx: &App) -> AuthConfig {
        AuthConfig {
            auth_type: self.auth_type,
            username: self
                .username_input
                .as_ref()
                .map(|i| i.read(cx).text().to_string())
                .unwrap_or_default(),
            password: self
                .password_input
                .as_ref()
                .map(|i| i.read(cx).text().to_string())
                .unwrap_or_default(),
            token: self
                .token_input
                .as_ref()
                .map(|i| i.read(cx).text().to_string())
                .unwrap_or_default(),
            api_key_name: self
                .api_key_name_input
                .as_ref()
                .map(|i| i.read(cx).text().to_string())
                .unwrap_or_default(),
            api_key_value: self
                .api_key_value_input
                .as_ref()
                .map(|i| i.read(cx).text().to_string())
                .unwrap_or_default(),
            api_key_in_header: self.api_key_in_header,
        }
    }
}

impl Focusable for AuthEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AuthEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_inputs(window, cx);

        let theme = Theme::dark();
        let this = cx.entity().clone();
        let selected_label: SharedString = self.auth_type.as_str().into();

        div()
            .id("auth-editor-container")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .bg(theme.colors.bg_tertiary)
            // Header with title and type selector
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(36.0))
                    .px(px(16.0))
                    .bg(theme.colors.bg_secondary)
                    .border_b_1()
                    .border_color(theme.colors.border_primary)
                    // Left: Title
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Authorization"),
                    )
                    // Right: Auth Type Dropdown
                    .child(
                        DropdownButton::new("auth-type-dropdown")
                            .button(
                                Button::new("auth-type-btn")
                                    .label(selected_label)
                                    .small()
                                    .ghost(),
                            )
                            .small()
                            .dropdown_menu({
                                let this = this.clone();
                                move |menu, _window, _cx| {
                                    let this = this.clone();
                                    AuthType::all().iter().fold(menu, |menu, auth_type| {
                                        let auth_type = *auth_type;
                                        let this = this.clone();
                                        menu.item(PopupMenuItem::new(auth_type.as_str()).on_click(
                                            move |_, _, cx| {
                                                this.update(cx, |editor, cx| {
                                                    editor.set_auth_type(auth_type, cx);
                                                });
                                            },
                                        ))
                                    })
                                }
                            }),
                    ),
            )
            // Content area
            .child(
                div()
                    .id("auth-content-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .p(px(16.0))
                    // Auth type specific fields
                    .when(self.auth_type == AuthType::Basic, |el| {
                        el.child(self.render_basic_auth(&theme))
                    })
                    .when(self.auth_type == AuthType::Bearer, |el| {
                        el.child(self.render_bearer_auth(&theme))
                    })
                    .when(self.auth_type == AuthType::ApiKey, |el| {
                        el.child(self.render_api_key_auth(&theme, this.clone()))
                    })
                    .when(self.auth_type == AuthType::None, |el| {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .flex_1()
                                .py(px(40.0))
                                .text_color(theme.colors.text_muted.opacity(0.5))
                                .text_size(px(12.0))
                                .child("This request does not use any authorization"),
                        )
                    }),
            )
    }
}

impl AuthEditor {
    fn render_basic_auth(&self, theme: &Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            // Username
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Username"),
                    )
                    .when_some(self.username_input.as_ref(), |el, input| {
                        el.child(
                            div()
                                .bg(theme.colors.bg_secondary)
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.colors.border_primary)
                                .child(Input::new(input).appearance(false).small()),
                        )
                    }),
            )
            // Password
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Password"),
                    )
                    .when_some(self.password_input.as_ref(), |el, input| {
                        el.child(
                            div()
                                .bg(theme.colors.bg_secondary)
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.colors.border_primary)
                                .child(Input::new(input).appearance(false).small()),
                        )
                    }),
            )
    }

    fn render_bearer_auth(&self, theme: &Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_color(theme.colors.text_muted)
                    .text_size(px(11.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Token"),
            )
            .when_some(self.token_input.as_ref(), |el, input| {
                el.child(
                    div()
                        .bg(theme.colors.bg_secondary)
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(theme.colors.border_primary)
                        .child(Input::new(input).appearance(false).small()),
                )
            })
    }

    fn render_api_key_auth(
        &self,
        theme: &Theme,
        this: gpui::Entity<AuthEditor>,
    ) -> impl IntoElement {
        let api_key_location: SharedString = if self.api_key_in_header {
            "Header".into()
        } else {
            "Query Param".into()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            // Add to selector
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Add to"),
                    )
                    .child(
                        DropdownButton::new("api-key-location-dropdown")
                            .button(
                                Button::new("api-key-location-btn")
                                    .label(api_key_location)
                                    .small()
                                    .ghost(),
                            )
                            .small()
                            .dropdown_menu({
                                let this = this.clone();
                                move |menu, _window, _cx| {
                                    let this_header = this.clone();
                                    let this_query = this.clone();
                                    menu.item(PopupMenuItem::new("Header").on_click(
                                        move |_, _, cx| {
                                            this_header.update(cx, |editor, cx| {
                                                editor.api_key_in_header = true;
                                                cx.notify();
                                            });
                                        },
                                    ))
                                    .item(
                                        PopupMenuItem::new("Query Param").on_click(
                                            move |_, _, cx| {
                                                this_query.update(cx, |editor, cx| {
                                                    editor.api_key_in_header = false;
                                                    cx.notify();
                                                });
                                            },
                                        ),
                                    )
                                }
                            }),
                    ),
            )
            // Key name
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Key"),
                    )
                    .when_some(self.api_key_name_input.as_ref(), |el, input| {
                        el.child(
                            div()
                                .bg(theme.colors.bg_secondary)
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.colors.border_primary)
                                .child(Input::new(input).appearance(false).small()),
                        )
                    }),
            )
            // Key value
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Value"),
                    )
                    .when_some(self.api_key_value_input.as_ref(), |el, input| {
                        el.child(
                            div()
                                .bg(theme.colors.bg_secondary)
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.colors.border_primary)
                                .child(Input::new(input).appearance(false).small()),
                        )
                    }),
            )
    }
}
