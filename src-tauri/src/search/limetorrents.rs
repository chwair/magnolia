use super::{SearchProvider, SearchResult, parse_audio_codec};
use async_trait::async_trait;
use reqwest::Client;
use std::error::Error;
use regex::Regex;

pub struct LimeTorrentsProvider {
    client: Client,
    season_regex: Regex,
    episode_regex: Regex,
    quality_regex: Regex,
    encode_regex: Regex,
    batch_regex: Regex,
    seeds_regex: Regex,
}

impl LimeTorrentsProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap(),
            season_regex: Regex::new(r"(?i)S(\d{1,2})|Season\s*(\d{1,2})").unwrap(),
            episode_regex: Regex::new(r"(?i)S\d{1,2}E(\d+)|E(\d+)|Episode\s*(\d+)|\s-\s*(\d+)\s*(?:v\d)?").unwrap(),
            quality_regex: Regex::new(r"(?i)(\d{3,4}p|4K|8K|2160p|1440p|1080p|720p|480p)").unwrap(),
            encode_regex: Regex::new(r"(?i)(x264|x265|H\.?264|H\.?265|HEVC|AVC|VP9|AV1)").unwrap(),
            batch_regex: Regex::new(r"(?i)(batch|complete|\d+-\d+|S\d+E\d+-E?\d+)").unwrap(),
            seeds_regex: Regex::new(r"Seeds:\s*(\d+)").unwrap(),
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

    fn format_size(bytes: u64) -> String {
        if bytes >= 1_073_741_824 {
            format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
        } else if bytes >= 1_048_576 {
            format!("{:.2} MB", bytes as f64 / 1_048_576.0)
        } else {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        }
    }

    fn parse_seeds_leechers(&self, description: &str) -> (u32, u32) {
        let seeds = self.seeds_regex.captures(description)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        
        let leechers_regex = Regex::new(r"Leechers\s*(\d+)").unwrap();
        let leechers = leechers_regex.captures(description)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        
        (seeds, leechers)
    }
}

#[async_trait]
impl SearchProvider for LimeTorrentsProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        let mut results = Vec::new();
        
        let encoded_query = query.replace(" ", "%20").replace(":", "%3A");
        let url = format!("https://www.limetorrents.fun/searchrss/{}/", encoded_query);
        
        println!("LimeTorrents: Fetching {}", url);
        
        let response = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                println!("LimeTorrents: Request failed: {}", e);
                return Ok(results);
            }
        };
        
        if !response.status().is_success() {
            println!("LimeTorrents: Status {}", response.status());
            return Ok(results);
        }
        
        let xml = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                println!("LimeTorrents: Failed to read body: {}", e);
                return Ok(results);
            }
        };
        
        println!("LimeTorrents: Got RSS feed, length: {}", xml.len());
        
        // Parse XML manually since we don't need a full XML parser
        // Each item looks like:
        // <item>
        //   <title>...</title>
        //   <link>...</link>
        //   <category>Anime</category>
        //   <size>933497354</size>
        //   <description>Seeds: 39 , Leechers 2</description>
        // </item>
        
        let item_regex = Regex::new(r"(?s)<item>(.*?)</item>").unwrap();
        let title_regex = Regex::new(r"<title>(?:<!\[CDATA\[)?(.*?)(?:\]\]>)?</title>").unwrap();
        let link_regex = Regex::new(r"<link>(?:<!\[CDATA\[)?(.*?)(?:\]\]>)?</link>").unwrap();
        let category_regex = Regex::new(r"<category>(?:<!\[CDATA\[)?(.*?)(?:\]\]>)?</category>").unwrap();
        let size_regex = Regex::new(r"<size>(\d+)</size>").unwrap();
        let desc_regex = Regex::new(r"<description>(?:<!\[CDATA\[)?(.*?)(?:\]\]>)?</description>").unwrap();
        
        let allowed_categories = ["Movies", "TV shows", "Anime"];
        
        for item_cap in item_regex.captures_iter(&xml) {
            let item_xml = &item_cap[1];
            
            // Extract category and filter
            let category = category_regex.captures(item_xml)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim())
                .unwrap_or("");
            
            if !allowed_categories.contains(&category) {
                continue;
            }
            
            let title = match title_regex.captures(item_xml) {
                Some(c) => c.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default(),
                None => continue,
            };
            
            if title.is_empty() {
                continue;
            }
            
            let link = link_regex.captures(item_xml)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            
            let size_bytes = size_regex.captures(item_xml)
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse::<u64>().ok())
                .unwrap_or(0);
            
            let description = desc_regex.captures(item_xml)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim())
                .unwrap_or("");
            
            let (seeds, peers) = self.parse_seeds_leechers(description);
            
            // LimeTorrents doesn't provide magnet links in RSS, need to fetch the page
            // For now, store the link and we'll fetch magnet separately
            // Actually, let's try to get the hash from the link URL and construct a magnet
            // Links are like: https://www.limetorrents.fun/NieR-Automata-torrent-12345.html
            // We need to visit the page to get the magnet link
            
            let (season, episode, quality, encode, is_batch) = self.parse_metadata(&title);
            let audio_codec = parse_audio_codec(&title);
            
            results.push(SearchResult {
                title: title.clone(),
                size: Self::format_size(size_bytes),
                seeds,
                peers,
                magnet_link: link, // Temporarily store page link, will fetch magnet below
                provider: "LimeTorrents".to_string(),
                season,
                episode,
                quality,
                encode,
                is_batch,
                audio_codec,
            });
        }
        
        println!("LimeTorrents: Found {} results after category filter", results.len());
        
        // Sort by seeds descending
        results.sort_by(|a, b| b.seeds.cmp(&a.seeds));
        
        // Limit to top 15 and fetch magnet links
        results.truncate(15);
        
        // Fetch magnet links from detail pages
        let mut final_results = Vec::new();
        for result in results {
            if result.magnet_link.starts_with("http") {
                // Need to fetch the detail page to get magnet link
                if let Ok(magnet) = self.fetch_magnet_link(&result.magnet_link).await {
                    let mut r = result;
                    r.magnet_link = magnet;
                    final_results.push(r);
                }
            } else {
                final_results.push(result);
            }
        }
        
        println!("LimeTorrents: Returning {} results with magnet links", final_results.len());
        
        Ok(final_results)
    }
}

impl LimeTorrentsProvider {
    async fn fetch_magnet_link(&self, page_url: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let response = self.client.get(page_url).send().await?;
        
        if !response.status().is_success() {
            return Err("Failed to fetch detail page".into());
        }
        
        let html = response.text().await?;
        
        // Look for magnet link in the page
        let magnet_regex = Regex::new(r#"href=["'](magnet:\?xt=urn:btih:[^"']+)["']"#).unwrap();
        
        if let Some(cap) = magnet_regex.captures(&html) {
            if let Some(magnet) = cap.get(1) {
                return Ok(magnet.as_str().to_string());
            }
        }
        
        Err("No magnet link found on page".into())
    }
}
