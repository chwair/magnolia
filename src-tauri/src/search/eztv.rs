use crate::search::{SearchProvider, SearchResult};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::error::Error;

#[derive(Debug, Deserialize)]
struct EZTVResponse {
    torrents: Vec<EZTVTorrent>,
}

#[derive(Debug, Deserialize)]
struct EZTVTorrent {
    title: String,
    magnet_url: String,
    size_bytes: String,
}

pub struct EZTVProvider {
    client: Client,
}

impl EZTVProvider {
    pub fn new() -> Self {
        EZTVProvider {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }
    
    fn parse_metadata(&self, title: &str) -> (Option<u32>, Option<u32>, Option<String>, Option<String>, bool) {
        let title_lower = title.to_lowercase();
        
        let season_regex = regex::Regex::new(r"s(\d+)").unwrap();
        let episode_regex = regex::Regex::new(r"e(\d+)").unwrap();
        
        let season = season_regex.captures(&title_lower)
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse().ok());
        
        let episode = episode_regex.captures(&title_lower)
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse().ok());
        
        let quality = if title_lower.contains("2160p") || title_lower.contains("4k") {
            Some("2160p".to_string())
        } else if title_lower.contains("1080p") {
            Some("1080p".to_string())
        } else if title_lower.contains("720p") {
            Some("720p".to_string())
        } else if title_lower.contains("480p") {
            Some("480p".to_string())
        } else {
            None
        };
        
        let encode = if title_lower.contains("x265") || title_lower.contains("hevc") {
            Some("x265".to_string())
        } else if title_lower.contains("x264") {
            Some("x264".to_string())
        } else if title_lower.contains("xvid") {
            Some("XviD".to_string())
        } else {
            None
        };
        
        let is_batch = false;
        
        (season, episode, quality, encode, is_batch)
    }
    
    fn format_size(bytes_str: &str) -> String {
        if let Ok(bytes) = bytes_str.parse::<u64>() {
            if bytes >= 1_073_741_824 {
                format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
            } else {
                format!("{:.2} MB", bytes as f64 / 1_048_576.0)
            }
        } else {
            "Unknown".to_string()
        }
    }
    
    pub async fn search_by_imdb(&self, imdb_id: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        let url = format!("https://eztvx.to/api/get-torrents?imdb_id={}", imdb_id);
        println!("Fetching EZTV torrents from: {}", url);
        
        let response = self.client.get(&url).send().await?;
        let data: EZTVResponse = response.json().await?;
        
        let mut results = Vec::new();
        for torrent in data.torrents.iter().take(20) {
            let (season, episode, quality, encode, is_batch) = self.parse_metadata(&torrent.title);
            
            results.push(SearchResult {
                title: torrent.title.clone(),
                size: Self::format_size(&torrent.size_bytes),
                seeds: 0,
                peers: 0,
                magnet_link: torrent.magnet_url.clone(),
                provider: "eztv".to_string(),
                season,
                episode,
                quality,
                encode,
                is_batch,
                audio_codec: None,
            });
        }
        
        Ok(results)
    }
}

#[async_trait]
impl SearchProvider for EZTVProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        println!("EZTV provider search called with query: {} (IMDb ID required via search_by_imdb)", query);
        Ok(vec![])
    }
}
