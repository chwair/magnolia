use super::{SearchProvider, SearchResult, parse_audio_codec};
use async_trait::async_trait;
use reqwest::Client;
use std::error::Error;
use regex::Regex;

pub struct PirateBayProvider {
    client: Client,
    season_regex: Regex,
    episode_regex: Regex,
    quality_regex: Regex,
    encode_regex: Regex,
    batch_regex: Regex,
}

impl PirateBayProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap(),
            season_regex: Regex::new(r"(?i)S(\d{1,2})|Season\s*(\d{1,2})").unwrap(),
            episode_regex: Regex::new(r"(?i)S\d{1,2}E(\d+)|E(\d+)|Episode\s*(\d+)|\s-\s*(\d+)\s*(?:v\d)?").unwrap(),
            quality_regex: Regex::new(r"(?i)(\d{3,4}p|4K|8K|2160p|1440p|1080p|720p|480p)").unwrap(),
            encode_regex: Regex::new(r"(?i)(x264|x265|H\.?264|H\.?265|HEVC|AVC|VP9|AV1)").unwrap(),
            batch_regex: Regex::new(r"(?i)(batch|complete|\d+-\d+|S\d+E\d+-E?\d+)").unwrap(),
        }
    }

    fn parse_metadata(&self, title: &str) -> (Option<u32>, Option<u32>, Option<String>, Option<String>, bool) {
        let season = self.season_regex.captures(title)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .and_then(|m| m.as_str().parse().ok());

        let episode = self.episode_regex.captures(title)
            .and_then(|c| c.get(1).or_else(|| c.get(2)).or_else(|| c.get(3)).or_else(|| c.get(4)))
            .and_then(|m| m.as_str().parse().ok());

        let quality = self.quality_regex.captures(title)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_uppercase());

        let encode = self.encode_regex.captures(title)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_uppercase());

        let mut is_batch = self.batch_regex.is_match(title);

        if season.is_some() && (episode.is_none() || title.to_lowercase().contains("season")) {
            is_batch = true;
        }

        (season, episode, quality, encode, is_batch)
    }
    
    /// Search with optional IMDB ID for prioritization
    /// Results matching the IMDB ID will be boosted to the top
    pub async fn search_with_imdb(&self, query: &str, target_imdb: Option<&str>) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        let mut results = Vec::new();
        let encoded_query = urlencoding::encode(query);
        
        // Use apibay.org API - cat=200 is video category
        let api_url = format!("https://apibay.org/q.php?q={}&cat=200", encoded_query);
        
        println!("TPB: Trying API at {}", api_url);
        
        // Normalize target IMDB (strip "tt" prefix if present)
        let normalized_target_imdb = target_imdb.map(|id| {
            id.trim_start_matches("tt").to_string()
        });
        
        match self.client.get(&api_url).send().await {
            Ok(response) => {
                println!("TPB API: Got response, status: {}", response.status());
                if let Ok(text) = response.text().await {
                    println!("TPB API: Got response, length: {}", text.len());
                    // Parse JSON array of torrents
                    if let Ok(torrents) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                        for torrent in torrents {
                            // Skip "No results" placeholder
                            if torrent.get("id").and_then(|v| v.as_str()) == Some("0") {
                                continue;
                            }
                            
                            let name = torrent.get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            
                            if name.is_empty() || name == "No results returned" {
                                continue;
                            }
                            
                            let info_hash = torrent.get("info_hash")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            
                            if info_hash.is_empty() {
                                continue;
                            }
                            
                            // Get IMDB from result (strip "tt" prefix for comparison)
                            let result_imdb = torrent.get("imdb")
                                .and_then(|v| v.as_str())
                                .map(|s| s.trim_start_matches("tt").to_string())
                                .filter(|s| !s.is_empty());
                            
                            // Check if this result matches target IMDB
                            let matches_imdb = match (&normalized_target_imdb, &result_imdb) {
                                (Some(target), Some(result)) => target == result,
                                _ => false,
                            };
                            
                            // Build magnet link
                            let magnet_link = format!(
                                "magnet:?xt=urn:btih:{}&dn={}",
                                info_hash,
                                urlencoding::encode(&name)
                            );
                            
                            let size_bytes: u64 = torrent.get("size")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);
                            
                            let size = if size_bytes >= 1_073_741_824 {
                                format!("{:.2} GiB", size_bytes as f64 / 1_073_741_824.0)
                            } else if size_bytes >= 1_048_576 {
                                format!("{:.2} MiB", size_bytes as f64 / 1_048_576.0)
                            } else if size_bytes >= 1024 {
                                format!("{:.2} KiB", size_bytes as f64 / 1024.0)
                            } else {
                                format!("{} B", size_bytes)
                            };
                            
                            let seeds: u32 = torrent.get("seeders")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);
                            
                            let peers: u32 = torrent.get("leechers")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);
                            
                            let (season, episode, quality, encode, is_batch) = self.parse_metadata(&name);
                            let audio_codec = parse_audio_codec(&name);

                            results.push((matches_imdb, SearchResult {
                                title: name,
                                size,
                                seeds,
                                peers,
                                magnet_link,
                                provider: "ThePirateBay".to_string(),
                                season,
                                episode,
                                quality,
                                encode,
                                is_batch,
                                audio_codec,
                            }));
                        }
                    }
                }
            }
            Err(e) => {
                println!("TPB API: Error fetching: {}", e);
            }
        }

        // Sort: IMDB matches first, then by seeds descending
        results.sort_by(|a, b| {
            // First compare by IMDB match (matches come first)
            match (a.0, b.0) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                // If same match status, sort by seeds
                _ => b.1.seeds.cmp(&a.1.seeds),
            }
        });
        
        // Extract just the SearchResults
        let final_results: Vec<SearchResult> = results.into_iter().map(|(_, r)| r).collect();
        
        println!("TPB: Returning {} results", final_results.len());
        Ok(final_results)
    }
}

#[async_trait]
impl SearchProvider for PirateBayProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        // Default search without IMDB prioritization
        self.search_with_imdb(query, None).await
    }
}
