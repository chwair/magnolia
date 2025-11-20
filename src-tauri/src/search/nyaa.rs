use super::{SearchProvider, SearchResult};
use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
use std::error::Error;
use regex::Regex;
use serde::Deserialize;

pub struct NyaaProvider {
    client: Client,
    season_regex: Regex,
    episode_regex: Regex,
    quality_regex: Regex,
    encode_regex: Regex,
    batch_regex: Regex,
}

impl NyaaProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            // Updated to capture season in multiple formats including "Season X"
            season_regex: Regex::new(r"(?i)S(\d{1,2})|Season\s*(\d{1,2})").unwrap(),
            // Updated to handle 3+ digit episodes
            episode_regex: Regex::new(r"(?i)S\d{1,2}E(\d+)|E(\d+)|Episode\s*(\d+)|\s-\s*(\d+)\s*(?:v\d)?").unwrap(),
            quality_regex: Regex::new(r"(?i)(\d{3,4}p|4K|8K|2160p|1440p|1080p|720p|480p)").unwrap(),
            encode_regex: Regex::new(r"(?i)(x264|x265|H\.?264|H\.?265|HEVC|AVC|VP9|AV1)").unwrap(),
            batch_regex: Regex::new(r"(?i)(batch|complete|\d+-\d+|S\d+E\d+-E?\d+)").unwrap(),
        }
    }

    fn parse_metadata(&self, title: &str, magnet: &str) -> (Option<u32>, Option<u32>, Option<String>, Option<String>, bool) {
        let mut season = None;
        let mut episode = None;
        let mut quality = None;
        let mut encode = None;
        let mut is_batch = false;

        // Try to extract info hash and fetch torrent metadata first
        if let Some(info_hash) = self.extract_info_hash(magnet) {
            if let Ok(metadata) = self.fetch_torrent_metadata(&info_hash) {
                if let Some((s, e, q, enc, batch)) = self.parse_torrent_metadata(&metadata) {
                    season = s;
                    episode = e;
                    quality = q;
                    encode = enc;
                    is_batch = batch;
                }
            }
        }

        // Use title parsing as fallback if bencode didn't find metadata
        if season.is_none() {
            if let Some(caps) = self.season_regex.captures(title) {
                season = caps.get(1).or_else(|| caps.get(2))
                    .and_then(|m| m.as_str().parse().ok());
            }
        }

        if episode.is_none() {
            if let Some(caps) = self.episode_regex.captures(title) {
                // Try all capture groups for episode number (handles various formats)
                episode = caps.get(1)
                    .or_else(|| caps.get(2))
                    .or_else(|| caps.get(3))
                    .or_else(|| caps.get(4))
                    .and_then(|m| m.as_str().parse().ok());
            }
        }

        if quality.is_none() {
            if let Some(caps) = self.quality_regex.captures(title) {
                quality = Some(caps.get(1).unwrap().as_str().to_uppercase());
            }
        }

        if encode.is_none() {
            if let Some(caps) = self.encode_regex.captures(title) {
                encode = Some(caps.get(1).unwrap().as_str().to_uppercase());
            }
        }

        // Check if it's a batch release from title if not already detected
        if !is_batch {
            is_batch = self.batch_regex.is_match(title);
        }

        // Mark as batch if "Season X" format appears in title (even with episode numbers)
        // This catches torrents like "Season 1" which are always full season packs
        if season.is_some() && title.to_lowercase().contains("season") {
            is_batch = true;
        }

        // Also mark as batch if has season but no episode
        if season.is_some() && episode.is_none() {
            is_batch = true;
        }

        (season, episode, quality, encode, is_batch)
    }

    fn extract_info_hash(&self, magnet: &str) -> Option<String> {
        if let Some(start) = magnet.find("urn:btih:") {
            let hash_start = start + 9;
            let hash_part = &magnet[hash_start..];
            if let Some(end) = hash_part.find('&') {
                Some(hash_part[..end].to_string())
            } else {
                Some(hash_part.to_string())
            }
        } else {
            None
        }
    }

    fn fetch_torrent_metadata(&self, _info_hash: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        // For now, return empty as we'd need to actually fetch .torrent file
        // This would require accessing torrent trackers or DHT
        Err("Metadata fetching not implemented".into())
    }

    fn parse_torrent_metadata(&self, data: &[u8]) -> Option<(Option<u32>, Option<u32>, Option<String>, Option<String>, bool)> {
        #[derive(Debug, Deserialize)]
        struct FileEntry {
            path: Vec<String>,
            length: Option<i64>,
        }

        #[derive(Debug, Deserialize)]
        struct TorrentInfo {
            name: Option<String>,
            files: Option<Vec<FileEntry>>,
        }

        #[derive(Debug, Deserialize)]
        struct TorrentMetadata {
            info: Option<TorrentInfo>,
        }

        if let Ok(metadata) = serde_bencode::from_bytes::<TorrentMetadata>(data) {
            if let Some(info) = metadata.info {
                let name = info.name.as_deref().unwrap_or("");
                
                // Extract video files only
                let video_files: Vec<String> = if let Some(files) = &info.files {
                    files.iter()
                        .filter_map(|f| {
                            let path_str = f.path.join("/");
                            let lower = path_str.to_lowercase();
                            if lower.ends_with(".mkv") || lower.ends_with(".mp4") || 
                               lower.ends_with(".avi") || lower.ends_with(".m4v") {
                                Some(path_str)
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    // Single file torrent
                    if name.to_lowercase().ends_with(".mkv") || 
                       name.to_lowercase().ends_with(".mp4") {
                        vec![name.to_string()]
                    } else {
                        vec![]
                    }
                };

                // Batch detection based on video file count
                let mut is_batch = video_files.len() > 1;

                // Parse metadata from torrent name first
                let season = self.season_regex.captures(name)
                    .and_then(|c| c.get(1).or_else(|| c.get(2)))
                    .and_then(|m| m.as_str().parse().ok());
                    
                let mut episode = self.episode_regex.captures(name)
                    .and_then(|c| c.get(1).or_else(|| c.get(2)).or_else(|| c.get(3)).or_else(|| c.get(4)))
                    .and_then(|m| m.as_str().parse().ok());
                    
                let quality = self.quality_regex.captures(name)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_uppercase());
                    
                let encode = self.encode_regex.captures(name)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_uppercase());

                // If no episode found in name, scan video filenames
                if episode.is_none() && !video_files.is_empty() {
                    for vf in &video_files {
                        if let Some(caps) = self.episode_regex.captures(vf) {
                            episode = caps.get(1)
                                .or_else(|| caps.get(2))
                                .or_else(|| caps.get(3))
                                .or_else(|| caps.get(4))
                                .and_then(|m| m.as_str().parse().ok());
                            if episode.is_some() {
                                break;
                            }
                        }
                    }
                }

                // Additional batch indicators
                if !is_batch {
                    is_batch = self.batch_regex.is_match(name);
                }

                // Mark as batch if season without specific episode
                if season.is_some() && episode.is_none() {
                    is_batch = true;
                }

                return Some((season, episode, quality, encode, is_batch));
            }
        }
        None
    }
}

#[async_trait]
impl SearchProvider for NyaaProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        let row_selector = Selector::parse("tr.default, tr.success, tr.danger").unwrap();
        let title_selector = Selector::parse("td:nth-child(2) a:not(.comments)").unwrap();
        let magnet_selector = Selector::parse("td:nth-child(3) a[href^='magnet:']").unwrap();
        let size_selector = Selector::parse("td:nth-child(4)").unwrap();
        let seeds_selector = Selector::parse("td:nth-child(6)").unwrap();
        let peers_selector = Selector::parse("td:nth-child(7)").unwrap();

        let mut results = Vec::new();

        // Fetch first 3 pages for more results (75 total)
        for page in 1..=3 {
            let url = format!("https://nyaa.si/?f=0&c=1_0&q={}&s=seeders&o=desc&p={}", query, page);
            let response = self.client.get(&url).send().await?.text().await?;
            let document = Html::parse_document(&response);

            for row in document.select(&row_selector) {
            let title = match row.select(&title_selector).next() {
                Some(el) => el.text().collect::<String>().trim().to_string(),
                None => continue,
            };

            let magnet_link = match row.select(&magnet_selector).next() {
                Some(el) => match el.value().attr("href") {
                    Some(href) => href.to_string(),
                    None => continue,
                },
                None => continue,
            };

            let size = match row.select(&size_selector).next() {
                Some(el) => el.text().collect::<String>().trim().to_string(),
                None => "Unknown".to_string(),
            };

            let seeds = match row.select(&seeds_selector).next() {
                Some(el) => el.text().collect::<String>().trim().parse().unwrap_or(0),
                None => 0,
            };

            let peers = match row.select(&peers_selector).next() {
                Some(el) => el.text().collect::<String>().trim().parse().unwrap_or(0),
                None => 0,
            };

            let (season, episode, quality, encode, is_batch) = self.parse_metadata(&title, &magnet_link);

            // Debug logging
            if season.is_some() || episode.is_some() {
                println!("Parsed: {} -> S:{:?} E:{:?} Batch:{}", 
                    title, season, episode, is_batch);
            }

            results.push(SearchResult {
                title,
                size,
                seeds,
                peers,
                magnet_link,
                provider: "Nyaa".to_string(),
                season,
                episode,
                quality,
                encode,
                is_batch,
            });
            }
        }

        Ok(results)
    }
}
