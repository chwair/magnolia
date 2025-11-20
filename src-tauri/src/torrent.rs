use anyhow::{Context, Result};
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, api::TorrentIdOrHash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::net::SocketAddr;
use tauri::State;
use tokio::sync::RwLock;
use axum::{
    Router,
    routing::get,
    extract::Path,
    response::{IntoResponse, Response},
    http::{StatusCode, header, HeaderMap},
    body::Body,
};
use tower_http::cors::CorsLayer;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[derive(Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    pub index: usize,
    pub name: String,
    pub size: u64,
    pub path: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    pub index: usize,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SubtitleTrack {
    pub index: usize,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub index: usize,
    pub title: Option<String>,
    pub start_time: f64,
    pub end_time: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MkvMetadata {
    pub audio_tracks: Vec<AudioTrack>,
    pub subtitle_tracks: Vec<SubtitleTrack>,
    pub chapters: Vec<Chapter>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TorrentInfo {
    pub handle_id: usize,
    pub name: String,
    pub size: u64,
    pub files: Vec<TorrentFile>,
    pub progress: f64,
    pub download_speed: u64,
    pub upload_speed: u64,
    pub peers: usize,
    pub is_paused: bool,
    pub state: String, // "checking", "downloading", "paused", "live"
}

#[derive(Clone, Serialize)]
pub struct StreamInfo {
    pub url: String,
    pub file_name: String,
    pub file_size: u64,
    pub metadata: Option<MkvMetadata>,
}

#[derive(Clone, Serialize)]
pub struct StreamStatus {
    pub status: String, // "initializing", "ready", "error"
    pub progress_bytes: u64,
    pub total_bytes: u64,
    pub peers: usize,
    pub download_speed: u64,
    pub stream_info: Option<StreamInfo>,
    pub state: String, // "checking", "downloading"
}

#[derive(Clone)]
struct AppState {
    session: Arc<Session>,
}

struct TorrentEntry {
    magnet_url: String,
    session_id: Option<usize>, // None if not yet added to session
}

pub struct TorrentManager {
    session: Arc<Session>,
    download_dir: PathBuf,
    torrents: Arc<RwLock<HashMap<usize, TorrentEntry>>>,
    next_id: Arc<RwLock<usize>>,
    http_addr: SocketAddr,
}

async fn stream_file(
    Path((session_id, file_id)): Path<(usize, usize)>,
    headers: HeaderMap,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    use std::io::SeekFrom;
    use tokio_util::io::ReaderStream;

    let handle = match state.session.get(TorrentIdOrHash::Id(session_id)) {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, "Torrent not found").into_response(),
    };

    let file_size = match handle.with_metadata(|meta| {
        meta.file_infos.get(file_id).map(|f| f.len)
    }) {
        Ok(Some(size)) => size,
        _ => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };

    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    
    let (start, end, status_code) = if let Some(range_str) = range {
        if let Some(range_values) = range_str.strip_prefix("bytes=") {
            let parts: Vec<&str> = range_values.split('-').collect();
            let start = parts[0].parse::<u64>().unwrap_or(0);
            let end = if parts.len() > 1 && !parts[1].is_empty() {
                parts[1].parse::<u64>().unwrap_or(file_size - 1).min(file_size - 1)
            } else {
                file_size - 1
            };
            (start, end, StatusCode::PARTIAL_CONTENT)
        } else {
            (0, file_size - 1, StatusCode::OK)
        }
    } else {
        (0, file_size - 1, StatusCode::OK)
    };

    let mut stream = match handle.stream(file_id) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create stream for file_id {}: {}", file_id, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to stream: {}", e)).into_response();
        }
    };

    if start > 0 {
        if let Err(e) = stream.seek(SeekFrom::Start(start)).await {
            tracing::error!("Failed to seek stream to {}: {}", start, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to seek: {}", e)).into_response();
        }
    }

    let content_length = end - start + 1;
    let limited_stream = stream.take(content_length);
    
    let reader_stream = ReaderStream::new(limited_stream);
    let body = Body::from_stream(reader_stream);

    let mut response = Response::builder()
        .status(status_code)
        .header(header::CONTENT_TYPE, "video/x-matroska")
        .header(header::CONTENT_LENGTH, content_length.to_string())
        .header(header::ACCEPT_RANGES, "bytes");
    
    if status_code == StatusCode::PARTIAL_CONTENT {
        let content_range = format!("bytes {}-{}/{}", start, end, file_size);
        response = response.header(header::CONTENT_RANGE, content_range);
    }

    response.body(body).unwrap().into_response()
}

impl TorrentManager {
    pub async fn new(download_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&download_dir)?;

        // Create session with default options
        let session = Session::new(download_dir.clone())
            .await
            .context("Failed to create librqbit session")?;

        let torrents = Arc::new(RwLock::new(HashMap::new()));
        let next_id = Arc::new(RwLock::new(0));

        // Note: We don't load existing torrents from session since we store URLs separately
        // and only add them to session when streaming starts
        tracing::info!("TorrentManager initialized");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let http_addr = listener.local_addr()?;

        let state = AppState {
            session: session.clone(),
        };

        let app = Router::new()
            .route("/torrents/{session_id}/stream/{file_id}", get(stream_file))
            .layer(CorsLayer::permissive())
            .with_state(state);

        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        Ok(Self {
            session,
            download_dir,
            torrents,
            next_id,
            http_addr,
        })
    }

    pub async fn add_torrent(&self, magnet_or_url: String) -> Result<usize> {
        tracing::info!("Adding torrent with list_only to fetch metadata: {}", magnet_or_url);
        
        let add_torrent = if magnet_or_url.starts_with("magnet:") {
            AddTorrent::from_url(&magnet_or_url)
        } else if magnet_or_url.starts_with("http") {
            AddTorrent::from_url(&magnet_or_url)
        } else {
            AddTorrent::from_local_filename(&magnet_or_url)?
        };
        
        let opts = AddTorrentOptions {
            list_only: true,
            ..Default::default()
        };
        
        let response = self.session.add_torrent(add_torrent, Some(opts)).await?;
        
        // Extract session_id if it was added (shouldn't happen with list_only, but handle it)
        let session_id = match response {
            AddTorrentResponse::Added(id, _) | AddTorrentResponse::AlreadyManaged(id, _) => {
                tracing::info!("Torrent was added to session with id: {}", id);
                Some(id)
            }
            AddTorrentResponse::ListOnly(_) => {
                tracing::info!("Got list-only response (metadata fetched)");
                None
            }
        };
        
        let mut id_lock = self.next_id.write().await;
        let our_id = *id_lock;
        *id_lock += 1;
        drop(id_lock);
        
        let mut torrents = self.torrents.write().await;
        torrents.insert(our_id, TorrentEntry {
            magnet_url: magnet_or_url,
            session_id,
        });
        
        tracing::info!("Stored torrent with our_id: {}", our_id);
        Ok(our_id)
    }

    pub async fn get_torrent_info(&self, handle_id: usize) -> Result<TorrentInfo> {
        let torrents = self.torrents.read().await;
        let entry = torrents
            .get(&handle_id)
            .context("Torrent handle not found")?;
        
        // If not yet added to session, fetch metadata via list_only
        if entry.session_id.is_none() {
            let magnet_url = entry.magnet_url.clone();
            drop(torrents);
            
            let add_torrent = if magnet_url.starts_with("magnet:") {
                AddTorrent::from_url(&magnet_url)
            } else if magnet_url.starts_with("http") {
                AddTorrent::from_url(&magnet_url)
            } else {
                AddTorrent::from_local_filename(&magnet_url)?
            };
            
            let opts = AddTorrentOptions {
                list_only: true,
                ..Default::default()
            };
            
            let response = self.session.add_torrent(add_torrent, Some(opts)).await?;
            
            match response {
                AddTorrentResponse::ListOnly(list_info) => {
                    let files: Vec<TorrentFile> = list_info.info
                        .iter_file_details()?
                        .enumerate()
                        .filter_map(|(index, detail)| {
                            let filename_str = detail.filename.to_string().ok()?;
                            if filename_str.to_lowercase().ends_with(".mkv") {
                                let pathbuf = detail.filename.to_pathbuf().ok()?;
                                let name = pathbuf
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                
                                Some(TorrentFile {
                                    index,
                                    name,
                                    size: detail.len,
                                    path: filename_str,
                                })
                            } else {
                                None
                            }
                        })
                        .collect();
                    
                    let name = match &list_info.info.name {
                        Some(n) => n.to_string(),
                        None => "Unknown".to_string(),
                    };
                    
                    return Ok(TorrentInfo {
                        handle_id,
                        name,
                        size: files.iter().map(|f| f.size).sum(),
                        files,
                        progress: 0.0,
                        download_speed: 0,
                        upload_speed: 0,
                        peers: 0,
                        is_paused: true,
                        state: "paused".to_string(),
                    });
                }
                _ => {
                    return Err(anyhow::anyhow!("Expected list_only response"));
                }
            }
        }
        
        let session_id = entry.session_id.unwrap();

        let handle = self
            .session
            .get(TorrentIdOrHash::Id(session_id))
            .context("Session torrent not found")?;

        // Get torrent metadata - filter to only .mkv files
        let files: Vec<TorrentFile> = handle
            .with_metadata(|meta| {
                meta.file_infos
                    .iter()
                    .enumerate()
                    .filter_map(|(index, file_info)| {
                        let filename = file_info
                            .relative_filename
                            .to_string_lossy()
                            .to_string();
                        
                        if filename.to_lowercase().ends_with(".mkv") {
                            Some(TorrentFile {
                                index,
                                name: file_info
                                    .relative_filename
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown")
                                    .to_string(),
                                size: file_info.len,
                                path: filename,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })?;

        let torrent_name = handle.name().unwrap_or_else(|| "Unknown".to_string());
        let stats = handle.stats();
        let is_paused = handle.is_paused();
        
        // Determine state: when stats.live is None, torrent is checking/hashing
        let state = if is_paused {
            "paused".to_string()
        } else if stats.live.is_none() {
            "checking".to_string()
        } else {
            "live".to_string()
        };

        Ok(TorrentInfo {
            handle_id,
            name: torrent_name,
            size: files.iter().map(|f| f.size).sum(),
            files,
            progress: if stats.total_bytes > 0 {
                stats.progress_bytes as f64 / stats.total_bytes as f64 * 100.0
            } else {
                0.0
            },
            download_speed: stats
                .live
                .as_ref()
                .map(|l| l.download_speed.mbps as u64)
                .unwrap_or(0),
            upload_speed: stats
                .live
                .as_ref()
                .map(|l| l.upload_speed.mbps as u64)
                .unwrap_or(0),
            peers: stats.live.as_ref().map(|l| l.snapshot.peer_stats.live).unwrap_or(0),
            is_paused,
            state,
        })
    }

    pub async fn list_torrents(&self) -> Result<Vec<TorrentInfo>> {
        let torrents = self.torrents.read().await;
        let mut result = Vec::new();

        for (our_id, _) in torrents.iter() {
            if let Ok(info) = self.get_torrent_info(*our_id).await {
                result.push(info);
            }
        }

        Ok(result)
    }

    pub async fn prepare_stream(&self, handle_id: usize, file_index: usize) -> Result<()> {
        let torrents = self.torrents.read().await;
        let entry = torrents
            .get(&handle_id)
            .context("Torrent handle not found")?;
        
        // Add the torrent with ONLY the specific file selected
        let add_torrent = if entry.magnet_url.starts_with("magnet:") {
            AddTorrent::from_url(&entry.magnet_url)
        } else if entry.magnet_url.starts_with("http") {
            AddTorrent::from_url(&entry.magnet_url)
        } else {
            AddTorrent::from_local_filename(&entry.magnet_url)?
        };
        
        tracing::info!("Preparing stream for file index {}", file_index);
        
        let opts = AddTorrentOptions {
            overwrite: true,
            paused: false,
            only_files: Some(vec![file_index]),
            force_tracker_interval: Some(std::time::Duration::from_secs(5)), // Request peers faster
            ..Default::default()
        };
        
        let response = self.session.add_torrent(add_torrent, Some(opts)).await?;
        let (session_id, _handle) = match response {
            AddTorrentResponse::Added(id, h) => (id, h),
            AddTorrentResponse::AlreadyManaged(id, h) => {
                tracing::info!("Torrent already managed, reusing existing download");
                if h.is_paused() {
                    self.session.unpause(&h).await?;
                }
                (id, h)
            }
            AddTorrentResponse::ListOnly(_) => {
                return Err(anyhow::anyhow!("Unexpected list_only response"));
            }
        };
        
        drop(torrents);
        let mut torrents = self.torrents.write().await;
        if let Some(entry) = torrents.get_mut(&handle_id) {
            entry.session_id = Some(session_id);
        }
        
        Ok(())
    }

    pub async fn get_stream_status(&self, handle_id: usize, file_index: usize) -> Result<StreamStatus> {
        let torrents = self.torrents.read().await;
        let entry = torrents
            .get(&handle_id)
            .context("Torrent handle not found")?;
            
        let session_id = entry.session_id.context("Torrent not yet added to session")?;
        
        let handle = self.session.get(TorrentIdOrHash::Id(session_id)).context("Session torrent not found")?;
        let stats = handle.stats();
        
        let file_info = handle.with_metadata(|meta| {
            meta.file_infos.get(file_index).map(|fi| (
                fi.relative_filename.clone(),
                fi.len
            ))
        })?.context("File index out of range")?;

        let (file_name_path, file_size) = file_info;
        let file_name = file_name_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Check if ready
        // We need to ensure:
        // 1. The stream can be created (handle.stream succeeds)
        // 2. We have enough data for headers (at least 2MB or finished)
        let is_streamable = handle.stream(file_index).is_ok();
        let has_buffer = stats.progress_bytes > 2 * 1024 * 1024 || stats.finished;
        
        let is_ready = is_streamable && has_buffer;
        
        if !is_ready {
            tracing::debug!(
                "Stream not ready: streamable={}, buffer={} ({} bytes), finished={}", 
                is_streamable, has_buffer, stats.progress_bytes, stats.finished
            );
        }
        
        let stream_info = if is_ready {
             // Extract MKV metadata if it's an MKV file and already downloaded
            let metadata = if file_name.to_lowercase().ends_with(".mkv") {
                // Check if file is already fully downloaded
                if stats.progress_bytes >= stats.total_bytes && stats.total_bytes > 0 {
                    let file_path = self.download_dir.join(&file_name_path);
                    extract_mkv_metadata(&file_path).await.ok()
                } else {
                    None
                }
            } else {
                None
            };

            Some(StreamInfo {
                url: format!(
                    "http://{}/torrents/{}/stream/{}",
                    self.http_addr,
                    session_id,
                    file_index
                ),
                file_name,
                file_size,
                metadata,
            })
        } else {
            None
        };

        let state = if stats.live.is_none() {
            "checking".to_string()
        } else {
            "downloading".to_string()
        };

        Ok(StreamStatus {
            status: if is_ready { "ready".to_string() } else { "initializing".to_string() },
            progress_bytes: stats.progress_bytes,
            total_bytes: stats.total_bytes,
            peers: stats.live.as_ref().map(|l| l.snapshot.peer_stats.live).unwrap_or(0),
            download_speed: stats.live.as_ref().map(|l| l.download_speed.mbps as u64).unwrap_or(0),
            stream_info,
            state,
        })
    }
    
    pub async fn stop_stream(&self, handle_id: usize, delete_files: bool) -> Result<()> {
        tracing::info!("Stopping stream for handle_id: {}, delete_files: {}", handle_id, delete_files);
        
        let mut torrents = self.torrents.write().await;
        if let Some(entry) = torrents.get_mut(&handle_id) {
            if let Some(session_id) = entry.session_id.take() {
                tracing::info!("Deleting torrent session_id: {}", session_id);
                self.session.delete(TorrentIdOrHash::Id(session_id), delete_files).await?;
            }
        }
        
        Ok(())
    }

    pub async fn pause_torrent(&self, handle_id: usize) -> Result<()> {
        let torrents = self.torrents.read().await;
        let entry = torrents.get(&handle_id).context("Torrent not found")?;
        if let Some(session_id) = entry.session_id {
            let handle = self
                .session
                .get(TorrentIdOrHash::Id(session_id))
                .context("Session torrent not found")?;
            self.session.pause(&handle).await?;
        }
        Ok(())
    }

    pub async fn resume_torrent(&self, handle_id: usize) -> Result<()> {
        let torrents = self.torrents.read().await;
        let entry = torrents.get(&handle_id).context("Torrent not found")?;
        if let Some(session_id) = entry.session_id {
            let handle = self
                .session
                .get(TorrentIdOrHash::Id(session_id))
                .context("Session torrent not found")?;
            self.session.unpause(&handle).await?;
        }
        Ok(())
    }

    pub async fn remove_torrent(&self, handle_id: usize, delete_files: bool) -> Result<()> {
        let mut torrents = self.torrents.write().await;
        if let Some(entry) = torrents.remove(&handle_id) {
            if let Some(session_id) = entry.session_id {
                self.session.delete(TorrentIdOrHash::Id(session_id), delete_files).await?;
            }
        }
        Ok(())
    }

    pub fn get_download_dir(&self) -> PathBuf {
        self.download_dir.clone()
    }

    pub async fn cleanup_all(&self) -> Result<()> {
        tracing::info!("Cleaning up all torrents on app close");
        let torrents = self.torrents.read().await;
        let handles: Vec<usize> = torrents.keys().copied().collect();
        drop(torrents);

        for handle_id in handles {
            if let Err(e) = self.stop_stream(handle_id, true).await {
                tracing::error!("Error stopping torrent {}: {}", handle_id, e);
            }
        }
        Ok(())
    }
}

async fn extract_mkv_metadata(file_path: &std::path::Path) -> Result<MkvMetadata> {
    use std::fs::File;
    use std::io::BufReader;
    
    tracing::info!("Opening MKV file: {:?}", file_path);
    
    let file = File::open(file_path)
        .context("Failed to open MKV file")?;
    let reader = BufReader::new(file);
    
    let mkv = matroska::Matroska::open(reader)
        .context("Failed to parse MKV file")?;
    
    tracing::info!("Successfully parsed MKV file");
    
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();
    
    for track in mkv.tracks {
        match track.tracktype {
            matroska::Tracktype::Audio => {
                audio_tracks.push(AudioTrack {
                    index: audio_tracks.len(),
                    language: track.language.as_ref().map(|l| format!("{:?}", l)),
                    codec: Some(track.codec_id.clone()),
                    name: track.name.clone(),
                });
            },
            matroska::Tracktype::Subtitle => {
                subtitle_tracks.push(SubtitleTrack {
                    index: subtitle_tracks.len(),
                    language: track.language.as_ref().map(|l| format!("{:?}", l)),
                    codec: Some(track.codec_id.clone()),
                    name: track.name.clone(),
                });
            },
            _ => {}
        }
    }
    
    let chapters = Vec::new();
    // Note: matroska crate doesn't expose chapters directly in a simple way
    // We'll leave this empty for now - the video player can still use browser controls
    
    tracing::info!("Extracted {} audio tracks, {} subtitle tracks", 
        audio_tracks.len(), subtitle_tracks.len());
    
    Ok(MkvMetadata {
        audio_tracks,
        subtitle_tracks,
        chapters,
    })
}

// Tauri commands
#[tauri::command]
pub async fn add_torrent(
    manager: State<'_, Arc<TorrentManager>>,
    magnet_or_url: String,
) -> Result<usize, String> {
    manager
        .add_torrent(magnet_or_url)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_torrent_info(
    manager: State<'_, Arc<TorrentManager>>,
    handle_id: usize,
) -> Result<TorrentInfo, String> {
    manager
        .get_torrent_info(handle_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_torrents(
    manager: State<'_, Arc<TorrentManager>>,
) -> Result<Vec<TorrentInfo>, String> {
    manager.list_torrents().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prepare_stream(
    manager: State<'_, Arc<TorrentManager>>,
    handle_id: usize,
    file_index: usize,
) -> Result<(), String> {
    manager
        .prepare_stream(handle_id, file_index)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_stream_status(
    manager: State<'_, Arc<TorrentManager>>,
    handle_id: usize,
    file_index: usize,
) -> Result<StreamStatus, String> {
    manager
        .get_stream_status(handle_id, file_index)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_torrent(
    manager: State<'_, Arc<TorrentManager>>,
    handle_id: usize,
) -> Result<(), String> {
    manager
        .pause_torrent(handle_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_torrent(
    manager: State<'_, Arc<TorrentManager>>,
    handle_id: usize,
) -> Result<(), String> {
    manager
        .resume_torrent(handle_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_torrent(
    manager: State<'_, Arc<TorrentManager>>,
    handle_id: usize,
    delete_files: bool,
) -> Result<(), String> {
    manager
        .remove_torrent(handle_id, delete_files)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_stream(
    manager: State<'_, Arc<TorrentManager>>,
    handle_id: usize,
    delete_files: bool,
) -> Result<(), String> {
    manager
        .stop_stream(handle_id, delete_files)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_download_dir(manager: State<'_, Arc<TorrentManager>>) -> Result<String, String> {
    Ok(manager
        .get_download_dir()
        .to_string_lossy()
        .to_string())
}
