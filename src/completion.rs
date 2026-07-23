use anyhow::Result;
use gpui::{
    App, Context, Entity, Focusable, Global, IntoElement, ParentElement, RenderOnce, Styled,
    Subscription, Task, WeakEntity, Window, div,
};
use gpui_component::input::{
    CompletionProvider, Input, InputState, MoveDown, MoveUp, Rope, RopeExt,
};
use lsp_types::{
    CompletionContext as LspCompletionContext, CompletionItem, CompletionItemKind,
    CompletionResponse, CompletionTextEdit, Documentation, TextEdit,
};
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use uuid::Uuid;

use crate::entities::{EnvironmentScope, EnvironmentsEntity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionContext {
    Url,
    HeaderName,
    HeaderValue,
    QueryName,
    QueryValue,
    Auth,
    Body,
    FormName,
    FormValue,
    EnvironmentValue,
}

impl CompletionContext {
    fn supports_templates(self) -> bool {
        matches!(
            self,
            Self::Url
                | Self::HeaderName
                | Self::HeaderValue
                | Self::QueryName
                | Self::QueryValue
                | Self::Auth
                | Self::Body
                | Self::FormName
                | Self::FormValue
                | Self::EnvironmentValue
        )
    }
}

pub struct CompletionRequest {
    pub context: CompletionContext,
    pub text: String,
    pub cursor: usize,
    pub collection_id: Option<Uuid>,
}

pub struct CompletionSuggestion {
    pub label: String,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: String,
    pub replace_range: Range<usize>,
    pub kind: CompletionItemKind,
    pub filter_text: Option<String>,
}

pub trait CompletionSource {
    fn suggestions(&self, request: &CompletionRequest, cx: &App) -> Vec<CompletionSuggestion>;
}

#[derive(Clone)]
pub struct CompletionEngine {
    sources: Rc<Vec<Rc<dyn CompletionSource>>>,
    collection_id: Rc<Cell<Option<Uuid>>>,
}

impl CompletionEngine {
    pub fn for_environments(environments: gpui::Entity<EnvironmentsEntity>) -> Self {
        let collection_id = Rc::new(Cell::new(None));
        Self {
            sources: Rc::new(Vec::new()),
            collection_id,
        }
        .with_source(Rc::new(EnvironmentCompletionSource { environments }))
    }

    pub fn with_source(mut self, source: Rc<dyn CompletionSource>) -> Self {
        Rc::make_mut(&mut self.sources).push(source);
        self
    }

    pub fn set_collection_id(&self, collection_id: Option<Uuid>) {
        self.collection_id.set(collection_id);
    }

    pub fn configure_input(&self, mut input: InputState, context: CompletionContext) -> InputState {
        input.lsp.completion_provider = Some(Rc::new(ContextualCompletionProvider {
            sources: self.sources.clone(),
            context,
            collection_id: self.collection_id.clone(),
        }));
        input
    }
}

pub fn configure_completion(
    input: InputState,
    engine: Option<&CompletionEngine>,
    context: CompletionContext,
) -> InputState {
    if let Some(engine) = engine {
        engine.configure_input(input, context)
    } else {
        input
    }
}

#[derive(Default)]
struct CompletionNavigationRegistry {
    inputs: Vec<WeakEntity<InputState>>,
    _interceptor: Option<Subscription>,
}

impl Global for CompletionNavigationRegistry {}

/// Registers a pre-dispatch keyboard interceptor for completion-enabled inputs.
///
/// This avoids the single-line Up/Down action gap in gpui-component while
/// leaving the dependency untouched.
pub fn init_completion_navigation(cx: &mut App) {
    if cx.has_global::<CompletionNavigationRegistry>() {
        return;
    }
    cx.set_global(CompletionNavigationRegistry::default());
    let interceptor = cx.intercept_keystrokes(|event, window, cx| {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let action: Box<dyn gpui::Action> = match event.keystroke.key.as_str() {
            "up" => Box::new(MoveUp),
            "down" => Box::new(MoveDown),
            _ => return,
        };
        let inputs = cx.global::<CompletionNavigationRegistry>().inputs.clone();
        for input in inputs.into_iter().rev().filter_map(|input| input.upgrade()) {
            if !input.read(cx).focus_handle(cx).is_focused(window) {
                continue;
            }
            let handled = input.update(cx, |input, cx| {
                input.handle_action_for_context_menu(action, window, cx)
            });
            if handled {
                cx.stop_propagation();
            }
            break;
        }
    });
    cx.global_mut::<CompletionNavigationRegistry>()._interceptor = Some(interceptor);
}

/// Registers an input with Setu's completion key router while preserving the
/// normal gpui-component `Input` rendering and focus behavior.
#[derive(IntoElement)]
pub struct CompletionInput {
    input: Input,
    state: Entity<InputState>,
}

impl CompletionInput {
    pub fn new(state: &Entity<InputState>, input: Input) -> Self {
        Self {
            input,
            state: state.clone(),
        }
    }
}

impl RenderOnce for CompletionInput {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let registry = cx.global_mut::<CompletionNavigationRegistry>();
        registry.inputs.retain(|input| input.upgrade().is_some());
        if !registry.inputs.iter().any(|input| input == &self.state) {
            registry.inputs.push(self.state.downgrade());
        }
        div().w_full().child(self.input)
    }
}

struct ContextualCompletionProvider {
    sources: Rc<Vec<Rc<dyn CompletionSource>>>,
    context: CompletionContext,
    collection_id: Rc<Cell<Option<Uuid>>>,
}

impl CompletionProvider for ContextualCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: LspCompletionContext,
        _window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<CompletionResponse>> {
        let request = CompletionRequest {
            context: self.context,
            text: rope.to_string(),
            cursor: offset,
            collection_id: self.collection_id.get(),
        };
        let mut suggestions = self
            .sources
            .iter()
            .flat_map(|source| source.suggestions(&request, cx))
            .collect::<Vec<_>>();
        suggestions.sort_by(|left, right| left.label.cmp(&right.label));

        let items = suggestions
            .into_iter()
            .map(|suggestion| {
                let start = rope.offset_to_position(suggestion.replace_range.start);
                let end = rope.offset_to_position(suggestion.replace_range.end);
                let glyph = completion_kind_glyph(suggestion.kind);
                let highlighted_prefix = suggestion
                    .filter_text
                    .as_deref()
                    .filter(|query| !query.is_empty())
                    .map(|query| format!("{glyph}  {query}"))
                    .unwrap_or_else(|| glyph.to_string());
                CompletionItem {
                    label: format!("{glyph}  {}", suggestion.label),
                    kind: Some(suggestion.kind),
                    detail: suggestion.detail,
                    documentation: suggestion.documentation.map(Documentation::String),
                    filter_text: Some(highlighted_prefix),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: lsp_types::Range::new(start, end),
                        new_text: suggestion.insert_text,
                    })),
                    ..Default::default()
                }
            })
            .collect();
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        _new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        true
    }
}

fn completion_kind_glyph(kind: CompletionItemKind) -> &'static str {
    if kind == CompletionItemKind::VARIABLE {
        "x"
    } else if kind == CompletionItemKind::CLASS {
        "c"
    } else if kind == CompletionItemKind::FUNCTION || kind == CompletionItemKind::METHOD {
        "f"
    } else if kind == CompletionItemKind::PROPERTY || kind == CompletionItemKind::FIELD {
        "p"
    } else if kind == CompletionItemKind::MODULE {
        "m"
    } else if kind == CompletionItemKind::FILE {
        "f"
    } else if kind == CompletionItemKind::FOLDER {
        "d"
    } else if kind == CompletionItemKind::KEYWORD {
        "k"
    } else if kind == CompletionItemKind::SNIPPET {
        "s"
    } else if kind == CompletionItemKind::CONSTANT || kind == CompletionItemKind::VALUE {
        "v"
    } else {
        "·"
    }
}

struct EnvironmentCompletionSource {
    environments: gpui::Entity<EnvironmentsEntity>,
}

impl CompletionSource for EnvironmentCompletionSource {
    fn suggestions(&self, request: &CompletionRequest, cx: &App) -> Vec<CompletionSuggestion> {
        if !request.context.supports_templates() {
            return Vec::new();
        }
        let Some((start, query)) = template_query(&request.text, request.cursor) else {
            return Vec::new();
        };
        let normalized_query = query.to_ascii_lowercase();
        self.environments
            .read(cx)
            .effective_variable_completions(request.collection_id)
            .into_iter()
            .filter(|variable| {
                variable
                    .key
                    .to_ascii_lowercase()
                    .starts_with(&normalized_query)
            })
            .map(|variable| {
                let origin = match variable.scope {
                    EnvironmentScope::Global => "Inherited from Global Variables",
                    EnvironmentScope::Workspace => "Inherited from Workspace Variables",
                    EnvironmentScope::Project(_) => "Inherited from Project Variables",
                };
                let documentation = if variable.secret {
                    format!("{origin}\n\nSecret variable")
                } else {
                    origin.to_string()
                };
                CompletionSuggestion {
                    label: variable.key.clone(),
                    detail: Some("variable".to_string()),
                    documentation: Some(documentation),
                    insert_text: format!("{{{{{}}}}}", variable.key),
                    replace_range: start..request.cursor,
                    kind: CompletionItemKind::VARIABLE,
                    filter_text: Some(query.clone()),
                }
            })
            .collect()
    }
}

fn template_query(text: &str, cursor: usize) -> Option<(usize, String)> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let before_cursor = &text[..cursor];
    let start = before_cursor.rfind("{{")?;
    let query = &before_cursor[start + 2..];
    if query.contains("}}") || query.chars().any(char::is_whitespace) {
        return None;
    }
    Some((start, query.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_open_template_at_cursor() {
        assert_eq!(
            template_query("https://{{base_u", 16),
            Some((8, "base_u".to_string()))
        );
    }

    #[test]
    fn ignores_closed_or_whitespace_templates() {
        assert_eq!(template_query("{{done}}", 8), None);
        assert_eq!(template_query("{{not valid", 11), None);
    }

    #[test]
    fn maps_completion_kinds_to_compact_glyphs() {
        assert_eq!(completion_kind_glyph(CompletionItemKind::VARIABLE), "x");
        assert_eq!(completion_kind_glyph(CompletionItemKind::CLASS), "c");
        assert_eq!(completion_kind_glyph(CompletionItemKind::FUNCTION), "f");
    }
}
