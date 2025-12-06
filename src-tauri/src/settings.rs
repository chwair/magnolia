use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub external_player: String,
    pub remember_preferences: bool,
    pub show_skip_prompts: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            external_player: "mpv".to_string(),
            remember_preferences: true,
            show_skip_prompts: true,
        }
    }
}

pub struct SettingsManager {
    file_path: PathBuf,
    data: Arc<RwLock<Settings>>,
}

impl SettingsManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let file_path = app_data_dir.join("settings.json");
        let data = if file_path.exists() {
            let content = fs::read_to_string(&file_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Settings::default()
        };

        Self {
            file_path,
            data: Arc::new(RwLock::new(data)),
        }
    }

    pub async fn save(&self, settings: Settings) {
        let mut data = self.data.write().await;
        *data = settings;

        if let Ok(content) = serde_json::to_string_pretty(&*data) {
            let _ = fs::write(&self.file_path, content);
        }
    }

    pub async fn get(&self) -> Settings {
        let data = self.data.read().await;
        data.clone()
    }
}
