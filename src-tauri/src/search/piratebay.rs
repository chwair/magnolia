use super::{SearchProvider, SearchResult};
use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
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

    fn parse_size(&self, size_str: &str) -> String {
        // TPB uses format like "1.5 GiB" or "700 MiB"
        let parts: Vec<&str> = size_str.split_whitespace().collect();
        if parts.len() >= 2 {
            format!("{} {}", parts[0], parts[1])
        } else {
            "Unknown".to_string()
        }
    }
}

#[async_trait]
impl SearchProvider for PirateBayProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        let mut results = Vec::new();

        // Try multiple TPB mirrors
        let mirrors = vec![
            "https://thepiratebay10.org",
            "https://thepiratebay0.org",
            "https://tpb.party",
        ];

        for mirror in mirrors {
            println!("TPB: Trying mirror {}", mirror);
            // Fetch 3 pages for more results
            for page in 0..3 {
                let url = format!("{}/search/{}/{}/7/0", mirror, query.replace(" ", "%20"), page);
                println!("TPB: Fetching {}", url);
            
                match self.client.get(&url).send().await {
                    Ok(response) => {
                        println!("TPB: Got response, status: {}", response.status());
                        if let Ok(html) = response.text().await {
                            println!("TPB: Got HTML, length: {}", html.len());
                            let document = Html::parse_document(&html);
                            let row_selector = Selector::parse("#searchResult tbody tr").unwrap();
                            let row_count = document.select(&row_selector).count();
                            println!("TPB: Found {} rows", row_count);
                            let name_selector = Selector::parse("td:nth-child(2) a.detLink").unwrap();
                            let magnet_selector = Selector::parse("td:nth-child(2) a[href^='magnet:']").unwrap();
                            let info_selector = Selector::parse("td:nth-child(2) font.detDesc").unwrap();
                            let seeds_selector = Selector::parse("td:nth-child(3)").unwrap();
                            let peers_selector = Selector::parse("td:nth-child(4)").unwrap();

                            for row in document.select(&row_selector) {
                                let name = match row.select(&name_selector).next() {
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

                                let size = match row.select(&info_selector).next() {
                                    Some(el) => {
                                        let text = el.text().collect::<String>();
                                        if let Some(size_idx) = text.find("Size") {
                                            let size_part = &text[size_idx..];
                                            if let Some(comma_idx) = size_part.find(',') {
                                                let size_str = &size_part[5..comma_idx].trim();
                                                self.parse_size(size_str)
                                            } else {
                                                "Unknown".to_string()
                                            }
                                        } else {
                                            "Unknown".to_string()
                                        }
                                    },
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

                                let (season, episode, quality, encode, is_batch) = self.parse_metadata(&name);

                                results.push(SearchResult {
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
                                });
                            }
                        }
                    }
                    Err(_) => continue, // Try next page or mirror
                }
            }
            
            if !results.is_empty() {
                break; // Got results, stop trying mirrors
            }
        }

        Ok(results)
    }
}
