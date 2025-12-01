use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use sha2::{Sha256, Digest};

#[derive(Clone, Copy, Debug)]
pub enum TrackType {
    Subtitle,
    Audio,
}

impl TrackType {
    fn folder_name(&self) -> &str {
        match self {
            TrackType::Subtitle => "subtitles",
            TrackType::Audio => "audio",
        }
    }
}

pub struct MediaCache {
    base_dir: PathBuf,
    _lock: Arc<RwLock<()>>,
}

impl MediaCache {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            base_dir: app_data_dir,
            _lock: Arc::new(RwLock::new(())),
        }
    }

    fn get_cache_dir(&self, track_type: TrackType) -> PathBuf {
        let cache_dir = self.base_dir.join(track_type.folder_name());
        if !cache_dir.exists() {
            let _ = fs::create_dir_all(&cache_dir);
        }
        cache_dir
    }

    fn get_cache_path(&self, track_type: TrackType, cache_id: &str, file_index: usize, track_index: usize) -> PathBuf {
        let filename = format!("{}_{}_{}_{}.cache", 
            cache_id, 
            file_index, 
            track_index,
            self.hash_key(cache_id, file_index, track_index)
        );
        self.get_cache_dir(track_type).join(filename)
    }

    fn hash_key(&self, cache_id: &str, file_index: usize, track_index: usize) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}-{}-{}", cache_id, file_index, track_index));
        let result = hasher.finalize();
        format!("{:x}", result)[..8].to_string()
    }

    pub async fn save_track(&self, track_type: TrackType, cache_id: &str, file_index: usize, track_index: usize, data: Vec<u8>) -> Result<(), String> {
        let path = self.get_cache_path(track_type, cache_id, file_index, track_index);
        fs::write(&path, data).map_err(|e| format!("Failed to save track cache: {}", e))?;
        println!("[{:?} Cache] Saved to {:?}", match track_type {
            TrackType::Subtitle => "Subtitle",
            TrackType::Audio => "Audio",
        }, path);
        Ok(())
    }

    pub async fn load_track(&self, track_type: TrackType, cache_id: &str, file_index: usize, track_index: usize) -> Result<Option<Vec<u8>>, String> {
        let path = self.get_cache_path(track_type, cache_id, file_index, track_index);
        if path.exists() {
            let data = fs::read(&path).map_err(|e| format!("Failed to load track cache: {}", e))?;
            println!("[{:?} Cache] Loaded {} bytes from {:?}", match track_type {
                TrackType::Subtitle => "Subtitle",
                TrackType::Audio => "Audio",
            }, data.len(), path);
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    pub async fn clear_cache(&self, track_type: TrackType) -> Result<(), String> {
        let cache_dir = self.get_cache_dir(track_type);
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)
                .map_err(|e| format!("Failed to clear cache: {}", e))?;
            fs::create_dir_all(&cache_dir)
                .map_err(|e| format!("Failed to recreate cache dir: {}", e))?;
            println!("[{:?} Cache] Cleared all cached tracks", match track_type {
                TrackType::Subtitle => "Subtitle",
                TrackType::Audio => "Audio",
            });
        }
        Ok(())
    }
}
