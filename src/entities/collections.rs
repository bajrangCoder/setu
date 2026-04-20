use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

use crate::importers::{ImportedCollection, ImportedNode};
use crate::utils::DebouncedJsonWriter;

use super::RequestData;

const COLLECTIONS_STORAGE_VERSION: u32 = 1;
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

fn default_expanded() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRequestNode {
    pub id: Uuid,
    pub request: RequestData,
}

impl CollectionRequestNode {
    pub fn new(request: RequestData) -> Self {
        Self {
            id: Uuid::new_v4(),
            request,
        }
    }

    fn from_legacy(id: Uuid, request: RequestData) -> Self {
        Self { id, request }
    }

    pub fn display_name(&self) -> String {
        if !self.request.name.is_empty() && self.request.name != "New Request" {
            self.request.name.clone()
        } else if !self.request.url.is_empty() {
            self.request
                .url
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .chars()
                .take(40)
                .collect()
        } else {
            "Untitled Request".to_string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionFolderNode {
    pub id: Uuid,
    pub name: String,
    #[serde(default = "default_expanded")]
    pub expanded: bool,
    #[serde(default)]
    pub children: Vec<CollectionNode>,
}

impl CollectionFolderNode {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            expanded: true,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CollectionNode {
    Folder(CollectionFolderNode),
    Request(CollectionRequestNode),
}

impl CollectionNode {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Folder(folder) => folder.id,
            Self::Request(request) => request.id,
        }
    }

    pub fn request(&self) -> Option<&CollectionRequestNode> {
        match self {
            Self::Request(request) => Some(request),
            Self::Folder(_) => None,
        }
    }

    pub fn folder(&self) -> Option<&CollectionFolderNode> {
        match self {
            Self::Folder(folder) => Some(folder),
            Self::Request(_) => None,
        }
    }

    pub fn folder_mut(&mut self) -> Option<&mut CollectionFolderNode> {
        match self {
            Self::Folder(folder) => Some(folder),
            Self::Request(_) => None,
        }
    }

    pub fn request_count(&self) -> usize {
        match self {
            Self::Request(_) => 1,
            Self::Folder(folder) => folder
                .children
                .iter()
                .map(CollectionNode::request_count)
                .sum(),
        }
    }

    fn matches_query(&self, query: &str) -> bool {
        match self {
            Self::Folder(folder) => folder.name.to_lowercase().contains(query),
            Self::Request(request) => {
                request.request.name.to_lowercase().contains(query)
                    || request.request.url.to_lowercase().contains(query)
                    || request
                        .request
                        .method
                        .as_str()
                        .to_lowercase()
                        .contains(query)
            }
        }
    }

    fn contains_node(&self, node_id: Uuid) -> bool {
        if self.id() == node_id {
            return true;
        }

        match self {
            Self::Folder(folder) => folder
                .children
                .iter()
                .any(|child| child.contains_node(node_id)),
            Self::Request(_) => false,
        }
    }

    fn filtered_clone(&self, query: &str) -> Option<Self> {
        match self {
            Self::Request(request) => {
                if self.matches_query(query) {
                    Some(Self::Request(request.clone()))
                } else {
                    None
                }
            }
            Self::Folder(folder) => {
                if folder.name.to_lowercase().contains(query) {
                    let mut clone = folder.clone();
                    clone.expanded = true;
                    return Some(Self::Folder(clone));
                }

                let filtered_children: Vec<_> = folder
                    .children
                    .iter()
                    .filter_map(|child| child.filtered_clone(query))
                    .collect();

                if filtered_children.is_empty() {
                    None
                } else {
                    let mut clone = folder.clone();
                    clone.children = filtered_children;
                    clone.expanded = true;
                    Some(Self::Folder(clone))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<CollectionNode>,
    #[serde(default = "default_expanded")]
    pub expanded: bool,
}

impl Collection {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            nodes: Vec::new(),
            expanded: true,
        }
    }

    pub fn request_count(&self) -> usize {
        self.nodes.iter().map(CollectionNode::request_count).sum()
    }

    pub fn filtered_clone(&self, query: &str) -> Option<Self> {
        if query.is_empty() {
            return Some(self.clone());
        }

        let query = query.to_lowercase();
        if self.name.to_lowercase().contains(&query) {
            let mut clone = self.clone();
            clone.expanded = true;
            return Some(clone);
        }

        let filtered_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|node| node.filtered_clone(&query))
            .collect();

        if filtered_nodes.is_empty() {
            None
        } else {
            let mut clone = self.clone();
            clone.nodes = filtered_nodes;
            clone.expanded = true;
            Some(clone)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollectionsStore {
    version: u32,
    collections: Vec<Collection>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyCollectionItem {
    id: Uuid,
    request: RequestData,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyCollection {
    id: Uuid,
    name: String,
    #[serde(default)]
    items: Vec<LegacyCollectionItem>,
    #[serde(default = "default_expanded")]
    expanded: bool,
}

impl LegacyCollection {
    fn into_collection(self) -> Collection {
        Collection {
            id: self.id,
            name: self.name,
            nodes: self
                .items
                .into_iter()
                .map(|item| {
                    CollectionNode::Request(CollectionRequestNode::from_legacy(
                        item.id,
                        item.request,
                    ))
                })
                .collect(),
            expanded: self.expanded,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionDestination {
    pub collection_id: Uuid,
    pub folder_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionDestinationEntry {
    pub destination: CollectionDestination,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveNodeError {
    NodeNotFound,
    TargetCollectionNotFound,
    TargetFolderNotFound,
    CannotMoveIntoSelf,
    CannotMoveIntoDescendant,
}

impl fmt::Display for MoveNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            MoveNodeError::NodeNotFound => "The selected node could not be found.",
            MoveNodeError::TargetCollectionNotFound => "The target collection no longer exists.",
            MoveNodeError::TargetFolderNotFound => "The target folder could not be found.",
            MoveNodeError::CannotMoveIntoSelf => "A node cannot be moved into itself.",
            MoveNodeError::CannotMoveIntoDescendant => {
                "A folder cannot be moved into one of its descendants."
            }
        };

        write!(f, "{message}")
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum CollectionsEvent {
    CollectionAdded(Uuid),
    CollectionRemoved(Uuid),
    CollectionUpdated(Uuid),
    NodeAdded(Uuid, Uuid),
    NodeRemoved(Uuid, Uuid),
    NodeMoved {
        source_collection_id: Uuid,
        target_collection_id: Uuid,
        node_id: Uuid,
    },
}

pub struct CollectionsEntity {
    pub collections: Vec<Collection>,
    persistor: Option<DebouncedJsonWriter<CollectionsStore>>,
}

#[allow(dead_code)]
impl CollectionsEntity {
    pub fn new() -> Self {
        let storage_path = Self::get_storage_path();
        let mut entity = Self {
            collections: Vec::new(),
            persistor: storage_path
                .clone()
                .map(|path| DebouncedJsonWriter::new("collections", path, SAVE_DEBOUNCE)),
        };

        if let Some(ref path) = storage_path {
            entity.load_from_file(path);
        }

        entity
    }

    fn get_storage_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|mut path: PathBuf| {
            path.push("setu");
            path.push("collections.json");
            path
        })
    }

    fn load_from_file(&mut self, path: &PathBuf) {
        if !path.exists() {
            return;
        }

        let Ok(contents) = fs::read_to_string(path) else {
            return;
        };

        match deserialize_store(&contents) {
            Ok((collections, migrated)) => {
                self.collections = collections;
                log::info!(
                    "Loaded {} collections from {:?}",
                    self.collections.len(),
                    path
                );

                if migrated {
                    self.save_to_file();
                }
            }
            Err(err) => {
                log::error!("Failed to parse collections store: {}", err);
            }
        }
    }

    fn save_to_file(&self) {
        if let Some(persistor) = &self.persistor {
            let store = CollectionsStore {
                version: COLLECTIONS_STORAGE_VERSION,
                collections: self.collections.clone(),
            };
            persistor.schedule_save(store);
        }
    }

    pub fn create_collection(&mut self, name: &str, cx: &mut Context<Self>) -> Uuid {
        let collection = Collection::new(name);
        let id = collection.id;
        self.collections.push(collection);
        self.save_to_file();
        cx.emit(CollectionsEvent::CollectionAdded(id));
        cx.notify();
        id
    }

    pub fn import_collection(
        &mut self,
        imported: ImportedCollection,
        cx: &mut Context<Self>,
    ) -> Uuid {
        let collection = Collection {
            id: Uuid::new_v4(),
            name: imported.name,
            nodes: imported
                .nodes
                .into_iter()
                .map(CollectionNode::from_imported_node)
                .collect(),
            expanded: true,
        };
        let id = collection.id;
        self.collections.push(collection);
        self.save_to_file();
        cx.emit(CollectionsEvent::CollectionAdded(id));
        cx.notify();
        id
    }

    pub fn remove_collection(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(pos) = self.collections.iter().position(|c| c.id == id) {
            self.collections.remove(pos);
            self.save_to_file();
            cx.emit(CollectionsEvent::CollectionRemoved(id));
            cx.notify();
        }
    }

    pub fn rename_collection(&mut self, id: Uuid, new_name: &str, cx: &mut Context<Self>) {
        if let Some(collection) = self.collections.iter_mut().find(|c| c.id == id) {
            collection.name = new_name.to_string();
            self.save_to_file();
            cx.emit(CollectionsEvent::CollectionUpdated(id));
            cx.notify();
        }
    }

    pub fn rename_node(
        &mut self,
        collection_id: Uuid,
        node_id: Uuid,
        new_name: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(collection) = self.collections.iter_mut().find(|c| c.id == collection_id) else {
            return false;
        };

        let Some(node) = find_node_mut(&mut collection.nodes, node_id) else {
            return false;
        };

        match node {
            CollectionNode::Folder(folder) => folder.name = new_name.to_string(),
            CollectionNode::Request(request) => request.request.name = new_name.to_string(),
        }

        self.save_to_file();
        cx.emit(CollectionsEvent::CollectionUpdated(collection_id));
        cx.notify();
        true
    }

    pub fn toggle_collection_expanded(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(collection) = self.collections.iter_mut().find(|c| c.id == id) {
            collection.expanded = !collection.expanded;
            self.save_to_file();
            cx.emit(CollectionsEvent::CollectionUpdated(id));
            cx.notify();
        }
    }

    pub fn toggle_node_expanded(
        &mut self,
        collection_id: Uuid,
        node_id: Uuid,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(collection) = self.collections.iter_mut().find(|c| c.id == collection_id) else {
            return false;
        };

        let Some(node) = find_node_mut(&mut collection.nodes, node_id) else {
            return false;
        };

        let Some(folder) = node.folder_mut() else {
            return false;
        };

        folder.expanded = !folder.expanded;
        self.save_to_file();
        cx.emit(CollectionsEvent::CollectionUpdated(collection_id));
        cx.notify();
        true
    }

    pub fn create_folder(
        &mut self,
        collection_id: Uuid,
        parent_folder_id: Option<Uuid>,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Option<Uuid> {
        let folder = CollectionFolderNode::new(name);
        let folder_id = folder.id;
        let node = CollectionNode::Folder(folder);

        let Some(collection) = self.collections.iter_mut().find(|c| c.id == collection_id) else {
            return None;
        };

        if insert_node(&mut collection.nodes, parent_folder_id, node).is_err() {
            return None;
        }

        self.save_to_file();
        cx.emit(CollectionsEvent::NodeAdded(collection_id, folder_id));
        cx.notify();
        Some(folder_id)
    }

    pub fn add_request_node(
        &mut self,
        collection_id: Uuid,
        parent_folder_id: Option<Uuid>,
        request: RequestData,
        cx: &mut Context<Self>,
    ) -> Option<Uuid> {
        let node = CollectionNode::Request(CollectionRequestNode::new(request));
        let node_id = node.id();

        let Some(collection) = self.collections.iter_mut().find(|c| c.id == collection_id) else {
            return None;
        };

        if insert_node(&mut collection.nodes, parent_folder_id, node).is_err() {
            return None;
        }

        self.save_to_file();
        cx.emit(CollectionsEvent::NodeAdded(collection_id, node_id));
        cx.notify();
        Some(node_id)
    }

    pub fn remove_node(
        &mut self,
        collection_id: Uuid,
        node_id: Uuid,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(collection) = self.collections.iter_mut().find(|c| c.id == collection_id) else {
            return false;
        };

        if remove_node(&mut collection.nodes, node_id).is_none() {
            return false;
        }

        self.save_to_file();
        cx.emit(CollectionsEvent::NodeRemoved(collection_id, node_id));
        cx.notify();
        true
    }

    pub fn move_node(
        &mut self,
        source_collection_id: Uuid,
        node_id: Uuid,
        target_collection_id: Uuid,
        target_parent_folder_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) -> Result<(), MoveNodeError> {
        move_node_between_collections(
            &mut self.collections,
            source_collection_id,
            node_id,
            target_collection_id,
            target_parent_folder_id,
        )?;

        self.save_to_file();
        cx.emit(CollectionsEvent::NodeMoved {
            source_collection_id,
            target_collection_id,
            node_id,
        });
        cx.notify();
        Ok(())
    }

    pub fn get_collection(&self, id: Uuid) -> Option<&Collection> {
        self.collections.iter().find(|c| c.id == id)
    }

    pub fn get_collection_mut(&mut self, id: Uuid) -> Option<&mut Collection> {
        self.collections.iter_mut().find(|c| c.id == id)
    }

    pub fn get_node(&self, collection_id: Uuid, node_id: Uuid) -> Option<&CollectionNode> {
        self.get_collection(collection_id)
            .and_then(|collection| find_node(&collection.nodes, node_id))
    }

    pub fn get_request_node(
        &self,
        collection_id: Uuid,
        node_id: Uuid,
    ) -> Option<&CollectionRequestNode> {
        self.get_node(collection_id, node_id)
            .and_then(CollectionNode::request)
    }

    pub fn filtered_collections(&self, query: &str) -> Vec<Collection> {
        self.collections
            .iter()
            .filter_map(|collection| collection.filtered_clone(query))
            .collect()
    }

    pub fn destination_entries(&self) -> Vec<CollectionDestinationEntry> {
        let mut entries = Vec::new();

        for collection in &self.collections {
            let root_path = collection.name.clone();
            entries.push(CollectionDestinationEntry {
                destination: CollectionDestination {
                    collection_id: collection.id,
                    folder_id: None,
                },
                label: root_path.clone(),
            });
            append_destination_entries(collection.id, &collection.nodes, &root_path, &mut entries);
        }

        entries
    }

    pub fn move_destinations_for_node(
        &self,
        source_collection_id: Uuid,
        node_id: Uuid,
    ) -> Vec<CollectionDestinationEntry> {
        self.destination_entries()
            .into_iter()
            .filter(|entry| {
                if entry.destination.collection_id != source_collection_id {
                    return true;
                }

                match entry.destination.folder_id {
                    Some(folder_id) if folder_id == node_id => false,
                    Some(folder_id) => self
                        .get_node(source_collection_id, node_id)
                        .map(|node| !node.contains_node(folder_id))
                        .unwrap_or(false),
                    None => true,
                }
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.collections.is_empty()
    }

    pub fn len(&self) -> usize {
        self.collections.len()
    }
}

impl EventEmitter<CollectionsEvent> for CollectionsEntity {}

impl CollectionNode {
    fn from_imported_node(node: ImportedNode) -> Self {
        match node {
            ImportedNode::Folder { name, children } => {
                CollectionNode::Folder(CollectionFolderNode {
                    id: Uuid::new_v4(),
                    name,
                    expanded: true,
                    children: children
                        .into_iter()
                        .map(CollectionNode::from_imported_node)
                        .collect(),
                })
            }
            ImportedNode::Request { request } => {
                CollectionNode::Request(CollectionRequestNode::new(request))
            }
        }
    }
}

fn deserialize_store(contents: &str) -> Result<(Vec<Collection>, bool), serde_json::Error> {
    if let Ok(store) = serde_json::from_str::<CollectionsStore>(contents) {
        return Ok((store.collections, false));
    }

    let legacy_collections = serde_json::from_str::<Vec<LegacyCollection>>(contents)?;
    Ok((
        legacy_collections
            .into_iter()
            .map(LegacyCollection::into_collection)
            .collect(),
        true,
    ))
}

fn append_destination_entries(
    collection_id: Uuid,
    nodes: &[CollectionNode],
    parent_path: &str,
    entries: &mut Vec<CollectionDestinationEntry>,
) {
    for node in nodes {
        let Some(folder) = node.folder() else {
            continue;
        };

        let path = format!("{parent_path} / {}", folder.name);
        entries.push(CollectionDestinationEntry {
            destination: CollectionDestination {
                collection_id,
                folder_id: Some(folder.id),
            },
            label: path.clone(),
        });
        append_destination_entries(collection_id, &folder.children, &path, entries);
    }
}

fn find_node(nodes: &[CollectionNode], node_id: Uuid) -> Option<&CollectionNode> {
    for node in nodes {
        if node.id() == node_id {
            return Some(node);
        }

        if let Some(folder) = node.folder() {
            if let Some(found) = find_node(&folder.children, node_id) {
                return Some(found);
            }
        }
    }

    None
}

fn find_node_mut(nodes: &mut [CollectionNode], node_id: Uuid) -> Option<&mut CollectionNode> {
    for node in nodes {
        if node.id() == node_id {
            return Some(node);
        }

        if let Some(folder) = node.folder_mut() {
            if let Some(found) = find_node_mut(&mut folder.children, node_id) {
                return Some(found);
            }
        }
    }

    None
}

fn find_folder_mut(
    nodes: &mut [CollectionNode],
    folder_id: Uuid,
) -> Option<&mut CollectionFolderNode> {
    for node in nodes {
        if let Some(folder) = node.folder_mut() {
            if folder.id == folder_id {
                return Some(folder);
            }

            if let Some(found) = find_folder_mut(&mut folder.children, folder_id) {
                return Some(found);
            }
        }
    }

    None
}

fn insert_node(
    nodes: &mut Vec<CollectionNode>,
    parent_folder_id: Option<Uuid>,
    node: CollectionNode,
) -> Result<(), MoveNodeError> {
    match parent_folder_id {
        Some(folder_id) => {
            let Some(folder) = find_folder_mut(nodes.as_mut_slice(), folder_id) else {
                return Err(MoveNodeError::TargetFolderNotFound);
            };
            folder.children.push(node);
        }
        None => nodes.push(node),
    }

    Ok(())
}

fn remove_node(nodes: &mut Vec<CollectionNode>, node_id: Uuid) -> Option<CollectionNode> {
    if let Some(index) = nodes.iter().position(|node| node.id() == node_id) {
        return Some(nodes.remove(index));
    }

    for node in nodes {
        if let Some(folder) = node.folder_mut() {
            if let Some(removed) = remove_node(&mut folder.children, node_id) {
                return Some(removed);
            }
        }
    }

    None
}

fn move_node_between_collections(
    collections: &mut Vec<Collection>,
    source_collection_id: Uuid,
    node_id: Uuid,
    target_collection_id: Uuid,
    target_parent_folder_id: Option<Uuid>,
) -> Result<(), MoveNodeError> {
    let source_index = collections
        .iter()
        .position(|collection| collection.id == source_collection_id)
        .ok_or(MoveNodeError::NodeNotFound)?;
    let target_index = collections
        .iter()
        .position(|collection| collection.id == target_collection_id)
        .ok_or(MoveNodeError::TargetCollectionNotFound)?;

    {
        let source_collection = &collections[source_index];
        let Some(source_node) = find_node(&source_collection.nodes, node_id) else {
            return Err(MoveNodeError::NodeNotFound);
        };

        if let Some(folder_id) = target_parent_folder_id {
            let target_collection = &collections[target_index];
            if find_node(&target_collection.nodes, folder_id).is_none() {
                return Err(MoveNodeError::TargetFolderNotFound);
            }
        }

        if source_collection_id == target_collection_id {
            if target_parent_folder_id == Some(node_id) {
                return Err(MoveNodeError::CannotMoveIntoSelf);
            }

            if let Some(folder_id) = target_parent_folder_id {
                if source_node.contains_node(folder_id) {
                    return Err(MoveNodeError::CannotMoveIntoDescendant);
                }
            }
        }
    }

    if source_index == target_index {
        let collection = &mut collections[source_index];
        let node =
            remove_node(&mut collection.nodes, node_id).ok_or(MoveNodeError::NodeNotFound)?;
        insert_node(&mut collection.nodes, target_parent_folder_id, node)?;
        return Ok(());
    }

    let node = {
        let source_collection = &mut collections[source_index];
        remove_node(&mut source_collection.nodes, node_id).ok_or(MoveNodeError::NodeNotFound)?
    };

    let target_collection = &mut collections[target_index];
    insert_node(&mut target_collection.nodes, target_parent_folder_id, node)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{Header, HttpMethod, RequestBody};

    fn sample_request(name: &str, url: &str) -> RequestData {
        RequestData {
            id: Uuid::new_v4(),
            name: name.to_string(),
            url: url.to_string(),
            method: HttpMethod::Post,
            headers: vec![Header::new("Content-Type", "application/json")],
            body: RequestBody::Json(r#"{"ok":true}"#.to_string()),
            is_sending: false,
        }
    }

    #[test]
    fn migrates_legacy_flat_storage() {
        let collection_id = Uuid::new_v4();
        let item_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let legacy = format!(
            r#"[{{
                "id":"{collection_id}",
                "name":"Legacy",
                "expanded":false,
                "items":[{{
                    "id":"{item_id}",
                    "request":{{
                        "id":"{request_id}",
                        "name":"Legacy Request",
                        "url":"https://example.com",
                        "method":"Get",
                        "headers":[],
                        "body":"None"
                    }}
                }}]
            }}]"#
        );

        let (collections, migrated) = deserialize_store(&legacy).expect("legacy parse");
        assert!(migrated);
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].id, collection_id);
        assert!(!collections[0].expanded);

        let node = collections[0].nodes.first().expect("request node");
        let request = node.request().expect("request");
        assert_eq!(request.id, item_id);
        assert_eq!(request.request.id, request_id);
    }

    #[test]
    fn round_trips_versioned_tree_storage() {
        let request = sample_request("Create User", "https://example.com/users");
        let collection = Collection {
            id: Uuid::new_v4(),
            name: "Workspace".to_string(),
            expanded: true,
            nodes: vec![CollectionNode::Folder(CollectionFolderNode {
                id: Uuid::new_v4(),
                name: "Users".to_string(),
                expanded: true,
                children: vec![CollectionNode::Request(CollectionRequestNode::new(
                    request.clone(),
                ))],
            })],
        };

        let store = CollectionsStore {
            version: COLLECTIONS_STORAGE_VERSION,
            collections: vec![collection.clone()],
        };
        let encoded = serde_json::to_string(&store).expect("encode");
        let (decoded, migrated) = deserialize_store(&encoded).expect("decode");

        assert!(!migrated);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].name, collection.name);
        assert_eq!(decoded[0].request_count(), 1);
        let folder = decoded[0].nodes[0].folder().expect("folder");
        assert_eq!(folder.name, "Users");
        assert_eq!(
            folder.children[0].request().expect("request").request.url,
            request.url
        );
    }

    #[test]
    fn prevents_moving_folder_into_descendant() {
        let folder_a_id = Uuid::new_v4();
        let folder_b_id = Uuid::new_v4();
        let collection_id = Uuid::new_v4();
        let mut collections = vec![Collection {
            id: collection_id,
            name: "Workspace".to_string(),
            expanded: true,
            nodes: vec![CollectionNode::Folder(CollectionFolderNode {
                id: folder_a_id,
                name: "A".to_string(),
                expanded: true,
                children: vec![CollectionNode::Folder(CollectionFolderNode {
                    id: folder_b_id,
                    name: "B".to_string(),
                    expanded: true,
                    children: vec![],
                })],
            })],
        }];

        let result = move_node_between_collections(
            &mut collections,
            collection_id,
            folder_a_id,
            collection_id,
            Some(folder_b_id),
        );

        assert_eq!(result, Err(MoveNodeError::CannotMoveIntoDescendant));
    }

    #[test]
    fn moves_requests_across_collections() {
        let source_collection_id = Uuid::new_v4();
        let target_collection_id = Uuid::new_v4();
        let request_node = CollectionNode::Request(CollectionRequestNode::new(sample_request(
            "List Users",
            "https://example.com/users",
        )));
        let request_node_id = request_node.id();

        let mut collections = vec![
            Collection {
                id: source_collection_id,
                name: "Source".to_string(),
                expanded: true,
                nodes: vec![request_node],
            },
            Collection {
                id: target_collection_id,
                name: "Target".to_string(),
                expanded: true,
                nodes: vec![],
            },
        ];

        move_node_between_collections(
            &mut collections,
            source_collection_id,
            request_node_id,
            target_collection_id,
            None,
        )
        .expect("move succeeds");

        assert!(collections[0].nodes.is_empty());
        assert_eq!(collections[1].nodes.len(), 1);
        assert_eq!(collections[1].nodes[0].id(), request_node_id);
    }
}
