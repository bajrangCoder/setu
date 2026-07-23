use chrono::{DateTime, Utc};
use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::utils::{DebouncedJsonWriter, shared_tokio_runtime};

use super::{HttpMethod, RequestData, ResponseData, SidebarLoadState, default_workspace_id};

const HISTORY_STORAGE_VERSION: u32 = 2;
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryStore {
    version: u32,
    workspaces: HashMap<Uuid, Vec<HistoryEntry>>,
}

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
    Entry(HistoryRowEntry),
}

/// Lightweight render data produced away from the GPUI thread.
///
/// Keeping response bodies and request headers out of this type avoids cloning
/// complete history entries merely to display a single sidebar row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRowEntry {
    pub id: Uuid,
    pub method: HttpMethod,
    pub url_display: String,
    pub full_timestamp: String,
    pub starred: bool,
}

impl HistoryRowEntry {
    fn from_entry(entry: &HistoryEntry) -> Self {
        Self {
            id: entry.id,
            method: entry.request.method,
            url_display: if entry.request.url.is_empty() {
                "No URL".to_string()
            } else {
                entry.request.url.clone()
            },
            full_timestamp: entry.timestamp.format("%b %d, %Y at %H:%M:%S").to_string(),
            starred: entry.starred,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryGrouping {
    Time,
    Url,
}

/// Immutable, cheaply cloned input for background history filtering/grouping.
#[derive(Clone)]
pub struct HistoryRowsSnapshot {
    entries: Arc<Vec<Arc<HistoryEntry>>>,
    collapsed_groups: Arc<HashSet<TimeGroup>>,
    collapsed_url_groups: Arc<HashSet<String>>,
}

impl HistoryRowsSnapshot {
    pub fn flattened_rows(
        &self,
        query: &str,
        starred_only: bool,
        grouping: HistoryGrouping,
    ) -> Vec<HistoryRow> {
        let query = query.trim().to_ascii_lowercase();
        let matches = |entry: &HistoryEntry| {
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
                for entry in self.entries.iter().map(Arc::as_ref) {
                    if !matches(entry) {
                        continue;
                    }
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
                    let collapsed = self.collapsed_groups.contains(&group);
                    rows.push(HistoryRow::Group {
                        key: HistoryGroupKey::Time(group),
                        count: entries.len(),
                        collapsed,
                    });
                    if !collapsed {
                        rows.extend(
                            entries
                                .into_iter()
                                .map(HistoryRowEntry::from_entry)
                                .map(HistoryRow::Entry),
                        );
                    }
                }
                rows
            }
            HistoryGrouping::Url => {
                let mut groups: BTreeMap<String, Vec<&HistoryEntry>> = BTreeMap::new();
                for entry in self.entries.iter().map(Arc::as_ref) {
                    if !matches(entry) {
                        continue;
                    }
                    groups
                        .entry(HistoryEntity::extract_domain(&entry.request.url))
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
                        rows.extend(
                            entries
                                .into_iter()
                                .map(HistoryRowEntry::from_entry)
                                .map(HistoryRow::Entry),
                        );
                    }
                }
                rows
            }
        }
    }
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
    Reloaded,
    GroupingChanged,
}

pub struct HistoryEntity {
    pub entries: Arc<Vec<Arc<HistoryEntry>>>,
    pub load_state: SidebarLoadState,
    pub max_entries: usize,
    active_workspace_id: Uuid,
    workspace_entries: HashMap<Uuid, Arc<Vec<Arc<HistoryEntry>>>>,
    persistor: Option<DebouncedJsonWriter<HistoryStore>>,
    collapsed_groups: Arc<HashSet<TimeGroup>>,
    collapsed_url_groups: Arc<HashSet<String>>,
}

impl HistoryEntity {
    pub fn new() -> Self {
        Self::new_for_workspace(default_workspace_id())
    }

    pub fn new_for_workspace(active_workspace_id: Uuid) -> Self {
        let storage_path = Self::get_storage_path();
        let entity = Self {
            entries: Arc::new(Vec::new()),
            load_state: SidebarLoadState::Loading,
            max_entries: 5_000,
            active_workspace_id,
            workspace_entries: HashMap::new(),
            persistor: storage_path
                .clone()
                .map(|path| DebouncedJsonWriter::new("history", path, SAVE_DEBOUNCE)),
            collapsed_groups: Arc::new(HashSet::new()),
            collapsed_url_groups: Arc::new(HashSet::new()),
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
    -> tokio::sync::oneshot::Receiver<Result<(HashMap<Uuid, Vec<HistoryEntry>>, bool), String>>
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let path = Self::get_storage_path();
        shared_tokio_runtime().spawn(async move {
            let result = async {
                let Some(path) = path else {
                    return Ok((HashMap::new(), false));
                };
                match tokio::fs::read_to_string(&path).await {
                    Ok(contents) => {
                        deserialize_history_store(&contents).map_err(|error| error.to_string())
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Ok((HashMap::new(), false))
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
        result: Result<(HashMap<Uuid, Vec<HistoryEntry>>, bool), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok((mut workspaces, migrated)) => {
                self.entries = Arc::new(
                    workspaces
                        .remove(&self.active_workspace_id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(Arc::new)
                        .collect(),
                );
                self.workspace_entries = workspaces
                    .into_iter()
                    .map(|(workspace_id, entries)| {
                        (
                            workspace_id,
                            Arc::new(entries.into_iter().map(Arc::new).collect()),
                        )
                    })
                    .collect();
                self.load_state = SidebarLoadState::Ready;
                if migrated {
                    self.save_to_file();
                }
                cx.emit(HistoryEvent::Reloaded);
            }
            Err(error) => self.load_state = SidebarLoadState::Error(error.into()),
        }
        cx.notify();
    }

    fn save_to_file(&self) {
        if let Some(persistor) = &self.persistor {
            let mut workspaces: HashMap<_, Vec<_>> = self
                .workspace_entries
                .iter()
                .map(|(workspace_id, entries)| {
                    (
                        *workspace_id,
                        entries.iter().map(|entry| (**entry).clone()).collect(),
                    )
                })
                .collect();
            workspaces.insert(
                self.active_workspace_id,
                self.entries.iter().map(|entry| (**entry).clone()).collect(),
            );
            persistor.schedule_save(HistoryStore {
                version: HISTORY_STORAGE_VERSION,
                workspaces,
            });
        }
    }

    pub fn set_active_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if workspace_id == self.active_workspace_id {
            return;
        }
        let previous = std::mem::replace(&mut self.entries, Arc::new(Vec::new()));
        self.workspace_entries
            .insert(self.active_workspace_id, previous);
        self.active_workspace_id = workspace_id;
        self.entries = self
            .workspace_entries
            .remove(&workspace_id)
            .unwrap_or_else(|| Arc::new(Vec::new()));
        self.collapsed_groups = Arc::new(HashSet::new());
        self.collapsed_url_groups = Arc::new(HashSet::new());
        self.save_to_file();
        cx.emit(HistoryEvent::Reloaded);
        cx.notify();
    }

    pub fn remove_workspace(&mut self, workspace_id: Uuid) {
        if workspace_id != self.active_workspace_id {
            self.workspace_entries.remove(&workspace_id);
            self.save_to_file();
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

        let entries = Arc::make_mut(&mut self.entries);
        entries.insert(0, Arc::new(entry));

        if entries.len() > self.max_entries {
            entries.pop();
        }

        self.save_to_file();
        cx.emit(HistoryEvent::EntryAdded(id));
        cx.notify();
    }

    pub fn remove_entry(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let entries = Arc::make_mut(&mut self.entries);
        if let Some(pos) = entries.iter().position(|e| e.id == id) {
            entries.remove(pos);
            self.save_to_file();
            cx.emit(HistoryEvent::EntryRemoved(id));
            cx.notify();
        }
    }

    pub fn toggle_star(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let entries = Arc::make_mut(&mut self.entries);
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            let entry = Arc::make_mut(entry);
            entry.starred = !entry.starred;
            self.save_to_file();
            cx.emit(HistoryEvent::EntryUpdated(id));
            cx.notify();
        }
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        Arc::make_mut(&mut self.entries).clear();
        self.save_to_file();
        cx.emit(HistoryEvent::Cleared);
        cx.notify();
    }

    pub fn clear_unstarred(&mut self, cx: &mut Context<Self>) {
        Arc::make_mut(&mut self.entries).retain(|e| e.starred);
        self.save_to_file();
        cx.emit(HistoryEvent::Cleared);
        cx.notify();
    }

    pub fn get_entry(&self, id: Uuid) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id).map(Arc::as_ref)
    }

    #[cfg(test)]
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let query = query.to_lowercase();
        self.entries
            .iter()
            .map(Arc::as_ref)
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

        for entry in self.entries.iter() {
            let domain = Self::extract_domain(&entry.request.url);
            groups.entry(domain).or_default().push(entry.as_ref());
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
        let collapsed_groups = Arc::make_mut(&mut self.collapsed_groups);
        if collapsed_groups.contains(&group) {
            collapsed_groups.remove(&group);
        } else {
            collapsed_groups.insert(group);
        }
        cx.emit(HistoryEvent::GroupingChanged);
        cx.notify();
    }

    pub fn toggle_url_group_collapsed(&mut self, domain: &str, cx: &mut Context<Self>) {
        let collapsed_url_groups = Arc::make_mut(&mut self.collapsed_url_groups);
        if !collapsed_url_groups.remove(domain) {
            collapsed_url_groups.insert(domain.to_string());
        }
        cx.emit(HistoryEvent::GroupingChanged);
        cx.notify();
    }

    #[cfg(test)]
    fn flattened_rows(
        &self,
        query: &str,
        starred_only: bool,
        grouping: HistoryGrouping,
    ) -> Vec<HistoryRow> {
        self.rows_snapshot()
            .flattened_rows(query, starred_only, grouping)
    }

    pub fn rows_snapshot(&self) -> HistoryRowsSnapshot {
        HistoryRowsSnapshot {
            entries: self.entries.clone(),
            collapsed_groups: self.collapsed_groups.clone(),
            collapsed_url_groups: self.collapsed_url_groups.clone(),
        }
    }
}

fn deserialize_history_store(
    contents: &str,
) -> Result<(HashMap<Uuid, Vec<HistoryEntry>>, bool), serde_json::Error> {
    let (mut workspaces, mut migrated) =
        if let Ok(store) = serde_json::from_str::<HistoryStore>(contents) {
            if store.version == HISTORY_STORAGE_VERSION {
                (store.workspaces, false)
            } else {
                (HashMap::new(), true)
            }
        } else {
            let entries = serde_json::from_str::<Vec<HistoryEntry>>(contents)?;
            (HashMap::from([(default_workspace_id(), entries)]), true)
        };

    for entries in workspaces.values_mut() {
        for entry in entries {
            if let Some(response) = entry.response.as_mut() {
                migrated |= response.compact_storage();
            }
        }
    }

    Ok((workspaces, migrated))
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
            entries: Arc::new(vec![
                Arc::new(sample_entry(
                    "List Users",
                    "https://api.example.com/users",
                    HttpMethod::Get,
                )),
                Arc::new(sample_entry(
                    "Create Team",
                    "https://admin.example.com/teams",
                    HttpMethod::Post,
                )),
            ]),
            max_entries: 500,
            load_state: SidebarLoadState::Ready,
            active_workspace_id: default_workspace_id(),
            workspace_entries: HashMap::new(),
            persistor: None,
            collapsed_groups: Arc::new(HashSet::new()),
            collapsed_url_groups: Arc::new(HashSet::new()),
        };

        assert_eq!(history.search("users").len(), 1);
        assert_eq!(history.search("ADMIN.EXAMPLE.COM").len(), 1);
        assert_eq!(history.search("post").len(), 1);
    }

    #[test]
    fn grouped_by_url_uses_domain_and_returns_sorted_groups() {
        let history = HistoryEntity {
            entries: Arc::new(vec![
                Arc::new(sample_entry(
                    "Second",
                    "https://beta.example.com/projects",
                    HttpMethod::Get,
                )),
                Arc::new(sample_entry(
                    "First",
                    "https://alpha.example.com/users",
                    HttpMethod::Get,
                )),
                Arc::new(sample_entry(
                    "Third",
                    "https://alpha.example.com/teams",
                    HttpMethod::Post,
                )),
            ]),
            max_entries: 500,
            load_state: SidebarLoadState::Ready,
            active_workspace_id: default_workspace_id(),
            workspace_entries: HashMap::new(),
            persistor: None,
            collapsed_groups: Arc::new(HashSet::new()),
            collapsed_url_groups: Arc::new(HashSet::new()),
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
            entries: Arc::new(vec![
                Arc::new(sample_entry(
                    "List Users",
                    "https://api.example.com/users",
                    HttpMethod::Get,
                )),
                Arc::new(sample_entry(
                    "Create Team",
                    "https://admin.example.com/teams",
                    HttpMethod::Post,
                )),
            ]),
            load_state: SidebarLoadState::Ready,
            max_entries: 5_000,
            active_workspace_id: default_workspace_id(),
            workspace_entries: HashMap::new(),
            persistor: None,
            collapsed_groups: Arc::new(HashSet::new()),
            collapsed_url_groups: Arc::new(HashSet::new()),
        };
        let rows = history.flattened_rows("users", false, HistoryGrouping::Url);
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], HistoryRow::Group { count: 1, .. }));
        assert!(matches!(rows[1], HistoryRow::Entry(_)));
    }

    #[test]
    fn url_group_collapse_hides_descendants() {
        let history = HistoryEntity {
            entries: Arc::new(vec![Arc::new(sample_entry(
                "List Users",
                "https://api.example.com/users",
                HttpMethod::Get,
            ))]),
            load_state: SidebarLoadState::Ready,
            max_entries: 5_000,
            active_workspace_id: default_workspace_id(),
            workspace_entries: HashMap::new(),
            persistor: None,
            collapsed_groups: Arc::new(HashSet::new()),
            collapsed_url_groups: Arc::new(HashSet::from(["api.example.com".to_string()])),
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
    fn shared_entries_keep_the_existing_json_storage_shape() {
        let entry = sample_entry(
            "List Users",
            "https://api.example.com/users",
            HttpMethod::Get,
        );
        let entry_id = entry.id;
        let entries = Arc::new(vec![Arc::new(entry)]);

        let encoded = serde_json::to_string(&entries).expect("serialize shared history");
        let decoded =
            serde_json::from_str::<Vec<HistoryEntry>>(&encoded).expect("decode stored history");

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].id, entry_id);
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
