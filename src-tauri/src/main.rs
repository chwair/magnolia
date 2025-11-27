
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod search;
mod torrent;
mod tracking;
mod dash;
mod media_cache;

use search::{nyaa::NyaaProvider, x1337::X1337Provider, piratebay::PirateBayProvider, 
             SearchProvider};
use std::sync::Arc;
use tauri::{Manager, State};
use torrent::TorrentManager;
use tracking::TrackingManager;
use media_cache::{MediaCache, TrackType};

#[tauri::command]
async fn search_nyaa(query: String) -> Result<Vec<search::SearchResult>, String> {
    let provider = NyaaProvider::new();
    provider.search(&query).await.map_err(|e| e.to_string())
}

async fn search_by_media_type(query: &str, media_type: &Option<String>) -> Result<Vec<search::SearchResult>, String> {
    if let Some(ref mt) = media_type {
        println!("Media type: {}", mt);
        if mt == "anime" {
            // Use Nyaa for anime
            println!("Using Nyaa provider for anime");
            let provider = NyaaProvider::new();
            provider.search(query).await.map_err(|e| e.to_string())
        } else {
            // Use 1337x and PirateBay for all non-anime content (movies and TV)
            println!("Using 1337x and ThePirateBay for non-anime");
            let mut all_results = Vec::new();
            
            // Try 1337x
            println!("Searching 1337x...");
            let x1337 = X1337Provider::new();
            match x1337.search(query).await {
                Ok(mut results) => {
                    println!("1337x returned {} results", results.len());
                    all_results.append(&mut results);
                }
                Err(e) => {
                    println!("1337x error: {}", e);
                }
            }
            
            // Add PirateBay results
            println!("Searching ThePirateBay...");
            let tpb = PirateBayProvider::new();
            match tpb.search(query).await {
                Ok(mut results) => {
                    println!("ThePirateBay returned {} results", results.len());
                    all_results.append(&mut results);
                }
                Err(e) => {
                    println!("ThePirateBay error: {}", e);
                }
            }
            
            Ok(all_results)
        }
    } else {
        // Default to Nyaa if no media type specified
        let provider = NyaaProvider::new();
        provider.search(query).await.map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn search_nyaa_filtered(
    query: String,
    season: Option<u32>,
    episode: Option<u32>,
    is_movie: bool,
    media_type: Option<String>, // "anime", "tv", "movie"
    tracker_preference: Option<Vec<String>>, // ["nyaa", "1337x", ...] or None for auto
) -> Result<Vec<search::SearchResult>, String> {
    println!("search_nyaa_filtered called with tracker_preference: {:?}", tracker_preference);
    
    // Normalize query
    let normalized_query = query
        .replace("-", " ")
        .replace(":", " ")
        .replace("_", " ");
    
    // Determine which trackers to use
    let trackers: Vec<String> = if let Some(prefs) = tracker_preference {
        if prefs.is_empty() {
            // Empty array means auto mode
            match media_type.as_deref() {
                Some("anime") => vec!["nyaa".to_string()],
                _ => vec!["1337x".to_string(), "thepiratebay".to_string()],
            }
        } else {
            // Use specified trackers
            prefs
        }
    } else {
        // null/undefined means auto mode
        match media_type.as_deref() {
            Some("anime") => vec!["nyaa".to_string()],
            _ => vec!["1337x".to_string(), "thepiratebay".to_string()],
        }
    };
    
    println!("Using trackers: {:?}", trackers);
    
    // Search all trackers in parallel
    let mut handles = vec![];
    
    for tracker in trackers {
        // Skip EZTV as it requires IMDb ID (use search_eztv_by_imdb instead)
        if tracker == "eztv" {
            println!("Skipping EZTV in regular search (requires IMDb ID)");
            continue;
        }
        
        let query_clone = normalized_query.clone();
        let handle = tokio::spawn(async move {
            let result: Result<Vec<search::SearchResult>, Box<dyn std::error::Error + Send + Sync>> = match tracker.as_str() {
                "nyaa" => {
                    println!("Searching Nyaa...");
                    NyaaProvider::new().search(&query_clone).await
                }
                "1337x" => {
                    println!("Searching 1337x...");
                    X1337Provider::new().search(&query_clone).await
                }
                "thepiratebay" => {
                    println!("Searching ThePirateBay...");
                    PirateBayProvider::new().search(&query_clone).await
                }
                _ => {
                    println!("Unknown tracker: {}", tracker);
                    Ok(vec![])
                }
            };
            
            match result {
                Ok(results) => {
                    println!("{} returned {} results", tracker, results.len());
                    results
                }
                Err(e) => {
                    println!("{} error: {}", tracker, e);
                    vec![]
                }
            }
        });
        handles.push(handle);
    }
    
    // Wait for all searches to complete
    let mut all_results = Vec::new();
    for handle in handles {
        if let Ok(results) = handle.await {
            all_results.extend(results);
        }
    }
    
    println!("Total results before deduplication: {}", all_results.len());
    
    // Deduplicate by info hash (from magnet link)
    let mut seen_hashes = std::collections::HashSet::new();
    all_results.retain(|result| {
        if let Some(hash) = extract_info_hash(&result.magnet_link) {
            seen_hashes.insert(hash)
        } else {
            true // Keep if can't extract hash
        }
    });
    
    println!("Total results after deduplication: {}", all_results.len());
    
    // Filter results based on media type and requested episode/season
    all_results.retain(|result| {
        if is_movie {
            true
        } else {
            match (season, episode) {
                (Some(req_season), Some(req_episode)) => {
                    if let Some(s) = result.season {
                        if s != req_season {
                            return false;
                        }
                        if result.is_batch {
                            return true;
                        }
                        if let Some(e) = result.episode {
                            return e == req_episode;
                        }
                        return true;
                    } else {
                        true
                    }
                }
                (Some(req_season), None) => {
                    if let Some(s) = result.season {
                        s == req_season
                    } else {
                        true
                    }
                }
                _ => true,
            }
        }
    });

    // Sort by relevance score
    all_results.sort_by(|a, b| {
        let score_a = calculate_relevance_score(a, season, episode, &normalized_query);
        let score_b = calculate_relevance_score(b, season, episode, &normalized_query);
        match score_b.cmp(&score_a) {
            std::cmp::Ordering::Equal => b.seeds.cmp(&a.seeds),
            other => other,
        }
    });

    Ok(all_results)
}

// Extract info hash from magnet link for deduplication
fn extract_info_hash(magnet: &str) -> Option<String> {
    magnet
        .split('&')
        .find(|part| part.starts_with("xt=urn:btih:"))
        .and_then(|part| part.strip_prefix("xt=urn:btih:"))
        .map(|hash| hash.to_lowercase())
}

fn calculate_relevance_score(
    result: &search::SearchResult,
    requested_season: Option<u32>,
    requested_episode: Option<u32>,
    query: &str,
) -> i32 {
    let mut score = 0;

    // Exact season/episode match is highly relevant
    if let (Some(req_s), Some(req_e)) = (requested_season, requested_episode) {
        if let Some(s) = result.season {
            if s == req_s {
                score += 100; // Correct season
                
                if let Some(e) = result.episode {
                    if e == req_e {
                        score += 100; // Exact episode match - highest priority
                    } else {
                        score -= 50; // Wrong episode in correct season
                    }
                } else if result.is_batch {
                    score += 50; // Batch for correct season - good fallback
                }
            } else {
                score -= 100; // Wrong season
            }
        }
    }

    // Quality detection adds relevance
    if result.quality.is_some() {
        score += 10;
        
        // Prefer 1080p
        if let Some(ref quality) = result.quality {
            if quality.contains("1080") {
                score += 15;
            } else if quality.contains("720") {
                score += 10;
            } else if quality.contains("2160") || quality.contains("4K") {
                score += 5; // 4K is good but might be too large
            }
        }
    }

    // Encode detection adds relevance
    if result.encode.is_some() {
        score += 5;
        
        // Prefer modern codecs
        if let Some(ref encode) = result.encode {
            if encode.contains("265") || encode.contains("HEVC") {
                score += 10; // Modern efficient codec
            } else if encode.contains("264") || encode.contains("AVC") {
                score += 5; // Standard codec
            }
        }
    }

    // Title similarity to query (simple word matching)
    let title_lower = result.title.to_lowercase();
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let matched_words = query_words.iter().filter(|word| title_lower.contains(*word)).count();
    score += (matched_words * 5) as i32;

    // Seed count contributes to relevance (but less than exact matches)
    score += (result.seeds.min(100) / 10) as i32; // Max 10 points from seeds

    // Penalize if it's marked as batch when we want a specific episode
    if requested_episode.is_some() && result.is_batch && result.episode.is_none() {
        score -= 30; // Batch without specific episode when we want one episode
    }

    score
}

#[tauri::command]
async fn search_eztv_by_imdb(imdb_id: String) -> Result<Vec<search::SearchResult>, String> {
    println!("Searching EZTV with IMDb ID: {}", imdb_id);
    let provider = search::eztv::EZTVProvider::new();
    provider.search_by_imdb(&imdb_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_torrent_selection(
    tracking: State<'_, TrackingManager>,
    show_id: u32,
    season: u32,
    episode: u32,
    magnet_link: String,
    file_index: usize,
) -> Result<(), String> {
    tracking
        .save_selection(show_id, season, episode, magnet_link, file_index)
        .await;
    Ok(())
}
#[tauri::command]
async fn get_saved_selection(
    tracking: State<'_, TrackingManager>,
    #[allow(non_snake_case)] showId: u32,
    season: u32,
    episode: u32,
) -> Result<Option<tracking::EpisodeTorrent>, String> {
    Ok(tracking.get_selection(showId, season, episode).await)
}

#[tauri::command]
async fn save_subtitle_cache(
    cache: State<'_, MediaCache>,
    cache_id: String,
    file_index: usize,
    track_index: usize,
    data: String,
) -> Result<(), String> {
    cache.save_track(TrackType::Subtitle, &cache_id, file_index, track_index, data.into_bytes()).await
}

#[tauri::command]
async fn load_subtitle_cache(
    cache: State<'_, MediaCache>,
    cache_id: String,
    file_index: usize,
    track_index: usize,
) -> Result<Option<String>, String> {
    let result = cache.load_track(TrackType::Subtitle, &cache_id, file_index, track_index).await?;
    Ok(result.and_then(|bytes| String::from_utf8(bytes).ok()))
}

#[tauri::command]
async fn clear_subtitle_cache(
    cache: State<'_, MediaCache>,
) -> Result<(), String> {
    cache.clear_cache(TrackType::Subtitle).await
}

#[tauri::command]
async fn save_audio_cache(
    cache: State<'_, MediaCache>,
    cache_id: String,
    file_index: usize,
    track_index: usize,
    data: Vec<u8>,
) -> Result<(), String> {
    cache.save_track(TrackType::Audio, &cache_id, file_index, track_index, data).await
}

#[tauri::command]
async fn load_audio_cache(
    cache: State<'_, MediaCache>,
    cache_id: String,
    file_index: usize,
    track_index: usize,
) -> Result<Option<Vec<u8>>, String> {
    cache.load_track(TrackType::Audio, &cache_id, file_index, track_index).await
}

#[tauri::command]
async fn clear_audio_cache(
    cache: State<'_, MediaCache>,
) -> Result<(), String> {
    cache.clear_cache(TrackType::Audio).await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_handle = app.handle();
            let app_data_dir = app_handle
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            
            // Create app data dir if it doesn't exist
            if !app_data_dir.exists() {
                std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
            }

            let tracking_manager = TrackingManager::new(app_data_dir.clone());
            app.manage(tracking_manager);

            let media_cache = MediaCache::new(app_data_dir.clone());
            app.manage(media_cache);

            let torrent_dir = app_data_dir.join("torrents");
            let torrent_manager = tauri::async_runtime::block_on(async {
                TorrentManager::new(torrent_dir)
                    .await
                    .expect("Failed to initialize torrent manager")
            });
            let torrent_manager_arc = Arc::new(torrent_manager);
            app.manage(torrent_manager_arc.clone());

            // Cleanup torrents on app close
            let manager_for_cleanup = torrent_manager_arc.clone();
            let main_window = app.get_webview_window("main").unwrap();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    tauri::async_runtime::block_on(async {
                        if let Err(e) = manager_for_cleanup.cleanup_all().await {
                            eprintln!("Error during cleanup: {}", e);
                        }
                    });
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            torrent::add_torrent,
            torrent::get_torrent_info,
            torrent::list_torrents,
            torrent::prepare_stream,
            torrent::get_stream_status,
            torrent::stop_stream,
            torrent::pause_torrent,
            torrent::resume_torrent,
            torrent::remove_torrent,
            torrent::get_download_dir,
            search_nyaa,
            search_nyaa_filtered,
            search_eztv_by_imdb,
            save_torrent_selection,
            get_saved_selection,
            save_subtitle_cache,
            load_subtitle_cache,
            clear_subtitle_cache,
            save_audio_cache,
            load_audio_cache,
            clear_audio_cache
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

