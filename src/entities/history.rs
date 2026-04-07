use chrono::{DateTime, Utc};
use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use super::{RequestData, ResponseData};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub request: RequestData,
    pub response: Option<ResponseData>,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub starred: bool,
}

impl HistoryEntry {
    pub fn new(request: RequestData, response: Option<ResponseData>) -> Self {
        Self {
            id: Uuid::new_v4(),
            request,
            response,
            timestamp: Utc::now(),
            starred: false,
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
                .take(50)
                .collect()
        } else {
            "Untitled Request".to_string()
        }
    }

    pub fn time_group(&self) -> TimeGroup {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.timestamp);

        if duration.num_hours() < 24 {
            TimeGroup::Today
        } else if duration.num_days() < 7 {
            TimeGroup::ThisWeek
        } else if duration.num_days() < 14 {
            TimeGroup::LastWeek
        } else if duration.num_days() < 30 {
            TimeGroup::ThisMonth
        } else {
            TimeGroup::Older
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeGroup {
    Today,
    ThisWeek,
    LastWeek,
    ThisMonth,
    Older,
}

impl TimeGroup {
    pub fn label(&self) -> &'static str {
        match self {
            TimeGroup::Today => "Today",
            TimeGroup::ThisWeek => "This Week",
            TimeGroup::LastWeek => "Last Week",
            TimeGroup::ThisMonth => "This Month",
            TimeGroup::Older => "Older",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum HistoryEvent {
    EntryAdded(Uuid),
    EntryRemoved(Uuid),
    EntryUpdated(Uuid),
    Cleared,
}

pub struct HistoryEntity {
    pub entries: Vec<HistoryEntry>,
    pub max_entries: usize,
    storage_path: Option<PathBuf>,
    collapsed_groups: HashSet<TimeGroup>,
}

impl HistoryEntity {
    pub fn new() -> Self {
        let storage_path = Self::get_storage_path();
        let mut entity = Self {
            entries: Vec::new(),
            max_entries: 500,
            storage_path: storage_path.clone(),
            collapsed_groups: HashSet::new(),
        };

        if let Some(ref path) = storage_path {
            entity.load_from_file(path);
        }

        entity
    }

    fn get_storage_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|mut path| {
            path.push("setu");
            path.push("history.json");
            path
        })
    }

    fn load_from_file(&mut self, path: &PathBuf) {
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(mut entries) = serde_json::from_str::<Vec<HistoryEntry>>(&contents) {
                    let mut compacted = false;
                    for entry in &mut entries {
                        if let Some(response) = entry.response.as_mut() {
                            compacted |= response.compact_storage();
                        }
                    }

                    self.entries = entries;
                    log::info!(
                        "Loaded {} history entries from {:?}",
                        self.entries.len(),
                        path
                    );

                    if compacted {
                        self.save_to_file();
                        log::info!("Compacted duplicate response payloads in history storage");
                    }
                }
            }
        }
    }

    fn save_to_file(&self) {
        if let Some(ref path) = self.storage_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(contents) = serde_json::to_string_pretty(&self.entries) {
                if let Err(e) = fs::write(path, contents) {
                    log::error!("Failed to save history: {}", e);
                }
            }
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

        self.entries.insert(0, entry);

        if self.entries.len() > self.max_entries {
            self.entries.pop();
        }

        self.save_to_file();
        cx.emit(HistoryEvent::EntryAdded(id));
        cx.notify();
    }

    pub fn remove_entry(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(pos);
            self.save_to_file();
            cx.emit(HistoryEvent::EntryRemoved(id));
            cx.notify();
        }
    }

    pub fn toggle_star(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.starred = !entry.starred;
            self.save_to_file();
            cx.emit(HistoryEvent::EntryUpdated(id));
            cx.notify();
        }
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.entries.clear();
        self.save_to_file();
        cx.emit(HistoryEvent::Cleared);
        cx.notify();
    }

    pub fn clear_unstarred(&mut self, cx: &mut Context<Self>) {
        self.entries.retain(|e| e.starred);
        self.save_to_file();
        cx.emit(HistoryEvent::Cleared);
        cx.notify();
    }

    pub fn get_entry(&self, id: Uuid) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn starred(&self) -> Vec<&HistoryEntry> {
        self.entries.iter().filter(|e| e.starred).collect()
    }

    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let query = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.request.url.to_lowercase().contains(&query)
                    || e.request.name.to_lowercase().contains(&query)
                    || e.request.method.as_str().to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn grouped_entries(&self) -> Vec<(TimeGroup, Vec<&HistoryEntry>)> {
        let mut groups: Vec<(TimeGroup, Vec<&HistoryEntry>)> = vec![
            (TimeGroup::Today, Vec::new()),
            (TimeGroup::ThisWeek, Vec::new()),
            (TimeGroup::LastWeek, Vec::new()),
            (TimeGroup::ThisMonth, Vec::new()),
            (TimeGroup::Older, Vec::new()),
        ];

        for entry in &self.entries {
            let group = entry.time_group();
            if let Some((_, entries)) = groups.iter_mut().find(|(g, _)| *g == group) {
                entries.push(entry);
            }
        }

        groups
            .into_iter()
            .filter(|(_, entries)| !entries.is_empty())
            .collect()
    }

    pub fn grouped_by_url(&self) -> Vec<(String, Vec<&HistoryEntry>)> {
        use std::collections::HashMap;

        let mut groups: HashMap<String, Vec<&HistoryEntry>> = HashMap::new();

        for entry in &self.entries {
            let domain = Self::extract_domain(&entry.request.url);
            groups.entry(domain).or_default().push(entry);
        }

        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups.sort_by(|(a, _), (b, _)| a.cmp(b));
        sorted_groups
    }

    fn extract_domain(url: &str) -> String {
        let url = url
            .trim_start_matches("https://")
            .trim_start_matches("http://");

        url.split('/').next().unwrap_or("Unknown").to_string()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn toggle_group_collapsed(&mut self, group: TimeGroup, cx: &mut Context<Self>) {
        if self.collapsed_groups.contains(&group) {
            self.collapsed_groups.remove(&group);
        } else {
            self.collapsed_groups.insert(group);
        }
        cx.notify();
    }

    pub fn is_group_collapsed(&self, group: &TimeGroup) -> bool {
        self.collapsed_groups.contains(group)
    }
}

impl Default for HistoryEntity {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter<HistoryEvent> for HistoryEntity {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{Header, HttpMethod, RequestBody};

    fn sample_request(name: &str, url: &str, method: HttpMethod) -> RequestData {
        RequestData {
            id: Uuid::new_v4(),
            name: name.to_string(),
            url: url.to_string(),
            method,
            headers: vec![Header::new("Accept", "application/json")],
            body: RequestBody::None,
            is_sending: false,
        }
    }

    fn sample_entry(name: &str, url: &str, method: HttpMethod) -> HistoryEntry {
        HistoryEntry {
            id: Uuid::new_v4(),
            request: sample_request(name, url, method),
            response: None,
            timestamp: Utc::now(),
            starred: false,
        }
    }

    #[test]
    fn display_name_prefers_request_name_and_falls_back_to_trimmed_url() {
        let named = sample_entry(
            "List Users",
            "https://api.example.com/users",
            HttpMethod::Get,
        );
        let unnamed = sample_entry(
            "New Request",
            "https://api.example.com/users",
            HttpMethod::Get,
        );
        let empty = sample_entry("New Request", "", HttpMethod::Get);

        assert_eq!(named.display_name(), "List Users");
        assert_eq!(unnamed.display_name(), "api.example.com/users");
        assert_eq!(empty.display_name(), "Untitled Request");
    }

    #[test]
    fn search_matches_name_url_and_method_case_insensitively() {
        let history = HistoryEntity {
            entries: vec![
                sample_entry(
                    "List Users",
                    "https://api.example.com/users",
                    HttpMethod::Get,
                ),
                sample_entry(
                    "Create Team",
                    "https://admin.example.com/teams",
                    HttpMethod::Post,
                ),
            ],
            max_entries: 500,
            storage_path: None,
            collapsed_groups: HashSet::new(),
        };

        assert_eq!(history.search("users").len(), 1);
        assert_eq!(history.search("ADMIN.EXAMPLE.COM").len(), 1);
        assert_eq!(history.search("post").len(), 1);
    }

    #[test]
    fn grouped_by_url_uses_domain_and_returns_sorted_groups() {
        let history = HistoryEntity {
            entries: vec![
                sample_entry(
                    "Second",
                    "https://beta.example.com/projects",
                    HttpMethod::Get,
                ),
                sample_entry("First", "https://alpha.example.com/users", HttpMethod::Get),
                sample_entry("Third", "https://alpha.example.com/teams", HttpMethod::Post),
            ],
            max_entries: 500,
            storage_path: None,
            collapsed_groups: HashSet::new(),
        };

        let grouped = history.grouped_by_url();

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, "alpha.example.com");
        assert_eq!(grouped[0].1.len(), 2);
        assert_eq!(grouped[1].0, "beta.example.com");
        assert_eq!(grouped[1].1.len(), 1);
    }

    #[test]
    fn extract_domain_strips_scheme_and_path() {
        assert_eq!(
            HistoryEntity::extract_domain("https://api.example.com/users?page=1"),
            "api.example.com"
        );
        assert_eq!(
            HistoryEntity::extract_domain("http://localhost:3000/v1/health"),
            "localhost:3000"
        );
        assert_eq!(
            HistoryEntity::extract_domain("example.com/path"),
            "example.com"
        );
    }
}
