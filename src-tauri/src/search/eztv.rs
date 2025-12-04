use crate::search::{SearchProvider, SearchResult, parse_audio_codec};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::error::Error;
use regex::Regex;

#[derive(Debug, Deserialize)]
struct EZTVResponse {
    #[serde(default)]
    torrents: Vec<EZTVTorrent>,
    #[serde(default)]
    torrents_count: u32,
}

#[derive(Debug, Deserialize)]
struct EZTVTorrent {
    title: String,
    magnet_url: String,
    #[serde(default)]
    size_bytes: String,
    #[serde(default)]
    seeds: u32,
    #[serde(default)]
    peers: u32,
    #[serde(default)]
    season: String,
    #[serde(default)]
    episode: String,
}

pub struct EZTVProvider {
    client: Client,
    season_regex: Regex,
    episode_regex: Regex,
    quality_regex: Regex,
    encode_regex: Regex,
}

impl EZTVProvider {
    pub fn new() -> Self {
        EZTVProvider {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            season_regex: Regex::new(r"(?i)S(\d+)").unwrap(),
            episode_regex: Regex::new(r"(?i)E(\d+)").unwrap(),
            quality_regex: Regex::new(r"(?i)(\d{3,4}p|4K|2160p|1080p|720p|480p)").unwrap(),
            encode_regex: Regex::new(r"(?i)(x264|x265|H\.?264|H\.?265|HEVC|AVC)").unwrap(),
        }
    }
    
    fn parse_metadata(&self, title: &str, api_season: &str, api_episode: &str) -> (Option<u32>, Option<u32>, Option<String>, Option<String>, bool) {
        // Try API fields first, then parse from title
        let season = api_season.parse::<u32>().ok()
            .or_else(|| self.season_regex.captures(title)
                .and_then(|cap| cap.get(1))
                .and_then(|m| m.as_str().parse().ok()));
        
        let episode = api_episode.parse::<u32>().ok()
            .or_else(|| self.episode_regex.captures(title)
                .and_then(|cap| cap.get(1))
                .and_then(|m| m.as_str().parse().ok()));
        
        let quality = self.quality_regex.captures(title)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_uppercase());
        
        let encode = self.encode_regex.captures(title)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_uppercase());
        
        // EZTV generally has single episodes, not batches
        let is_batch = title.to_lowercase().contains("complete") || 
                       title.to_lowercase().contains("season pack");
        
        (season, episode, quality, encode, is_batch)
    }
    
    fn format_size(bytes_str: &str) -> String {
        if let Ok(bytes) = bytes_str.parse::<u64>() {
            if bytes >= 1_073_741_824 {
                format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
            } else if bytes >= 1_048_576 {
                format!("{:.2} MB", bytes as f64 / 1_048_576.0)
            } else {
                format!("{:.2} KB", bytes as f64 / 1024.0)
            }
        } else {
            "Unknown".to_string()
        }
    }
    
    /// Search EZTV by IMDB ID (preferred method for TV shows)
    /// The imdb_id should be just the numeric part (e.g., "6048596" not "tt6048596")
    pub async fn search_by_imdb(&self, imdb_id: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        // Strip "tt" prefix if present
        let clean_id = imdb_id.trim_start_matches("tt");
        
        let url = format!("https://eztvx.to/api/get-torrents?imdb_id={}&limit=100", clean_id);
        println!("EZTV: Fetching {}", url);
        
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        
        if !status.is_success() {
            println!("EZTV: API returned status {}", status);
            return Ok(vec![]);
        }
        
        let data: EZTVResponse = match response.json().await {
            Ok(d) => d,
            Err(e) => {
                println!("EZTV: Failed to parse JSON: {}", e);
                return Ok(vec![]);
            }
        };
        
        println!("EZTV: Found {} torrents", data.torrents_count);
        
        let mut results = Vec::new();
        for torrent in data.torrents.iter() {
            let (season, episode, quality, encode, is_batch) = 
                self.parse_metadata(&torrent.title, &torrent.season, &torrent.episode);
            let audio_codec = parse_audio_codec(&torrent.title);
            
            results.push(SearchResult {
                title: torrent.title.clone(),
                size: Self::format_size(&torrent.size_bytes),
                seeds: torrent.seeds,
                peers: torrent.peers,
                magnet_link: torrent.magnet_url.clone(),
                provider: "EZTV".to_string(),
                season,
                episode,
                quality,
                encode,
                is_batch,
                audio_codec,
            });
        }
        
        // Sort by seeds descending
        results.sort_by(|a, b| b.seeds.cmp(&a.seeds));
        
        Ok(results)
    }
}

#[async_trait]
impl SearchProvider for EZTVProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        // EZTV API doesn't support text search, only IMDB ID lookup
        // This method is called when no IMDB ID is available
        // We'll just return empty and log a message
        println!("EZTV: Text search not supported. Query was: '{}'. Use search_by_imdb with IMDB ID instead.", query);
        Ok(vec![])
    }
}
