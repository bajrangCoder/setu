use chrono::{DateTime, Utc};
use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

use crate::utils::{DebouncedJsonWriter, shared_tokio_runtime};

use super::{RequestData, ResponseData, SidebarLoadState};

const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryGroupKey {
    Time(TimeGroup),
    Url(String),
}

#[derive(Debug, Clone)]
pub enum HistoryRow {
    Group {
        key: HistoryGroupKey,
        count: usize,
        collapsed: bool,
    },
    Entry(HistoryEntry),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryGrouping {
    Time,
    Url,
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
    pub load_state: SidebarLoadState,
    pub max_entries: usize,
    persistor: Option<DebouncedJsonWriter<Vec<HistoryEntry>>>,
    collapsed_groups: HashSet<TimeGroup>,
    collapsed_url_groups: HashSet<String>,
}

impl HistoryEntity {
    pub fn new() -> Self {
        let storage_path = Self::get_storage_path();
        let entity = Self {
            entries: Vec::new(),
            load_state: SidebarLoadState::Loading,
            max_entries: 5_000,
            persistor: storage_path
                .clone()
                .map(|path| DebouncedJsonWriter::new("history", path, SAVE_DEBOUNCE)),
            collapsed_groups: HashSet::new(),
            collapsed_url_groups: HashSet::new(),
        };

        entity
    }

    fn get_storage_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|mut path| {
            path.push("setu");
            path.push("history.json");
            path
        })
    }

    pub fn spawn_storage_load()
    -> tokio::sync::oneshot::Receiver<Result<(Vec<HistoryEntry>, bool), String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let path = Self::get_storage_path();
        shared_tokio_runtime().spawn(async move {
            let result = async {
                let Some(path) = path else {
                    return Ok((Vec::new(), false));
                };
                match tokio::fs::read_to_string(&path).await {
                    Ok(contents) => {
                        let mut entries = serde_json::from_str::<Vec<HistoryEntry>>(&contents)
                            .map_err(|error| error.to_string())?;
                        let mut compacted = false;
                        for entry in &mut entries {
                            if let Some(response) = entry.response.as_mut() {
                                compacted |= response.compact_storage();
                            }
                        }
                        Ok((entries, compacted))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Ok((Vec::new(), false))
                    }
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
        result: Result<(Vec<HistoryEntry>, bool), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok((entries, compacted)) => {
                self.entries = entries;
                self.load_state = SidebarLoadState::Ready;
                if compacted {
                    self.save_to_file();
                }
            }
            Err(error) => self.load_state = SidebarLoadState::Error(error.into()),
        }
        cx.notify();
    }

    fn save_to_file(&self) {
        if let Some(persistor) = &self.persistor {
            persistor.schedule_save(self.entries.clone());
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    pub fn toggle_url_group_collapsed(&mut self, domain: &str, cx: &mut Context<Self>) {
        if !self.collapsed_url_groups.remove(domain) {
            self.collapsed_url_groups.insert(domain.to_string());
        }
        cx.notify();
    }

    pub fn flattened_rows(
        &self,
        query: &str,
        starred_only: bool,
        grouping: HistoryGrouping,
    ) -> Vec<HistoryRow> {
        let query = query.trim().to_ascii_lowercase();
        let matches = |entry: &&HistoryEntry| {
            (!starred_only || entry.starred)
                && (query.is_empty()
                    || entry.request.url.to_ascii_lowercase().contains(&query)
                    || entry.request.name.to_ascii_lowercase().contains(&query)
                    || entry
                        .request
                        .method
                        .as_str()
                        .to_ascii_lowercase()
                        .contains(&query))
        };

        match grouping {
            HistoryGrouping::Time => {
                let mut groups: [Vec<&HistoryEntry>; 5] = std::array::from_fn(|_| Vec::new());
                for entry in self.entries.iter().filter(matches) {
                    let index = match entry.time_group() {
                        TimeGroup::Today => 0,
                        TimeGroup::ThisWeek => 1,
                        TimeGroup::LastWeek => 2,
                        TimeGroup::ThisMonth => 3,
                        TimeGroup::Older => 4,
                    };
                    groups[index].push(entry);
                }

                let labels = [
                    TimeGroup::Today,
                    TimeGroup::ThisWeek,
                    TimeGroup::LastWeek,
                    TimeGroup::ThisMonth,
                    TimeGroup::Older,
                ];
                let mut rows = Vec::new();
                for (group, entries) in labels.into_iter().zip(groups) {
                    if entries.is_empty() {
                        continue;
                    }
                    let collapsed = self.is_group_collapsed(&group);
                    rows.push(HistoryRow::Group {
                        key: HistoryGroupKey::Time(group),
                        count: entries.len(),
                        collapsed,
                    });
                    if !collapsed {
                        rows.extend(entries.into_iter().cloned().map(HistoryRow::Entry));
                    }
                }
                rows
            }
            HistoryGrouping::Url => {
                let mut groups: BTreeMap<String, Vec<&HistoryEntry>> = BTreeMap::new();
                for entry in self.entries.iter().filter(matches) {
                    groups
                        .entry(Self::extract_domain(&entry.request.url))
                        .or_default()
                        .push(entry);
                }

                let mut rows = Vec::new();
                for (domain, entries) in groups {
                    let collapsed = self.collapsed_url_groups.contains(&domain);
                    rows.push(HistoryRow::Group {
                        key: HistoryGroupKey::Url(domain),
                        count: entries.len(),
                        collapsed,
                    });
                    if !collapsed {
                        rows.extend(entries.into_iter().cloned().map(HistoryRow::Entry));
                    }
                }
                rows
            }
        }
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
            load_state: SidebarLoadState::Ready,
            persistor: None,
            collapsed_groups: HashSet::new(),
            collapsed_url_groups: HashSet::new(),
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
            load_state: SidebarLoadState::Ready,
            persistor: None,
            collapsed_groups: HashSet::new(),
            collapsed_url_groups: HashSet::new(),
        };

        let grouped = history.grouped_by_url();

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, "alpha.example.com");
        assert_eq!(grouped[0].1.len(), 2);
        assert_eq!(grouped[1].0, "beta.example.com");
        assert_eq!(grouped[1].1.len(), 1);
    }

    #[test]
    fn flattened_rows_filter_and_keep_group_headers() {
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
            load_state: SidebarLoadState::Ready,
            max_entries: 5_000,
            persistor: None,
            collapsed_groups: HashSet::new(),
            collapsed_url_groups: HashSet::new(),
        };
        let rows = history.flattened_rows("users", false, HistoryGrouping::Url);
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], HistoryRow::Group { count: 1, .. }));
        assert!(matches!(rows[1], HistoryRow::Entry(_)));
    }

    #[test]
    fn url_group_collapse_hides_descendants() {
        let history = HistoryEntity {
            entries: vec![sample_entry(
                "List Users",
                "https://api.example.com/users",
                HttpMethod::Get,
            )],
            load_state: SidebarLoadState::Ready,
            max_entries: 5_000,
            persistor: None,
            collapsed_groups: HashSet::new(),
            collapsed_url_groups: HashSet::from(["api.example.com".to_string()]),
        };
        let rows = history.flattened_rows("", false, HistoryGrouping::Url);
        assert_eq!(rows.len(), 1);
        assert!(matches!(
            rows[0],
            HistoryRow::Group {
                collapsed: true,
                ..
            }
        ));
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
