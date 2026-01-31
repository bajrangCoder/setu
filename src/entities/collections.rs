use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use super::RequestData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionItem {
    pub id: Uuid,
    pub request: RequestData,
}

#[allow(dead_code)]
impl CollectionItem {
    pub fn new(request: RequestData) -> Self {
        Self {
            id: Uuid::new_v4(),
            request,
        }
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
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    pub items: Vec<CollectionItem>,
    #[serde(default)]
    pub expanded: bool,
}

#[allow(dead_code)]
impl Collection {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            items: Vec::new(),
            expanded: true,
        }
    }

    pub fn add_item(&mut self, request: RequestData) -> Uuid {
        let item = CollectionItem::new(request);
        let id = item.id;
        self.items.push(item);
        id
    }

    pub fn remove_item(&mut self, item_id: Uuid) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i.id == item_id) {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn get_item(&self, item_id: Uuid) -> Option<&CollectionItem> {
        self.items.iter().find(|i| i.id == item_id)
    }

    pub fn get_item_mut(&mut self, item_id: Uuid) -> Option<&mut CollectionItem> {
        self.items.iter_mut().find(|i| i.id == item_id)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum CollectionsEvent {
    CollectionAdded(Uuid),
    CollectionRemoved(Uuid),
    CollectionUpdated(Uuid),
    ItemAdded(Uuid, Uuid),
    ItemRemoved(Uuid, Uuid),
}

pub struct CollectionsEntity {
    pub collections: Vec<Collection>,
    storage_path: Option<PathBuf>,
}

#[allow(dead_code)]
impl CollectionsEntity {
    pub fn new() -> Self {
        let storage_path = Self::get_storage_path();
        let mut entity = Self {
            collections: Vec::new(),
            storage_path: storage_path.clone(),
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
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(collections) = serde_json::from_str::<Vec<Collection>>(&contents) {
                    self.collections = collections;
                    log::info!(
                        "Loaded {} collections from {:?}",
                        self.collections.len(),
                        path
                    );
                }
            }
        }
    }

    fn save_to_file(&self) {
        if let Some(ref path) = self.storage_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(contents) = serde_json::to_string_pretty(&self.collections) {
                if let Err(e) = fs::write(path, contents) {
                    log::error!("Failed to save collections: {}", e);
                }
            }
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

    pub fn toggle_collection_expanded(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(collection) = self.collections.iter_mut().find(|c| c.id == id) {
            collection.expanded = !collection.expanded;
            self.save_to_file();
            cx.emit(CollectionsEvent::CollectionUpdated(id));
            cx.notify();
        }
    }

    pub fn add_item_to_collection(
        &mut self,
        collection_id: Uuid,
        request: RequestData,
        cx: &mut Context<Self>,
    ) -> Option<Uuid> {
        if let Some(collection) = self.collections.iter_mut().find(|c| c.id == collection_id) {
            let item_id = collection.add_item(request);
            self.save_to_file();
            cx.emit(CollectionsEvent::ItemAdded(collection_id, item_id));
            cx.notify();
            Some(item_id)
        } else {
            None
        }
    }

    pub fn remove_item_from_collection(
        &mut self,
        collection_id: Uuid,
        item_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        if let Some(collection) = self.collections.iter_mut().find(|c| c.id == collection_id) {
            if collection.remove_item(item_id) {
                self.save_to_file();
                cx.emit(CollectionsEvent::ItemRemoved(collection_id, item_id));
                cx.notify();
            }
        }
    }

    pub fn get_collection(&self, id: Uuid) -> Option<&Collection> {
        self.collections.iter().find(|c| c.id == id)
    }

    pub fn get_collection_mut(&mut self, id: Uuid) -> Option<&mut Collection> {
        self.collections.iter_mut().find(|c| c.id == id)
    }

    pub fn search(&self, query: &str) -> Vec<(&Collection, Vec<&CollectionItem>)> {
        let query = query.to_lowercase();
        self.collections
            .iter()
            .filter_map(|c| {
                let matching_items: Vec<&CollectionItem> = c
                    .items
                    .iter()
                    .filter(|i| {
                        i.request.url.to_lowercase().contains(&query)
                            || i.request.name.to_lowercase().contains(&query)
                            || i.request.method.as_str().to_lowercase().contains(&query)
                    })
                    .collect();

                if !matching_items.is_empty() || c.name.to_lowercase().contains(&query) {
                    Some((c, matching_items))
                } else {
                    None
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

    pub fn total_items(&self) -> usize {
        self.collections.iter().map(|c| c.items.len()).sum()
    }
}

impl Default for CollectionsEntity {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter<CollectionsEvent> for CollectionsEntity {}
