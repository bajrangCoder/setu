use gpui::{Context, EventEmitter, Hsla, hsla};
use gpui_component::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

use crate::entities::{Header, MultipartField, RequestBody, default_workspace_id};
use crate::utils::{DebouncedJsonWriter, shared_tokio_runtime};

const ENVIRONMENTS_STORAGE_VERSION: u32 = 3;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(350);
const MAX_INTERPOLATION_DEPTH: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", content = "collection_id", rename_all = "snake_case")]
pub enum EnvironmentScope {
    Global,
    Workspace,
    Project(Uuid),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentColor {
    #[default]
    Teal,
    Blue,
    Violet,
    Amber,
    Rose,
    Slate,
    Custom(String),
}

impl EnvironmentColor {
    pub const ALL: [Self; 6] = [
        Self::Teal,
        Self::Blue,
        Self::Violet,
        Self::Amber,
        Self::Rose,
        Self::Slate,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Teal => "Teal",
            Self::Blue => "Blue",
            Self::Violet => "Violet",
            Self::Amber => "Amber",
            Self::Rose => "Rose",
            Self::Slate => "Slate",
            Self::Custom(_) => "Custom",
        }
    }

    pub fn accent(&self) -> Hsla {
        match self {
            Self::Teal => hsla(165.0 / 360.0, 0.80, 0.48, 1.0),
            Self::Blue => hsla(205.0 / 360.0, 0.82, 0.58, 1.0),
            Self::Violet => hsla(270.0 / 360.0, 0.76, 0.66, 1.0),
            Self::Amber => hsla(38.0 / 360.0, 0.92, 0.58, 1.0),
            Self::Rose => hsla(348.0 / 360.0, 0.78, 0.62, 1.0),
            Self::Slate => hsla(220.0 / 360.0, 0.12, 0.62, 1.0),
            Self::Custom(hex) => {
                Hsla::parse_hex(hex).unwrap_or_else(|_| EnvironmentColor::default().accent())
            }
        }
    }

    pub fn custom(color: Hsla) -> Self {
        let hex = color.to_hex();
        Self::ALL
            .into_iter()
            .find(|preset| preset.accent().to_hex() == hex)
            .unwrap_or(Self::Custom(hex))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentVariable {
    pub id: Uuid,
    pub key: String,
    pub value: String,
    pub enabled: bool,
    pub secret: bool,
}

impl Default for EnvironmentVariable {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            key: String::new(),
            value: String::new(),
            enabled: true,
            secret: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: Uuid,
    pub name: String,
    pub scope: EnvironmentScope,
    #[serde(default)]
    pub color: EnvironmentColor,
    #[serde(default)]
    pub variables: Vec<EnvironmentVariable>,
}

impl Environment {
    pub fn new(name: impl Into<String>, scope: EnvironmentScope) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            scope,
            color: EnvironmentColor::default(),
            variables: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct EnvironmentWorkspaceStore {
    environments: Vec<Environment>,
    active_workspace_environment: Option<Uuid>,
    active_project_environments: HashMap<Uuid, Uuid>,
}

impl Default for EnvironmentWorkspaceStore {
    fn default() -> Self {
        Self {
            environments: Vec::new(),
            active_workspace_environment: None,
            active_project_environments: HashMap::new(),
        }
    }
}

impl EnvironmentWorkspaceStore {
    fn starter() -> Self {
        let environment = Environment::new("Development", EnvironmentScope::Workspace);
        Self {
            active_workspace_environment: Some(environment.id),
            environments: vec![environment],
            ..Self::default()
        }
    }

    fn validated(mut self) -> Self {
        let ids: HashSet<_> = self
            .environments
            .iter()
            .map(|environment| environment.id)
            .collect();
        if self
            .active_workspace_environment
            .is_some_and(|id| !ids.contains(&id))
        {
            self.active_workspace_environment = None;
        }
        self.active_project_environments
            .retain(|_, environment_id| ids.contains(environment_id));
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct EnvironmentsStore {
    version: u32,
    global_environments: Vec<Environment>,
    active_global_environment: Option<Uuid>,
    workspaces: HashMap<Uuid, EnvironmentWorkspaceStore>,
}

impl Default for EnvironmentsStore {
    fn default() -> Self {
        Self {
            version: ENVIRONMENTS_STORAGE_VERSION,
            global_environments: Vec::new(),
            active_global_environment: None,
            workspaces: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct LegacyEnvironmentsStore {
    version: u32,
    environments: Vec<Environment>,
    active_workspace_environment: Option<Uuid>,
    active_project_environments: HashMap<Uuid, Uuid>,
}

impl Default for LegacyEnvironmentsStore {
    fn default() -> Self {
        Self {
            version: 1,
            environments: Vec::new(),
            active_workspace_environment: None,
            active_project_environments: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum EnvironmentEvent {
    Changed,
    ActiveChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpolationError {
    pub unresolved: Vec<String>,
    pub cycles: Vec<String>,
}

impl InterpolationError {
    pub fn user_message(&self) -> String {
        let mut parts = Vec::new();
        if !self.unresolved.is_empty() {
            parts.push(format!(
                "Missing environment variable{}: {}",
                if self.unresolved.len() == 1 { "" } else { "s" },
                self.unresolved.join(", ")
            ));
        }
        if !self.cycles.is_empty() {
            parts.push(format!(
                "Circular environment variable reference{}: {}",
                if self.cycles.len() == 1 { "" } else { "s" },
                self.cycles.join(", ")
            ));
        }
        parts.join(". ")
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedRequestParts {
    pub url: String,
    pub headers: Vec<Header>,
    pub body: RequestBody,
}

pub struct EnvironmentsEntity {
    environments: Vec<Environment>,
    active_global_environment: Option<Uuid>,
    active_workspace_environment: Option<Uuid>,
    active_project_environments: HashMap<Uuid, Uuid>,
    active_workspace_id: Uuid,
    workspace_environments: HashMap<Uuid, EnvironmentWorkspaceStore>,
    revision: u64,
    persistor: Option<DebouncedJsonWriter<EnvironmentsStore>>,
}

impl EnvironmentsEntity {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::new_for_workspace(default_workspace_id())
    }

    pub fn new_for_workspace(active_workspace_id: Uuid) -> Self {
        let store = EnvironmentWorkspaceStore::starter();
        Self {
            environments: store.environments,
            active_global_environment: None,
            active_workspace_environment: store.active_workspace_environment,
            active_project_environments: store.active_project_environments,
            active_workspace_id,
            workspace_environments: HashMap::new(),
            revision: 0,
            persistor: storage_path()
                .map(|path| DebouncedJsonWriter::new("environments", path, SAVE_DEBOUNCE)),
        }
    }

    pub fn spawn_storage_load()
    -> tokio::sync::oneshot::Receiver<Result<Option<EnvironmentsStore>, String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let path = storage_path();
        shared_tokio_runtime().spawn(async move {
            let result = async {
                let Some(path) = path else {
                    return Ok(None);
                };
                match tokio::fs::read_to_string(path).await {
                    Ok(contents) => deserialize_environments_store(&contents)
                        .map(|store| Some(store.0))
                        .map_err(|error| error.to_string()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(error.to_string()),
                }
            }
            .await;
            let _ = tx.send(result);
        });
        rx
    }

    pub fn apply_storage_load(
        &mut self,
        result: Result<Option<EnvironmentsStore>, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Some(mut store)) => {
                let active = store
                    .workspaces
                    .remove(&self.active_workspace_id)
                    .unwrap_or_else(EnvironmentWorkspaceStore::starter)
                    .validated();
                self.environments = store.global_environments;
                self.environments.extend(active.environments);
                self.active_global_environment = store
                    .active_global_environment
                    .filter(|id| self.get(*id).is_some());
                self.active_workspace_environment = active.active_workspace_environment;
                self.active_project_environments = active.active_project_environments;
                self.workspace_environments = store.workspaces;
                self.save_to_file();
            }
            Ok(None) => self.save_to_file(),
            Err(error) => log::error!("Failed to load environments: {error}"),
        }
        self.bump_revision();
        cx.emit(EnvironmentEvent::Changed);
        cx.notify();
    }

    pub fn get(&self, id: Uuid) -> Option<&Environment> {
        self.environments
            .iter()
            .find(|environment| environment.id == id)
    }

    pub fn environments(&self) -> &[Environment] {
        &self.environments
    }

    pub fn is_active(&self, environment_id: Uuid) -> bool {
        let Some(environment) = self.get(environment_id) else {
            return false;
        };
        match environment.scope {
            EnvironmentScope::Global => self.active_global_environment == Some(environment_id),
            EnvironmentScope::Workspace => {
                self.active_workspace_environment == Some(environment_id)
            }
            EnvironmentScope::Project(project_id) => {
                self.active_project_environments.get(&project_id).copied() == Some(environment_id)
            }
        }
    }

    pub fn available_for(&self, collection_id: Option<Uuid>) -> Vec<&Environment> {
        self.environments
            .iter()
            .filter(|environment| match environment.scope {
                EnvironmentScope::Global => true,
                EnvironmentScope::Workspace => true,
                EnvironmentScope::Project(project_id) => Some(project_id) == collection_id,
            })
            .collect()
    }

    pub fn active_environment_id(&self, collection_id: Option<Uuid>) -> Option<Uuid> {
        collection_id
            .and_then(|id| self.active_project_environments.get(&id).copied())
            .or(self.active_workspace_environment)
            .or(self.active_global_environment)
            .filter(|id| self.get(*id).is_some())
    }

    pub fn active_global_environment_id(&self) -> Option<Uuid> {
        self.active_global_environment
            .filter(|id| self.get(*id).is_some())
    }

    pub fn active_workspace_environment_id(&self) -> Option<Uuid> {
        self.active_workspace_environment
            .filter(|id| self.get(*id).is_some())
    }

    pub fn active_project_environment_id(&self, project_id: Uuid) -> Option<Uuid> {
        self.active_project_environments
            .get(&project_id)
            .copied()
            .filter(|id| self.get(*id).is_some())
    }

    pub fn active_environment(&self, collection_id: Option<Uuid>) -> Option<&Environment> {
        self.active_environment_id(collection_id)
            .and_then(|id| self.get(id))
    }

    pub fn set_active(
        &mut self,
        collection_id: Option<Uuid>,
        environment_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        match (collection_id, environment_id.and_then(|id| self.get(id))) {
            (_, Some(environment)) if environment.scope == EnvironmentScope::Global => {
                self.active_global_environment = Some(environment.id);
            }
            (Some(project_id), Some(environment))
                if environment.scope == EnvironmentScope::Project(project_id) =>
            {
                self.active_project_environments
                    .insert(project_id, environment.id);
            }
            (Some(project_id), Some(environment))
                if environment.scope == EnvironmentScope::Workspace =>
            {
                self.active_workspace_environment = Some(environment.id);
                self.active_project_environments.remove(&project_id);
            }
            (Some(project_id), None) => {
                self.active_project_environments.remove(&project_id);
            }
            (None, Some(environment)) if environment.scope == EnvironmentScope::Workspace => {
                self.active_workspace_environment = Some(environment.id);
            }
            (None, None) => {
                self.active_global_environment = None;
                self.active_workspace_environment = None;
            }
            _ => return,
        }
        self.bump_revision();
        self.save_to_file();
        cx.emit(EnvironmentEvent::ActiveChanged);
        cx.notify();
    }

    pub fn create_environment(
        &mut self,
        name: impl Into<String>,
        scope: EnvironmentScope,
        cx: &mut Context<Self>,
    ) -> Uuid {
        let environment = Environment::new(name, scope);
        let id = environment.id;
        self.environments.push(environment);
        match scope {
            EnvironmentScope::Global => self.active_global_environment = Some(id),
            EnvironmentScope::Workspace => self.active_workspace_environment = Some(id),
            EnvironmentScope::Project(project_id) => {
                self.active_project_environments.insert(project_id, id);
            }
        }
        self.changed(EnvironmentEvent::ActiveChanged, cx);
        id
    }

    pub fn import_environment(
        &mut self,
        name: impl Into<String>,
        scope: EnvironmentScope,
        variables: Vec<EnvironmentVariable>,
        cx: &mut Context<Self>,
    ) -> Uuid {
        let mut environment = Environment::new(name, scope);
        environment.variables = variables;
        let id = environment.id;
        self.environments.push(environment);
        match scope {
            EnvironmentScope::Global => self.active_global_environment = Some(id),
            EnvironmentScope::Workspace => self.active_workspace_environment = Some(id),
            EnvironmentScope::Project(project_id) => {
                self.active_project_environments.insert(project_id, id);
            }
        }
        self.changed(EnvironmentEvent::ActiveChanged, cx);
        id
    }

    pub fn set_active_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if workspace_id == self.active_workspace_id {
            return;
        }
        let (global_environments, workspace_environments): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.environments)
                .into_iter()
                .partition(|environment| environment.scope == EnvironmentScope::Global);
        let previous = EnvironmentWorkspaceStore {
            environments: workspace_environments,
            active_workspace_environment: self.active_workspace_environment.take(),
            active_project_environments: std::mem::take(&mut self.active_project_environments),
        };
        self.workspace_environments
            .insert(self.active_workspace_id, previous);
        self.active_workspace_id = workspace_id;
        let active = self
            .workspace_environments
            .remove(&workspace_id)
            .unwrap_or_else(EnvironmentWorkspaceStore::starter)
            .validated();
        self.environments = global_environments;
        self.environments.extend(active.environments);
        self.active_workspace_environment = active.active_workspace_environment;
        self.active_project_environments = active.active_project_environments;
        self.changed(EnvironmentEvent::ActiveChanged, cx);
    }

    pub fn remove_workspace(&mut self, workspace_id: Uuid) {
        if workspace_id != self.active_workspace_id {
            self.workspace_environments.remove(&workspace_id);
            self.save_to_file();
        }
    }

    pub fn duplicate_environment(&mut self, id: Uuid, cx: &mut Context<Self>) -> Option<Uuid> {
        let source = self.get(id)?.clone();
        let scope = source.scope;
        let duplicate = duplicate_environment_data(source);
        let duplicate_id = duplicate.id;
        self.environments.push(duplicate);
        match scope {
            EnvironmentScope::Global => {
                self.active_global_environment = Some(duplicate_id);
            }
            EnvironmentScope::Workspace => {
                self.active_workspace_environment = Some(duplicate_id);
            }
            EnvironmentScope::Project(project_id) => {
                self.active_project_environments
                    .insert(project_id, duplicate_id);
            }
        }
        self.changed(EnvironmentEvent::ActiveChanged, cx);
        Some(duplicate_id)
    }

    pub fn remove_environment(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(index) = self
            .environments
            .iter()
            .position(|environment| environment.id == id)
        else {
            return;
        };
        self.environments.remove(index);
        if self.active_global_environment == Some(id) {
            self.active_global_environment = self
                .environments
                .iter()
                .find(|environment| environment.scope == EnvironmentScope::Global)
                .map(|environment| environment.id);
        }
        if self.active_workspace_environment == Some(id) {
            self.active_workspace_environment = self
                .environments
                .iter()
                .find(|environment| environment.scope == EnvironmentScope::Workspace)
                .map(|environment| environment.id);
        }
        self.active_project_environments
            .retain(|_, environment_id| *environment_id != id);
        self.changed(EnvironmentEvent::Changed, cx);
    }

    pub fn rename_environment(&mut self, id: Uuid, name: String, cx: &mut Context<Self>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let Some(environment) = self.get_mut(id) else {
            return;
        };
        environment.name = name.to_string();
        self.changed(EnvironmentEvent::Changed, cx);
    }

    pub fn set_environment_color(
        &mut self,
        id: Uuid,
        color: EnvironmentColor,
        cx: &mut Context<Self>,
    ) {
        let Some(environment) = self.get_mut(id) else {
            return;
        };
        environment.color = color;
        self.changed(EnvironmentEvent::Changed, cx);
    }

    pub fn remove_project_environments(&mut self, project_id: Uuid, cx: &mut Context<Self>) {
        let previous_len = self.environments.len();
        self.environments
            .retain(|environment| environment.scope != EnvironmentScope::Project(project_id));
        self.active_project_environments.remove(&project_id);
        if self.environments.len() != previous_len {
            self.changed(EnvironmentEvent::Changed, cx);
        }
    }

    pub fn add_variable(&mut self, environment_id: Uuid, cx: &mut Context<Self>) -> Option<Uuid> {
        let variable = EnvironmentVariable::default();
        let id = variable.id;
        self.get_mut(environment_id)?.variables.push(variable);
        self.changed(EnvironmentEvent::Changed, cx);
        Some(id)
    }

    pub fn update_variable(
        &mut self,
        environment_id: Uuid,
        variable_id: Uuid,
        key: Option<String>,
        value: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(variable) = self.variable_mut(environment_id, variable_id) else {
            return;
        };
        if let Some(key) = key {
            variable.key = key;
        }
        if let Some(value) = value {
            variable.value = value;
        }
        self.changed(EnvironmentEvent::Changed, cx);
    }

    pub fn toggle_variable(
        &mut self,
        environment_id: Uuid,
        variable_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        let Some(variable) = self.variable_mut(environment_id, variable_id) else {
            return;
        };
        variable.enabled = !variable.enabled;
        self.changed(EnvironmentEvent::Changed, cx);
    }

    pub fn toggle_secret(
        &mut self,
        environment_id: Uuid,
        variable_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        let Some(variable) = self.variable_mut(environment_id, variable_id) else {
            return;
        };
        variable.secret = !variable.secret;
        self.changed(EnvironmentEvent::Changed, cx);
    }

    pub fn duplicate_variable(
        &mut self,
        environment_id: Uuid,
        variable_id: Uuid,
        cx: &mut Context<Self>,
    ) -> Option<Uuid> {
        let source = self
            .get(environment_id)?
            .variables
            .iter()
            .find(|variable| variable.id == variable_id)?
            .clone();
        let duplicate_id = Uuid::new_v4();
        let duplicate = EnvironmentVariable {
            id: duplicate_id,
            key: if source.key.is_empty() {
                String::new()
            } else {
                format!("{}_copy", source.key)
            },
            value: if source.secret {
                String::new()
            } else {
                source.value
            },
            ..source
        };
        self.get_mut(environment_id)?.variables.push(duplicate);
        self.changed(EnvironmentEvent::Changed, cx);
        Some(duplicate_id)
    }

    pub fn remove_variable(
        &mut self,
        environment_id: Uuid,
        variable_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        let Some(environment) = self.get_mut(environment_id) else {
            return;
        };
        environment
            .variables
            .retain(|variable| variable.id != variable_id);
        self.changed(EnvironmentEvent::Changed, cx);
    }

    pub fn resolve_request(
        &self,
        collection_id: Option<Uuid>,
        url: &str,
        headers: &[Header],
        body: &RequestBody,
    ) -> Result<ResolvedRequestParts, InterpolationError> {
        let values = self.effective_values(collection_id);
        let mut resolver = Resolver::new(values);
        let url = resolver.resolve(url);
        let headers = headers
            .iter()
            .map(|header| Header {
                key: resolver.resolve(&header.key),
                value: resolver.resolve(&header.value),
                enabled: header.enabled,
            })
            .collect();
        let body = match body {
            RequestBody::None => RequestBody::None,
            RequestBody::Text(value) => RequestBody::Text(resolver.resolve(value)),
            RequestBody::Json(value) => RequestBody::Json(resolver.resolve(value)),
            RequestBody::FormData(fields) => RequestBody::FormData(
                fields
                    .iter()
                    .map(|(key, value)| (resolver.resolve(key), resolver.resolve(value)))
                    .collect(),
            ),
            RequestBody::MultipartFormData(fields) => RequestBody::MultipartFormData(
                fields
                    .iter()
                    .map(|field| MultipartField {
                        key: resolver.resolve(&field.key),
                        value: resolver.resolve(&field.value),
                        file_path: field.file_path.as_ref().map(|path| resolver.resolve(path)),
                    })
                    .collect(),
            ),
        };

        resolver.finish()?;
        Ok(ResolvedRequestParts { url, headers, body })
    }

    fn effective_values(&self, collection_id: Option<Uuid>) -> HashMap<String, String> {
        let mut values = HashMap::new();
        if let Some(global) = self.active_global_environment.and_then(|id| self.get(id)) {
            insert_environment_values(global, &mut values);
        }
        if let Some(workspace) = self
            .active_workspace_environment
            .and_then(|id| self.get(id))
        {
            insert_environment_values(workspace, &mut values);
        }
        if let Some(project) = collection_id
            .and_then(|id| self.active_project_environments.get(&id))
            .and_then(|id| self.get(*id))
        {
            insert_environment_values(project, &mut values);
        }
        values
    }

    fn get_mut(&mut self, id: Uuid) -> Option<&mut Environment> {
        self.environments
            .iter_mut()
            .find(|environment| environment.id == id)
    }

    fn variable_mut(
        &mut self,
        environment_id: Uuid,
        variable_id: Uuid,
    ) -> Option<&mut EnvironmentVariable> {
        self.get_mut(environment_id)?
            .variables
            .iter_mut()
            .find(|variable| variable.id == variable_id)
    }

    fn changed(&mut self, event: EnvironmentEvent, cx: &mut Context<Self>) {
        self.bump_revision();
        self.save_to_file();
        cx.emit(event);
        cx.notify();
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn save_to_file(&self) {
        if let Some(persistor) = &self.persistor {
            let mut workspaces = self.workspace_environments.clone();
            let global_environments = self
                .environments
                .iter()
                .filter(|environment| environment.scope == EnvironmentScope::Global)
                .cloned()
                .collect();
            workspaces.insert(
                self.active_workspace_id,
                EnvironmentWorkspaceStore {
                    environments: self
                        .environments
                        .iter()
                        .filter(|environment| environment.scope != EnvironmentScope::Global)
                        .cloned()
                        .collect(),
                    active_workspace_environment: self.active_workspace_environment,
                    active_project_environments: self.active_project_environments.clone(),
                },
            );
            persistor.schedule_save(EnvironmentsStore {
                version: ENVIRONMENTS_STORAGE_VERSION,
                global_environments,
                active_global_environment: self.active_global_environment,
                workspaces,
            });
        }
    }
}

impl EventEmitter<EnvironmentEvent> for EnvironmentsEntity {}

fn storage_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|mut path| {
        path.push("setu");
        path.push("environments.json");
        path
    })
}

fn deserialize_environments_store(
    contents: &str,
) -> Result<(EnvironmentsStore, bool), serde_json::Error> {
    if let Ok(mut store) = serde_json::from_str::<EnvironmentsStore>(contents)
        && matches!(store.version, 2 | ENVIRONMENTS_STORAGE_VERSION)
    {
        let migrated = store.version != ENVIRONMENTS_STORAGE_VERSION;
        store.version = ENVIRONMENTS_STORAGE_VERSION;
        store
            .global_environments
            .retain(|environment| environment.scope == EnvironmentScope::Global);
        let global_ids: HashSet<_> = store
            .global_environments
            .iter()
            .map(|environment| environment.id)
            .collect();
        if store
            .active_global_environment
            .is_some_and(|id| !global_ids.contains(&id))
        {
            store.active_global_environment = None;
        }
        store.workspaces = store
            .workspaces
            .into_iter()
            .map(|(workspace_id, workspace)| (workspace_id, workspace.validated()))
            .collect();
        return Ok((store, migrated));
    }

    let legacy = serde_json::from_str::<LegacyEnvironmentsStore>(contents)?;
    let workspace = EnvironmentWorkspaceStore {
        environments: legacy.environments,
        active_workspace_environment: legacy.active_workspace_environment,
        active_project_environments: legacy.active_project_environments,
    }
    .validated();
    Ok((
        EnvironmentsStore {
            version: ENVIRONMENTS_STORAGE_VERSION,
            global_environments: Vec::new(),
            active_global_environment: None,
            workspaces: HashMap::from([(default_workspace_id(), workspace)]),
        },
        true,
    ))
}

fn insert_environment_values(environment: &Environment, values: &mut HashMap<String, String>) {
    for variable in &environment.variables {
        let key = variable.key.trim();
        if variable.enabled && !key.is_empty() {
            values.insert(key.to_string(), variable.value.clone());
        }
    }
}

fn duplicate_environment_data(source: Environment) -> Environment {
    Environment {
        id: Uuid::new_v4(),
        name: format!("{} Copy", source.name),
        scope: source.scope,
        color: source.color,
        variables: source
            .variables
            .into_iter()
            .map(|variable| EnvironmentVariable {
                id: Uuid::new_v4(),
                value: if variable.secret {
                    String::new()
                } else {
                    variable.value
                },
                ..variable
            })
            .collect(),
    }
}

struct Resolver {
    values: HashMap<String, String>,
    cache: HashMap<String, String>,
    unresolved: HashSet<String>,
    cycles: HashSet<String>,
}

impl Resolver {
    fn new(values: HashMap<String, String>) -> Self {
        Self {
            values,
            cache: HashMap::new(),
            unresolved: HashSet::new(),
            cycles: HashSet::new(),
        }
    }

    fn resolve(&mut self, template: &str) -> String {
        self.resolve_template(template, &mut Vec::new(), 0)
    }

    fn resolve_template(
        &mut self,
        template: &str,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> String {
        if depth >= MAX_INTERPOLATION_DEPTH {
            if let Some(key) = stack.last() {
                self.cycles.insert(key.clone());
            }
            return template.to_string();
        }

        let mut output = String::with_capacity(template.len());
        let mut cursor = 0;
        while let Some(relative_start) = template[cursor..].find("{{") {
            let start = cursor + relative_start;
            if start > 0 && template.as_bytes()[start - 1] == b'\\' {
                output.push_str(&template[cursor..start - 1]);
                output.push_str("{{");
                cursor = start + 2;
                continue;
            }
            output.push_str(&template[cursor..start]);
            let Some(relative_end) = template[start + 2..].find("}}") else {
                output.push_str(&template[start..]);
                cursor = template.len();
                break;
            };
            let end = start + 2 + relative_end;
            let key = template[start + 2..end].trim();
            if key.is_empty() {
                output.push_str(&template[start..end + 2]);
            } else {
                output.push_str(&self.resolve_key(key, stack, depth + 1));
            }
            cursor = end + 2;
        }
        output.push_str(&template[cursor..]);
        output
    }

    fn resolve_key(&mut self, key: &str, stack: &mut Vec<String>, depth: usize) -> String {
        if let Some(value) = self.cache.get(key) {
            return value.clone();
        }
        if stack.iter().any(|entry| entry == key) {
            self.cycles.insert(key.to_string());
            return format!("{{{{{key}}}}}");
        }
        let Some(raw_value) = self.values.get(key).cloned() else {
            self.unresolved.insert(key.to_string());
            return format!("{{{{{key}}}}}");
        };

        stack.push(key.to_string());
        let value = self.resolve_template(&raw_value, stack, depth);
        stack.pop();
        self.cache.insert(key.to_string(), value.clone());
        value
    }

    fn finish(self) -> Result<(), InterpolationError> {
        if self.unresolved.is_empty() && self.cycles.is_empty() {
            return Ok(());
        }
        let mut unresolved: Vec<_> = self.unresolved.into_iter().collect();
        unresolved.sort();
        let mut cycles: Vec<_> = self.cycles.into_iter().collect();
        cycles.sort();
        Err(InterpolationError { unresolved, cycles })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity_with_variables(
        workspace: &[(&str, &str)],
        project: Option<(Uuid, &[(&str, &str)])>,
    ) -> EnvironmentsEntity {
        let mut workspace_environment =
            Environment::new("Development", EnvironmentScope::Workspace);
        workspace_environment.variables = workspace
            .iter()
            .map(|(key, value)| EnvironmentVariable {
                key: (*key).to_string(),
                value: (*value).to_string(),
                ..EnvironmentVariable::default()
            })
            .collect();
        let workspace_id = workspace_environment.id;
        let mut environments = vec![workspace_environment];
        let mut active_project_environments = HashMap::new();
        if let Some((project_id, variables)) = project {
            let mut project_environment =
                Environment::new("Project", EnvironmentScope::Project(project_id));
            project_environment.variables = variables
                .iter()
                .map(|(key, value)| EnvironmentVariable {
                    key: (*key).to_string(),
                    value: (*value).to_string(),
                    ..EnvironmentVariable::default()
                })
                .collect();
            active_project_environments.insert(project_id, project_environment.id);
            environments.push(project_environment);
        }
        EnvironmentsEntity {
            environments,
            active_global_environment: None,
            active_workspace_environment: Some(workspace_id),
            active_project_environments,
            active_workspace_id: default_workspace_id(),
            workspace_environments: HashMap::new(),
            revision: 0,
            persistor: None,
        }
    }

    #[test]
    fn interpolates_url_headers_and_body_recursively() {
        let entity = entity_with_variables(
            &[
                ("host", "api.example.com"),
                ("base_url", "https://{{host}}"),
                ("token", "secret"),
            ],
            None,
        );
        let resolved = entity
            .resolve_request(
                None,
                "{{base_url}}/users",
                &[Header::new("Authorization", "Bearer {{token}}")],
                &RequestBody::Json(r#"{"origin":"{{base_url}}"}"#.to_string()),
            )
            .expect("request should resolve");

        assert_eq!(resolved.url, "https://api.example.com/users");
        assert_eq!(resolved.headers[0].value, "Bearer secret");
        assert_eq!(
            resolved.body,
            RequestBody::Json(r#"{"origin":"https://api.example.com"}"#.to_string())
        );
    }

    #[test]
    fn project_variables_override_workspace_variables() {
        let project_id = Uuid::new_v4();
        let entity = entity_with_variables(
            &[("base_url", "https://workspace.example")],
            Some((project_id, &[("base_url", "https://project.example")])),
        );
        let resolved = entity
            .resolve_request(Some(project_id), "{{base_url}}", &[], &RequestBody::None)
            .expect("request should resolve");
        assert_eq!(resolved.url, "https://project.example");
    }

    #[test]
    fn specificity_order_is_global_then_workspace_then_project() {
        let project_id = Uuid::new_v4();
        let mut entity = entity_with_variables(
            &[("base_url", "https://workspace.example")],
            Some((project_id, &[("base_url", "https://project.example")])),
        );
        let mut global = Environment::new("Shared", EnvironmentScope::Global);
        global.variables = vec![
            EnvironmentVariable {
                key: "base_url".to_string(),
                value: "https://global.example".to_string(),
                ..EnvironmentVariable::default()
            },
            EnvironmentVariable {
                key: "shared_id".to_string(),
                value: "from-global".to_string(),
                ..EnvironmentVariable::default()
            },
        ];
        entity.active_global_environment = Some(global.id);
        entity.environments.insert(0, global);

        let workspace = entity
            .resolve_request(None, "{{base_url}}/{{shared_id}}", &[], &RequestBody::None)
            .expect("workspace values should override global values");
        assert_eq!(workspace.url, "https://workspace.example/from-global");

        let project = entity
            .resolve_request(Some(project_id), "{{base_url}}", &[], &RequestBody::None)
            .expect("project values should override workspace values");
        assert_eq!(project.url, "https://project.example");
    }

    #[test]
    fn reports_missing_and_circular_variables() {
        let entity = entity_with_variables(&[("a", "{{b}}"), ("b", "{{a}}")], None);
        let error = entity
            .resolve_request(None, "{{a}}/{{missing}}", &[], &RequestBody::None)
            .expect_err("request should fail");
        assert_eq!(error.unresolved, vec!["missing"]);
        assert!(!error.cycles.is_empty());
    }

    #[test]
    fn supports_escaped_placeholders() {
        let entity = entity_with_variables(&[("value", "done")], None);
        let resolved = entity
            .resolve_request(None, r"\{{value}}/{{value}}", &[], &RequestBody::None)
            .expect("request should resolve");
        assert_eq!(resolved.url, "{{value}}/done");
    }

    #[test]
    fn duplicated_environments_clear_secrets_and_keep_regular_values() {
        let mut source = Environment::new("Production", EnvironmentScope::Workspace);
        source.color = EnvironmentColor::Rose;
        source.variables = vec![
            EnvironmentVariable {
                key: "token".to_string(),
                value: "super-secret".to_string(),
                secret: true,
                ..EnvironmentVariable::default()
            },
            EnvironmentVariable {
                key: "base_url".to_string(),
                value: "https://api.example.com".to_string(),
                ..EnvironmentVariable::default()
            },
        ];

        let duplicate = duplicate_environment_data(source);
        assert_eq!(duplicate.name, "Production Copy");
        assert_eq!(duplicate.color, EnvironmentColor::Rose);
        assert!(duplicate.variables[0].value.is_empty());
        assert_eq!(duplicate.variables[1].value, "https://api.example.com");
    }

    #[test]
    fn legacy_environments_default_to_teal() {
        let environment = Environment::new("Legacy", EnvironmentScope::Workspace);
        let mut value = serde_json::to_value(environment).expect("serialize environment");
        value
            .as_object_mut()
            .expect("environment object")
            .remove("color");

        let decoded: Environment =
            serde_json::from_value(value).expect("deserialize legacy environment");
        assert_eq!(decoded.color, EnvironmentColor::Teal);
    }

    #[test]
    fn custom_environment_colors_round_trip() {
        let color = EnvironmentColor::custom(hsla(0.42, 0.71, 0.53, 1.0));
        let encoded = serde_json::to_string(&color).expect("serialize custom color");
        let decoded: EnvironmentColor =
            serde_json::from_str(&encoded).expect("deserialize custom color");
        assert_eq!(decoded, color);
    }

    #[test]
    fn version_two_workspace_store_migrates_without_data_loss() {
        let workspace_id = default_workspace_id();
        let environment = Environment::new("Development", EnvironmentScope::Workspace);
        let environment_id = environment.id;
        let contents = serde_json::json!({
            "version": 2,
            "workspaces": {
                workspace_id.to_string(): {
                    "environments": [environment],
                    "active_workspace_environment": environment_id,
                    "active_project_environments": {}
                }
            }
        })
        .to_string();

        let (store, migrated) =
            deserialize_environments_store(&contents).expect("migrate version two store");
        assert!(migrated);
        assert_eq!(store.version, ENVIRONMENTS_STORAGE_VERSION);
        assert_eq!(store.workspaces[&workspace_id].environments.len(), 1);
    }
}
