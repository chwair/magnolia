pub mod nyaa;
pub mod x1337;
pub mod piratebay;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub size: String,
    pub seeds: u32,
    pub peers: u32,
    pub magnet_link: String,
    pub provider: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub quality: Option<String>,
    pub encode: Option<String>,
    pub is_batch: bool,
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>;
}
