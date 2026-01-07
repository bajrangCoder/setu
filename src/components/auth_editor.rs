use std::io::Write;

use gpui::prelude::*;
use gpui::{
    div, px, App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Styled, Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::select::{Select, SelectEvent, SelectItem, SelectState};
use gpui_component::Sizable;

use gpui_component::ActiveTheme;

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

/// Implement SelectItem for AuthType
impl SelectItem for AuthType {
    type Value = AuthType;

    fn title(&self) -> SharedString {
        self.as_str().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

/// API Key location enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApiKeyLocation {
    #[default]
    Header,
    QueryParam,
}

impl ApiKeyLocation {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiKeyLocation::Header => "Header",
            ApiKeyLocation::QueryParam => "Query Param",
        }
    }

    pub fn all() -> &'static [ApiKeyLocation] {
        &[ApiKeyLocation::Header, ApiKeyLocation::QueryParam]
    }
}

/// Implement SelectItem for ApiKeyLocation
impl SelectItem for ApiKeyLocation {
    type Value = ApiKeyLocation;

    fn title(&self) -> SharedString {
        self.as_str().into()
    }

    fn value(&self) -> &Self::Value {
        self
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
    auth_type_select: Entity<SelectState<Vec<AuthType>>>,
    // Basic auth
    username_input: Option<gpui::Entity<InputState>>,
    password_input: Option<gpui::Entity<InputState>>,
    // Bearer token
    token_input: Option<gpui::Entity<InputState>>,
    // API Key
    api_key_name_input: Option<gpui::Entity<InputState>>,
    api_key_value_input: Option<gpui::Entity<InputState>>,
    api_key_location: ApiKeyLocation,
    api_key_location_select: Entity<SelectState<Vec<ApiKeyLocation>>>,

    focus_handle: FocusHandle,
}

impl AuthEditor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Create auth type select state
        let auth_type_items: Vec<AuthType> = AuthType::all().to_vec();
        let auth_type_select = cx.new(|cx| {
            SelectState::new(
                auth_type_items,
                Some(gpui_component::IndexPath::new(0)),
                window,
                cx,
            )
        });

        // Subscribe to auth type selection changes
        cx.subscribe(
            &auth_type_select,
            |this, _, event: &SelectEvent<Vec<AuthType>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.auth_type = *value;
                    cx.notify();
                }
            },
        )
        .detach();

        // Create API key location select state
        let api_key_location_items: Vec<ApiKeyLocation> = ApiKeyLocation::all().to_vec();
        let api_key_location_select = cx.new(|cx| {
            SelectState::new(
                api_key_location_items,
                Some(gpui_component::IndexPath::new(0)),
                window,
                cx,
            )
        });

        // Subscribe to API key location selection changes
        cx.subscribe(
            &api_key_location_select,
            |this, _, event: &SelectEvent<Vec<ApiKeyLocation>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.api_key_location = *value;
                    cx.notify();
                }
            },
        )
        .detach();

        Self {
            auth_type: AuthType::None,
            auth_type_select,
            username_input: None,
            password_input: None,
            token_input: None,
            api_key_name_input: None,
            api_key_value_input: None,
            api_key_location: ApiKeyLocation::Header,
            api_key_location_select,
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
    pub fn set_auth_type(
        &mut self,
        auth_type: AuthType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.auth_type = auth_type;
        self.auth_type_select.update(cx, |state, cx| {
            state.set_selected_value(&auth_type, window, cx);
        });
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
            api_key_in_header: self.api_key_location == ApiKeyLocation::Header,
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

        let theme = cx.theme();

        div()
            .id("auth-editor-container")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .bg(theme.muted)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(36.0))
                    .px(px(16.0))
                    .bg(theme.secondary)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Authorization"),
                    )
                    .child(
                        div().flex_shrink_0().child(
                            Select::new(&self.auth_type_select)
                                .small()
                                .menu_width(px(140.0)),
                        ),
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
                        el.child(self.render_api_key_auth(&theme))
                    })
                    .when(self.auth_type == AuthType::None, |el| {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .flex_1()
                                .py(px(40.0))
                                .text_color(theme.muted_foreground.opacity(0.5))
                                .text_size(px(12.0))
                                .child("This request does not use any authorization"),
                        )
                    }),
            )
    }
}

impl AuthEditor {
    fn render_basic_auth(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
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
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Username"),
                    )
                    .when_some(self.username_input.as_ref(), |el, input| {
                        el.child(
                            div()
                                .bg(theme.secondary)
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.border)
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
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Password"),
                    )
                    .when_some(self.password_input.as_ref(), |el, input| {
                        el.child(
                            div()
                                .bg(theme.secondary)
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.border)
                                .child(Input::new(input).appearance(false).small()),
                        )
                    }),
            )
    }

    fn render_bearer_auth(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .text_size(px(11.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Token"),
            )
            .when_some(self.token_input.as_ref(), |el, input| {
                el.child(
                    div()
                        .bg(theme.secondary)
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(theme.border)
                        .child(Input::new(input).appearance(false).small()),
                )
            })
    }

    fn render_api_key_auth(&self, theme: &gpui_component::theme::ThemeColor) -> impl IntoElement {
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
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Add to"),
                    )
                    .child(
                        Select::new(&self.api_key_location_select)
                            .small()
                            .menu_width(px(120.0)),
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
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Key"),
                    )
                    .when_some(self.api_key_name_input.as_ref(), |el, input| {
                        el.child(
                            div()
                                .bg(theme.secondary)
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.border)
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
                            .text_color(theme.muted_foreground)
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Value"),
                    )
                    .when_some(self.api_key_value_input.as_ref(), |el, input| {
                        el.child(
                            div()
                                .bg(theme.secondary)
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.border)
                                .child(Input::new(input).appearance(false).small()),
                        )
                    }),
            )
    }
}
