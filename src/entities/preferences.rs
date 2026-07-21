use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crate::utils::DebouncedJsonWriter;

const UI_PREFERENCES_VERSION: u32 = 1;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferredLayout {
    #[default]
    Stacked,
    SideBySide,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPreferences {
    pub version: u32,
    pub sidebar_visible: bool,
    pub sidebar_width: f32,
    pub layout: PreferredLayout,
    pub stacked_split: [f32; 2],
    pub side_by_side_split: [f32; 2],
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            version: UI_PREFERENCES_VERSION,
            sidebar_visible: true,
            sidebar_width: 300.0,
            layout: PreferredLayout::Stacked,
            stacked_split: [360.0, 360.0],
            side_by_side_split: [620.0, 620.0],
        }
    }
}

impl UiPreferences {
    pub fn validated(mut self) -> Self {
        if self.version != UI_PREFERENCES_VERSION {
            return Self::default();
        }
        self.sidebar_width = finite_clamp(self.sidebar_width, 200.0, 500.0, 300.0);
        self.stacked_split = validate_split(self.stacked_split, [360.0, 360.0]);
        self.side_by_side_split = validate_split(self.side_by_side_split, [620.0, 620.0]);
        self
    }
}

fn finite_clamp(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn validate_split(split: [f32; 2], fallback: [f32; 2]) -> [f32; 2] {
    if split
        .iter()
        .all(|value| value.is_finite() && *value >= 150.0)
    {
        split
    } else {
        fallback
    }
}

pub struct UiPreferencesStore {
    writer: Option<DebouncedJsonWriter<UiPreferences>>,
}

impl UiPreferencesStore {
    pub fn load() -> (UiPreferences, Self) {
        let path = storage_path();
        let preferences = path
            .as_ref()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|contents| serde_json::from_str::<UiPreferences>(&contents).ok())
            .unwrap_or_default()
            .validated();
        let writer =
            path.map(|path| DebouncedJsonWriter::new("UI preferences", path, SAVE_DEBOUNCE));
        (preferences, Self { writer })
    }

    pub fn save(&self, preferences: &UiPreferences) {
        if let Some(writer) = &self.writer {
            writer.schedule_save(preferences.clone().validated());
        }
    }
}

fn storage_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|mut path| {
        path.push("setu");
        path.push("ui-preferences.json");
        path
    })
}

#[cfg(test)]
mod tests {
    use super::{PreferredLayout, UiPreferences};

    #[test]
    fn validates_and_clamps_preferences() {
        let preferences = UiPreferences {
            sidebar_width: 9_000.0,
            stacked_split: [f32::NAN, 200.0],
            side_by_side_split: [100.0, 100.0],
            layout: PreferredLayout::SideBySide,
            ..UiPreferences::default()
        }
        .validated();

        assert_eq!(preferences.sidebar_width, 500.0);
        assert_eq!(preferences.stacked_split, [360.0, 360.0]);
        assert_eq!(preferences.side_by_side_split, [620.0, 620.0]);
        assert_eq!(preferences.layout, PreferredLayout::SideBySide);
    }

    #[test]
    fn resets_unknown_versions() {
        let preferences = UiPreferences {
            version: 99,
            sidebar_visible: false,
            ..UiPreferences::default()
        }
        .validated();
        assert_eq!(preferences, UiPreferences::default());
    }
}
