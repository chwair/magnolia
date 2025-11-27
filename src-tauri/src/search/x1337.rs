use super::{SearchProvider, SearchResult, parse_audio_codec};
use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
use std::error::Error;
use regex::Regex;

pub struct X1337Provider {
    client: Client,
    season_regex: Regex,
    episode_regex: Regex,
    quality_regex: Regex,
    encode_regex: Regex,
    batch_regex: Regex,
}

impl X1337Provider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(std::time::Duration::from_secs(10))
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
}

#[async_trait]
impl SearchProvider for X1337Provider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        let mut results = Vec::new();

        // Only fetch first page for faster results
        let url = format!("https://1337x.to/search/{}/1/", query.replace(" ", "+"));
        println!("1337x: Fetching {}", url);
        
        let response = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                println!("1337x: Request failed: {}", e);
                return Ok(results);
            }
        };
        
        let html = match response.text().await {
            Ok(h) => h,
            Err(e) => {
                println!("1337x: Failed to get text: {}", e);
                return Ok(results);
            }
        };
        
        println!("1337x: Got response, length: {}", html.len());
        
        // Collect data first, then fetch detail pages
        let pending_results = {
            let document = Html::parse_document(&html);
            
            // Try multiple selectors for better compatibility
            let row_selector = Selector::parse("table.table-list tbody tr").unwrap();
            let name_selector = Selector::parse("td.coll-1 a:nth-of-type(2)").unwrap();
            let seeds_selector = Selector::parse("td.coll-2").unwrap();
            let peers_selector = Selector::parse("td.coll-3").unwrap();
            let size_selector = Selector::parse("td.coll-4").unwrap();

            let mut pending = Vec::new();
            let row_count = document.select(&row_selector).count();
            println!("1337x: Found {} rows", row_count);
            
            // Debug: print the first few rows to see structure
            if row_count == 0 {
                println!("1337x: No rows found with table.table-list selector, checking HTML structure...");
                let all_tables = Selector::parse("table").unwrap();
                let table_count = document.select(&all_tables).count();
                println!("1337x: Found {} tables total", table_count);
            }

                for row in document.select(&row_selector) {
                    let name = match row.select(&name_selector).next() {
                        Some(el) => el.text().collect::<String>().trim().to_string(),
                        None => continue,
                    };

                    let link_path = match row.select(&name_selector).next() {
                        Some(el) => match el.value().attr("href") {
                            Some(href) => href.to_string(),
                            None => continue,
                        },
                        None => continue,
                    };

                    let seeds = match row.select(&seeds_selector).next() {
                        Some(el) => el.text().collect::<String>().trim().parse().unwrap_or(0),
                        None => 0,
                    };

                    let peers = match row.select(&peers_selector).next() {
                        Some(el) => el.text().collect::<String>().trim().parse().unwrap_or(0),
                        None => 0,
                    };

                    let size = match row.select(&size_selector).next() {
                        Some(el) => {
                            let text = el.text().collect::<String>();
                            let parts: Vec<&str> = text.split_whitespace().collect();
                            if parts.len() >= 2 {
                                format!("{} {}", parts[0], parts[1])
                            } else {
                                "Unknown".to_string()
                            }
                        },
                        None => "Unknown".to_string(),
                    };

                    pending.push((name, link_path, seeds, peers, size));
                }
                
                pending
            }; // document is dropped here

            // Sort by seeds and only fetch top 10 to avoid slowdown
            let mut sorted_pending = pending_results;
            sorted_pending.sort_by(|a, b| b.2.cmp(&a.2)); // Sort by seeds (index 2)
            sorted_pending.truncate(10);
            
            println!("1337x: Fetching detail pages for top {} results", sorted_pending.len());

            // Now fetch detail pages without holding document references
            for (i, (name, link_path, seeds, peers, size)) in sorted_pending.into_iter().enumerate() {
                println!("1337x: Fetching detail page {}/{}", i + 1, 10);
                let detail_url = format!("https://1337x.to{}", link_path);
                if let Ok(detail_response) = self.client.get(&detail_url).send().await {
                    if let Ok(detail_html) = detail_response.text().await {
                        let detail_doc = Html::parse_document(&detail_html);
                        let magnet_selector = Selector::parse("a[href^='magnet:']").unwrap();
                        
                        if let Some(magnet_el) = detail_doc.select(&magnet_selector).next() {
                            if let Some(magnet_link) = magnet_el.value().attr("href") {
                                let (season, episode, quality, encode, is_batch) = self.parse_metadata(&name);
                                let audio_codec = parse_audio_codec(&name);

                                results.push(SearchResult {
                                    title: name,
                                    size,
                                    seeds,
                                    peers,
                                    magnet_link: magnet_link.to_string(),
                                    provider: "1337x".to_string(),
                                    season,
                                    episode,
                                    quality,
                                    encode,
                                    is_batch,
                                    audio_codec,
                                });
                            }
                        }
                    }
                }
            }

        Ok(results)
    }
}
