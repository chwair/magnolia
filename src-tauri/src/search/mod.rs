pub mod nyaa;
pub mod limetorrents;
pub mod piratebay;
pub mod eztv;

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
    pub audio_codec: Option<String>,
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>;
}

pub fn parse_audio_codec(title: &str) -> Option<String> {
    let title_upper = title.to_uppercase();
    
    // Check for various audio codec patterns
    if title_upper.contains("FLAC") {
        Some("FLAC".to_string())
    } else if title_upper.contains("DTS-HD") || title_upper.contains("DTS-MA") {
        Some("DTS-HD".to_string())
    } else if title_upper.contains("DTS") {
        Some("DTS".to_string())
    } else if title_upper.contains("TRUEHD") || title_upper.contains("TRUE-HD") {
        Some("TrueHD".to_string())
    } else if title_upper.contains("DD+") || title_upper.contains("DDP") || title_upper.contains("E-AC-3") || title_upper.contains("EAC3") {
        Some("E-AC3".to_string())
    } else if title_upper.contains("AC3") || title_upper.contains("AC-3") || title_upper.contains("DD5.1") || title_upper.contains("DD 5.1") || title_upper.contains("DOLBY DIGITAL") {
        Some("AC3".to_string())
    } else if title_upper.contains("AAC") {
        Some("AAC".to_string())
    } else if title_upper.contains("OPUS") {
        Some("Opus".to_string())
    } else if title_upper.contains("VORBIS") {
        Some("Vorbis".to_string())
    } else if title_upper.contains("MP3") {
        Some("MP3".to_string())
    } else {
        None
    }
}

// Check if audio codec is supported by web browsers
// Based on: https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Audio_codecs
#[allow(dead_code)]
pub fn is_web_compatible(codec: Option<&str>) -> bool {
    match codec {
        // Supported web audio codecs
        Some("AAC") | Some("MP3") | Some("Opus") | Some("Vorbis") | Some("FLAC") => true,
        // Unsupported: AC3, E-AC3, DTS, DTS-HD, TrueHD
        Some("AC3") | Some("E-AC3") | Some("DTS") | Some("DTS-HD") | Some("TrueHD") => false,
        // Unknown codecs assumed incompatible
        None => true, // If no codec detected, don't filter out
        _ => false,
    }
}
