use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

use crate::utils::DebouncedJsonWriter;

const WORKSPACES_STORAGE_VERSION: u32 = 1;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(250);
const DEFAULT_WORKSPACE_UUID: u128 = 0x7365_7475_0000_4000_8000_0000_0000_0001;

pub fn default_workspace_id() -> Uuid {
    Uuid::from_u128(DEFAULT_WORKSPACE_UUID)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
}

impl Workspace {
    fn personal() -> Self {
        Self {
            id: default_workspace_id(),
            name: "Personal Workspace".to_string(),
        }
    }

    fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspacesStore {
    version: u32,
    workspaces: Vec<Workspace>,
    active_workspace_id: Uuid,
}

impl Default for WorkspacesStore {
    fn default() -> Self {
        let workspace = Workspace::personal();
        Self {
            version: WORKSPACES_STORAGE_VERSION,
            active_workspace_id: workspace.id,
            workspaces: vec![workspace],
        }
    }
}

impl WorkspacesStore {
    fn validated(mut self) -> Self {
        if self.version != WORKSPACES_STORAGE_VERSION || self.workspaces.is_empty() {
            return Self::default();
        }
        if !self
            .workspaces
            .iter()
            .any(|workspace| workspace.id == self.active_workspace_id)
        {
            self.active_workspace_id = self.workspaces[0].id;
        }
        self
    }
}

#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    Changed,
    ActiveChanged,
}

pub struct WorkspacesEntity {
    workspaces: Vec<Workspace>,
    active_workspace_id: Uuid,
    persistor: Option<DebouncedJsonWriter<WorkspacesStore>>,
}

impl WorkspacesEntity {
    pub fn load() -> Self {
        let path = storage_path();
        let store = path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|contents| serde_json::from_str::<WorkspacesStore>(&contents).ok())
            .map(WorkspacesStore::validated)
            .unwrap_or_default();

        Self {
            workspaces: store.workspaces,
            active_workspace_id: store.active_workspace_id,
            persistor: path.map(|path| DebouncedJsonWriter::new("workspaces", path, SAVE_DEBOUNCE)),
        }
    }

    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn active_workspace_id(&self) -> Uuid {
        self.active_workspace_id
    }

    pub fn active_workspace(&self) -> &Workspace {
        self.workspaces
            .iter()
            .find(|workspace| workspace.id == self.active_workspace_id)
            .unwrap_or(&self.workspaces[0])
    }

    pub fn create_workspace(&mut self, name: impl Into<String>, cx: &mut Context<Self>) -> Uuid {
        let requested_name = name.into();
        let name = requested_name.trim();
        let workspace = Workspace::new(if name.is_empty() {
            "Untitled Workspace"
        } else {
            name
        });
        let id = workspace.id;
        self.workspaces.push(workspace);
        self.changed(WorkspaceEvent::Changed, cx);
        id
    }

    pub fn set_active_workspace(&mut self, id: Uuid, cx: &mut Context<Self>) -> bool {
        if id == self.active_workspace_id
            || !self.workspaces.iter().any(|workspace| workspace.id == id)
        {
            return false;
        }
        self.active_workspace_id = id;
        self.changed(WorkspaceEvent::ActiveChanged, cx);
        true
    }

    pub fn rename_workspace(&mut self, id: Uuid, name: String, cx: &mut Context<Self>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let Some(workspace) = self
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
        else {
            return;
        };
        workspace.name = name.to_string();
        self.changed(WorkspaceEvent::Changed, cx);
    }

    pub fn remove_workspace(&mut self, id: Uuid, cx: &mut Context<Self>) -> bool {
        if self.workspaces.len() <= 1 {
            return false;
        }
        let Some(index) = self
            .workspaces
            .iter()
            .position(|workspace| workspace.id == id)
        else {
            return false;
        };
        self.workspaces.remove(index);
        if self.active_workspace_id == id {
            self.active_workspace_id = self.workspaces[0].id;
        }
        self.changed(WorkspaceEvent::Changed, cx);
        true
    }

    fn changed(&self, event: WorkspaceEvent, cx: &mut Context<Self>) {
        self.save_to_file();
        cx.emit(event);
        cx.notify();
    }

    fn save_to_file(&self) {
        if let Some(persistor) = &self.persistor {
            persistor.schedule_save(WorkspacesStore {
                version: WORKSPACES_STORAGE_VERSION,
                workspaces: self.workspaces.clone(),
                active_workspace_id: self.active_workspace_id,
            });
        }
    }
}

impl EventEmitter<WorkspaceEvent> for WorkspacesEntity {}

fn storage_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|mut path| {
        path.push("setu");
        path.push("workspaces.json");
        path
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_active_workspace_falls_back_to_first_workspace() {
        let first = Workspace::new("First");
        let store = WorkspacesStore {
            version: WORKSPACES_STORAGE_VERSION,
            workspaces: vec![first.clone()],
            active_workspace_id: Uuid::new_v4(),
        }
        .validated();

        assert_eq!(store.active_workspace_id, first.id);
    }

    #[test]
    fn empty_store_restores_personal_workspace() {
        let store = WorkspacesStore {
            version: WORKSPACES_STORAGE_VERSION,
            workspaces: Vec::new(),
            active_workspace_id: Uuid::new_v4(),
        }
        .validated();

        assert_eq!(store.workspaces.len(), 1);
        assert_eq!(store.active_workspace_id, default_workspace_id());
    }
}
