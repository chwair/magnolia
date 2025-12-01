use anyhow::Result;
use axum::{
    extract::Path,
    response::{IntoResponse, Response},
    http::{StatusCode, header},
    body::Body,
};
use std::sync::Arc;
use tokio::process::Command;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use librqbit::api::TorrentIdOrHash;
use crate::torrent::AppState;

pub async fn dash_manifest(
    Path((session_id, file_id)): Path<(usize, usize)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let handle = match state.session.get(TorrentIdOrHash::Id(session_id)) {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, "Torrent not found").into_response(),
    };

    // Get media metadata (audio tracks, subtitles, chapters)
    let metadata = match get_media_metadata(&handle, session_id, file_id, &state).await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to get metadata: {}", e);
            MediaMetadata::default()
        }
    };

    // Calculate duration (placeholder - should be extracted from metadata)
    let duration = metadata.duration.unwrap_or(3600.0);
    let segment_duration = 10.0;

    // Generate MPD manifest
    let manifest = generate_mpd_manifest(
        session_id,
        file_id,
        &metadata,
        duration,
        segment_duration,
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/dash+xml")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(manifest))
        .unwrap()
}

fn generate_mpd_manifest(
    _session_id: usize,
    _file_id: usize,
    metadata: &MediaMetadata,
    duration: f64,
    _segment_duration: f64,
) -> String {
    let duration_str = format_duration(duration);
    
    let mut manifest = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" 
     xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:schemaLocation="urn:mpeg:dash:schema:mpd:2011 http://standards.iso.org/ittf/PubliclyAvailableStandards/MPEG-DASH_schema_files/DASH-MPD.xsd"
     type="static"
     mediaPresentationDuration="{}"
     minBufferTime="PT2S"
     profiles="urn:mpeg:dash:profile:isoff-on-demand:2011">
  <Period>
"#,
        duration_str
    );

    // Add chapter information as EventStream
    if !metadata.chapters.is_empty() {
        manifest.push_str(r#"    <EventStream schemeIdUri="urn:mpeg:dash:event:2012" timescale="1000">
"#);
        for chapter in &metadata.chapters {
            let start_ms = (chapter.start_time * 1000.0) as u64;
            let title = chapter.title.as_deref().unwrap_or("Chapter");
            manifest.push_str(&format!(
                r#"      <Event presentationTime="{}" duration="0" id="{}">
        <ChapterInfo title="{}"/>
      </Event>
"#,
                start_ms,
                start_ms,
                title.replace('"', "&quot;")
            ));
        }
        manifest.push_str("    </EventStream>\n");
    }

    // Video AdaptationSet
    manifest.push_str(r#"    <AdaptationSet id="1" contentType="video" mimeType="video/mp4" segmentAlignment="true" startWithSAP="1">
      <Representation id="video-1" codecs="avc1.4d401f" width="1920" height="1080" frameRate="24" bandwidth="5000000">
        <SegmentTemplate timescale="1000" duration="10000" initialization="video/init.mp4" media="video/segment/$Number$" startNumber="0"/>
      </Representation>
    </AdaptationSet>
"#);

    // Audio AdaptationSets
    for (idx, track) in metadata.audio_tracks.iter().enumerate() {
        let lang = track.language.as_deref().unwrap_or("und");
        let default_name = format!("Audio Track {}", idx + 1);
        let track_name = track.name.as_deref().unwrap_or(&default_name);
        
        manifest.push_str(&format!(
            r#"    <AdaptationSet id="{}" contentType="audio" lang="{}" mimeType="audio/mp4" segmentAlignment="true" startWithSAP="1">
      <Label>{}</Label>
      <Representation id="audio-{}" codecs="mp4a.40.2" bandwidth="128000" audioSamplingRate="48000">
        <SegmentTemplate timescale="1000" duration="10000" initialization="audio/{}/init.mp4" media="audio/{}/segment/$Number$" startNumber="0"/>
      </Representation>
    </AdaptationSet>
"#,
            idx + 2, lang, track_name, idx, idx, idx
        ));
    }

    // Subtitle AdaptationSets
    for (idx, track) in metadata.subtitle_tracks.iter().enumerate() {
        let lang = track.language.as_deref().unwrap_or("und");
        let default_name = format!("Subtitle Track {}", idx + 1);
        let track_name = track.name.as_deref().unwrap_or(&default_name);
        
        // Check if it's ASS/SSA subtitle
        let is_ass = track.codec.as_deref().map(|c| c.contains("ass") || c.contains("ssa") || c == "ass").unwrap_or(false);
        
        if is_ass {
            // ASS subtitles - reference the subtitle file directly
            manifest.push_str(&format!(
                r#"    <AdaptationSet id="{}" contentType="text" lang="{}" mimeType="application/x-subrip">
      <Label>{}</Label>
      <Representation id="subtitle-{}" bandwidth="1000">
        <BaseURL>subtitles/{}/subtitle.ass</BaseURL>
      </Representation>
    </AdaptationSet>
"#,
                100 + idx, lang, track_name, idx, idx
            ));
        } else {
            // Regular subtitles (WebVTT)
            manifest.push_str(&format!(
                r#"    <AdaptationSet id="{}" contentType="text" lang="{}" mimeType="application/mp4" segmentAlignment="true">
      <Label>{}</Label>
      <Representation id="subtitle-{}" codecs="wvtt" bandwidth="1000">
        <SegmentTemplate timescale="1000" duration="10000" initialization="subtitles/{}/init.mp4" media="subtitles/{}/segment/$Number$" startNumber="0"/>
      </Representation>
    </AdaptationSet>
"#,
                100 + idx, lang, track_name, idx, idx, idx
            ));
        }
    }

    manifest.push_str("  </Period>\n</MPD>");
    
    manifest
}

fn format_duration(seconds: f64) -> String {
    let hours = (seconds / 3600.0).floor() as u32;
    let minutes = ((seconds % 3600.0) / 60.0).floor() as u32;
    let secs = (seconds % 60.0).floor() as u32;
    format!("PT{}H{}M{}S", hours, minutes, secs)
}

pub async fn dash_video_init(
    Path((session_id, file_id)): Path<(usize, usize)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    generate_init_segment(session_id, file_id, "video", None, state).await
}

pub async fn dash_audio_init(
    Path((session_id, file_id, track_id)): Path<(usize, usize, usize)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    generate_init_segment(session_id, file_id, "audio", Some(track_id), state).await
}

pub async fn dash_video_segment(
    Path((session_id, file_id, segment_num)): Path<(usize, usize, usize)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    generate_media_segment(session_id, file_id, "video", None, segment_num, state).await
}

pub async fn dash_audio_segment(
    Path((session_id, file_id, track_id, segment_num)): Path<(usize, usize, usize, usize)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    generate_media_segment(session_id, file_id, "audio", Some(track_id), segment_num, state).await
}

pub async fn dash_subtitle(
    Path((session_id, file_id, track_id)): Path<(usize, usize, usize)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let handle = match state.session.get(TorrentIdOrHash::Id(session_id)) {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, "Torrent not found").into_response(),
    };

    // Cache key for subtitle
    let cache_key = format!("subtitle_{}:{}:{}", session_id, file_id, track_id);
    
    // Check cache
    {
        let cache = state.hls_cache.lock().await;
        if let Some(subtitle_path) = cache.get(&cache_key) {
            if subtitle_path.exists() {
                match tokio::fs::read(subtitle_path).await {
                    Ok(data) => {
                        return Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "application/x-subtitle-ass")
                            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                            .body(Body::from(data))
                            .unwrap();
                    }
                    Err(_) => {}
                }
            }
        }
    }

    // Extract subtitle from video file
    let mut stream = match handle.stream(file_id) {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create stream: {}", e)).into_response();
        }
    };

    // Use ffmpeg to extract subtitle track
    let mut child = match Command::new("ffmpeg")
        .args(&[
            "-i", "pipe:0",
            "-map", &format!("0:s:{}", track_id),
            "-c:s", "copy",
            "-f", "ass",
            "pipe:1"
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to spawn ffmpeg: {}", e)).into_response();
        }
    };

    // Pipe video data to ffmpeg
    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1024 * 1024];
            loop {
                match stream.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tokio::io::AsyncWriteExt::write_all(&mut stdin, &buffer[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("FFmpeg execution failed: {}", e)).into_response();
        }
    };

    if !output.status.success() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to extract subtitle").into_response();
    }

    let subtitle_data = output.stdout;

    // Cache the subtitle
    if let Ok(temp_dir) = std::env::temp_dir().canonicalize() {
        let subtitle_path = temp_dir.join(format!("dash_sub_{}_{}_{}.ass", session_id, file_id, track_id));
        if tokio::fs::write(&subtitle_path, &subtitle_data).await.is_ok() {
            let mut cache = state.hls_cache.lock().await;
            cache.insert(cache_key, subtitle_path);
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-subtitle-ass")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(subtitle_data))
        .unwrap()
}

async fn generate_init_segment(
    session_id: usize,
    file_id: usize,
    media_type: &str,
    track_id: Option<usize>,
    state: AppState,
) -> Response {
    let handle = match state.session.get(TorrentIdOrHash::Id(session_id)) {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, "Torrent not found").into_response(),
    };

    let cache_key = format!("init_{}_{}:{}:{:?}", media_type, session_id, file_id, track_id);
    
    // Check cache
    {
        let cache = state.hls_cache.lock().await;
        if let Some(init_path) = cache.get(&cache_key) {
            if init_path.exists() {
                match tokio::fs::read(init_path).await {
                    Ok(data) => {
                        return Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "video/mp4")
                            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                            .body(Body::from(data))
                            .unwrap();
                    }
                    Err(_) => {}
                }
            }
        }
    }

    let mut stream = match handle.stream(file_id) {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create stream: {}", e)).into_response();
        }
    };

    // Build ffmpeg arguments for initialization segment
    let mut args = vec![
        "-i", "pipe:0",
    ];

    // Map appropriate stream
    let audio_map: String;
    if media_type == "video" {
        args.extend(&["-map", "0:v:0", "-c:v", "libx264", "-preset", "ultrafast"]);
    } else if media_type == "audio" {
        let track = track_id.unwrap_or(0);
        audio_map = format!("0:a:{}", track);
        args.extend(&[
            "-map", &audio_map,
            "-c:a", "aac",
            "-b:a", "128k",
        ]);
    }

    args.extend(&[
        "-movflags", "frag_keyframe+empty_moov+default_base_moof",
        "-f", "mp4",
        "-t", "0",
        "pipe:1",
    ]);

    let mut child = match Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to spawn ffmpeg: {}", e)).into_response();
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1024 * 1024];
            let mut total_read = 0;
            loop {
                if total_read > 10 * 1024 * 1024 { break; } // Read only first 10MB for init
                match stream.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => {
                        total_read += n;
                        if tokio::io::AsyncWriteExt::write_all(&mut stdin, &buffer[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("FFmpeg execution failed: {}", e)).into_response();
        }
    };

    if !output.status.success() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "FFmpeg failed").into_response();
    }

    let init_data = output.stdout;

    // Cache init segment
    if let Ok(temp_dir) = std::env::temp_dir().canonicalize() {
        let init_path = temp_dir.join(format!("dash_init_{}_{}_{:?}_{}.mp4", 
            media_type, session_id, track_id, chrono::Utc::now().timestamp()));
        if tokio::fs::write(&init_path, &init_data).await.is_ok() {
            let mut cache = state.hls_cache.lock().await;
            cache.insert(cache_key, init_path);
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(init_data))
        .unwrap()
}

async fn generate_media_segment(
    session_id: usize,
    file_id: usize,
    media_type: &str,
    track_id: Option<usize>,
    segment_num: usize,
    state: AppState,
) -> Response {
    let handle = match state.session.get(TorrentIdOrHash::Id(session_id)) {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, "Torrent not found").into_response(),
    };

    let cache_key = format!("seg_{}_{}:{}:{:?}:{}", media_type, session_id, file_id, track_id, segment_num);
    
    // Check cache
    {
        let cache = state.hls_cache.lock().await;
        if let Some(seg_path) = cache.get(&cache_key) {
            if seg_path.exists() {
                match tokio::fs::read(seg_path).await {
                    Ok(data) => {
                        return Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "video/mp4")
                            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                            .body(Body::from(data))
                            .unwrap();
                    }
                    Err(_) => {}
                }
            }
        }
    }

    let mut stream = match handle.stream(file_id) {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create stream: {}", e)).into_response();
        }
    };

    let segment_duration = 10;
    let start_time = segment_num * segment_duration;
    let start_time_str = start_time.to_string();
    let segment_duration_str = segment_duration.to_string();

    let mut args = vec![
        "-ss", &start_time_str,
        "-t", &segment_duration_str,
        "-i", "pipe:0",
    ];

    let audio_map: String;
    if media_type == "video" {
        args.extend(&[
            "-map", "0:v:0",
            "-c:v", "libx264",
            "-preset", "ultrafast",
            "-crf", "23",
        ]);
    } else if media_type == "audio" {
        let track = track_id.unwrap_or(0);
        audio_map = format!("0:a:{}", track);
        args.extend(&[
            "-map", &audio_map,
            "-c:a", "aac",
            "-b:a", "128k",
        ]);
    }

    args.extend(&[
        "-movflags", "frag_keyframe+empty_moov+default_base_moof",
        "-f", "mp4",
        "pipe:1",
    ]);

    let mut child = match Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to spawn ffmpeg: {}", e)).into_response();
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1024 * 1024];
            loop {
                match stream.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tokio::io::AsyncWriteExt::write_all(&mut stdin, &buffer[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("FFmpeg execution failed: {}", e)).into_response();
        }
    };

    if !output.status.success() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "FFmpeg failed").into_response();
    }

    let segment_data = output.stdout;

    // Cache segment
    if let Ok(temp_dir) = std::env::temp_dir().canonicalize() {
        let seg_path = temp_dir.join(format!("dash_seg_{}_{}_{:?}_{}_{}.m4s", 
            media_type, session_id, track_id, segment_num, chrono::Utc::now().timestamp()));
        if tokio::fs::write(&seg_path, &segment_data).await.is_ok() {
            let mut cache = state.hls_cache.lock().await;
            cache.insert(cache_key, seg_path);
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(segment_data))
        .unwrap()
}

#[derive(Default)]
struct MediaMetadata {
    duration: Option<f64>,
    audio_tracks: Vec<AudioTrackInfo>,
    subtitle_tracks: Vec<SubtitleTrackInfo>,
    chapters: Vec<ChapterInfo>,
}

struct AudioTrackInfo {
    _index: usize,
    language: Option<String>,
    _codec: Option<String>,
    name: Option<String>,
}

struct SubtitleTrackInfo {
    _index: usize,
    language: Option<String>,
    codec: Option<String>,
    name: Option<String>,
}

struct ChapterInfo {
    start_time: f64,
    _end_time: f64,
    title: Option<String>,
}

async fn get_media_metadata(
    _handle: &Arc<impl std::any::Any>,
    session_id: usize,
    file_id: usize,
    state: &AppState,
) -> Result<MediaMetadata> {
    use tokio::io::AsyncWriteExt;
    
    // Get torrent handle directly from state
    let torrent_handle = state.session.get(TorrentIdOrHash::Id(session_id))
        .ok_or_else(|| anyhow::anyhow!("Torrent not found"))?;
    
    let mut stream = torrent_handle.stream(file_id)?;
    
    // Create a temporary file to write enough data for ffprobe
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("probe_{}_{}.tmp", session_id, file_id));
    
    // Read first 50MB for probing
    let mut file = tokio::fs::File::create(&temp_file).await?;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut total_read = 0usize;
    let max_read = 50 * 1024 * 1024;
    
    while total_read < max_read {
        match stream.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                file.write_all(&buffer[..n]).await?;
                total_read += n;
            }
            Err(e) => {
                eprintln!("Error reading stream: {}", e);
                break;
            }
        }
    }
    file.flush().await?;
    drop(file);
    
    // Run ffprobe
    let output = Command::new("ffprobe")
        .args(&[
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            "-show_chapters",
            temp_file.to_str().unwrap(),
        ])
        .output()
        .await?;
    
    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_file).await;
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("ffprobe failed"));
    }
    
    // Parse ffprobe JSON output
    let probe_data: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    
    let mut metadata = MediaMetadata::default();
    
    // Extract duration
    if let Some(format) = probe_data.get("format") {
        if let Some(duration_str) = format.get("duration").and_then(|d| d.as_str()) {
            metadata.duration = duration_str.parse().ok();
        }
    }
    
    // Extract streams
    if let Some(streams) = probe_data.get("streams").and_then(|s| s.as_array()) {
        let mut audio_index = 0;
        let mut subtitle_index = 0;
        
        for stream in streams {
            let codec_type = stream.get("codec_type").and_then(|t| t.as_str());
            
            match codec_type {
                Some("audio") => {
                    let codec_name = stream.get("codec_name").and_then(|c| c.as_str()).unwrap_or("unknown");
                    let language = stream.get("tags")
                        .and_then(|t| t.get("language"))
                        .and_then(|l| l.as_str())
                        .unwrap_or("und")
                        .to_string();
                    let title = stream.get("tags")
                        .and_then(|t| t.get("title"))
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                    
                    metadata.audio_tracks.push(AudioTrackInfo {
                        _index: audio_index,
                        language: Some(language),
                        _codec: Some(codec_name.to_string()),
                        name: title,
                    });
                    audio_index += 1;
                }
                Some("subtitle") => {
                    let codec_name = stream.get("codec_name").and_then(|c| c.as_str()).unwrap_or("unknown");
                    let language = stream.get("tags")
                        .and_then(|t| t.get("language"))
                        .and_then(|l| l.as_str())
                        .unwrap_or("und")
                        .to_string();
                    let title = stream.get("tags")
                        .and_then(|t| t.get("title"))
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                    
                    metadata.subtitle_tracks.push(SubtitleTrackInfo {
                        _index: subtitle_index,
                        language: Some(language),
                        codec: Some(codec_name.to_string()),
                        name: title,
                    });
                    subtitle_index += 1;
                }
                _ => {}
            }
        }
    }
    
    // Extract chapters
    if let Some(chapters) = probe_data.get("chapters").and_then(|c| c.as_array()) {
        for chapter in chapters {
            let start_str = chapter.get("start_time").and_then(|s| s.as_str());
            let end_str = chapter.get("end_time").and_then(|e| e.as_str());
            let title = chapter.get("tags")
                .and_then(|t| t.get("title"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            
            if let (Some(start), Some(end)) = (start_str, end_str) {
                if let (Ok(start_time), Ok(end_time)) = (start.parse::<f64>(), end.parse::<f64>()) {
                    metadata.chapters.push(ChapterInfo {
                        start_time,
                        _end_time: end_time,
                        title,
                    });
                }
            }
        }
    }
    
    Ok(metadata)
}
