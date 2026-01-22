// Stores request/response history

use chrono::{DateTime, Utc};
use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{RequestData, ResponseData};

/// A single history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub request: RequestData,
    pub response: Option<ResponseData>,
    pub timestamp: DateTime<Utc>,
}

#[allow(dead_code)]
impl HistoryEntry {
    pub fn new(request: RequestData, response: Option<ResponseData>) -> Self {
        Self {
            id: Uuid::new_v4(),
            request,
            response,
            timestamp: Utc::now(),
        }
    }

    /// Display name for the history entry
    pub fn display_name(&self) -> String {
        if !self.request.name.is_empty() && self.request.name != "New Request" {
            self.request.name.clone()
        } else if !self.request.url.is_empty() {
            // Extract host/path from URL
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

/// Events emitted by HistoryEntity
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum HistoryEvent {
    EntryAdded(Uuid),
    EntryRemoved(Uuid),
    Cleared,
}

/// HistoryEntity - stores all request history
#[allow(dead_code)]
pub struct HistoryEntity {
    pub entries: Vec<HistoryEntry>,
    pub max_entries: usize,
}

#[allow(dead_code)]
impl HistoryEntity {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 100, // Keep last 100 entries
        }
    }

    pub fn add_entry(
        &mut self,
        request: RequestData,
        response: Option<ResponseData>,
        cx: &mut Context<Self>,
    ) {
        let entry = HistoryEntry::new(request, response);
        let id = entry.id;

        // Add to front (most recent first)
        self.entries.insert(0, entry);

        // Trim if over limit
        if self.entries.len() > self.max_entries {
            self.entries.pop();
        }

        cx.emit(HistoryEvent::EntryAdded(id));
        cx.notify();
    }

    pub fn remove_entry(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(pos);
            cx.emit(HistoryEvent::EntryRemoved(id));
            cx.notify();
        }
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.entries.clear();
        cx.emit(HistoryEvent::Cleared);
        cx.notify();
    }

    pub fn get_entry(&self, id: Uuid) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn recent(&self, count: usize) -> &[HistoryEntry] {
        let end = count.min(self.entries.len());
        &self.entries[..end]
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl EventEmitter<HistoryEvent> for HistoryEntity {}
