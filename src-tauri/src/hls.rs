use anyhow::{Context, Result};
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

pub async fn hls_master_playlist(
    Path((session_id, file_id)): Path<(usize, usize)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let handle = match state.session.get(TorrentIdOrHash::Id(session_id)) {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, "Torrent not found").into_response(),
    };

    // Get audio track count from file metadata
    let audio_tracks = match get_audio_tracks(&handle, file_id).await {
        Ok(tracks) => tracks,
        Err(_) => vec![0], // Default to single track
    };

    let mut playlist = String::from("#EXTM3U\n#EXT-X-VERSION:3\n\n");
    
    // Video + default audio
    playlist.push_str(&format!(
        "#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080,AUDIO=\"audio\"\n\
         video.m3u8\n\n"
    ));

    // Audio tracks
    for (idx, track_id) in audio_tracks.iter().enumerate() {
        let lang = format!("Track {}", idx + 1);
        let is_default = if idx == 0 { ",DEFAULT=YES" } else { "" };
        playlist.push_str(&format!(
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"{}\",LANGUAGE=\"{}\",URI=\"audio/{}.m3u8\"{}\n",
            lang, lang.to_lowercase(), track_id, is_default
        ));
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .body(Body::from(playlist))
        .unwrap()
}

pub async fn hls_video_playlist(
    Path((_session_id, _file_id)): Path<(usize, usize)>,
    axum::extract::State(_state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    // Simple playlist with segments (each 10 seconds)
    // In production, you'd calculate actual segment count based on file duration
    let mut playlist = String::from(
        "#EXTM3U\n\
         #EXT-X-VERSION:3\n\
         #EXT-X-TARGETDURATION:10\n\
         #EXT-X-MEDIA-SEQUENCE:0\n"
    );

    // Add segments (for now, just one large segment)
    playlist.push_str("#EXTINF:10.0,\nsegment/0\n");
    playlist.push_str("#EXT-X-ENDLIST\n");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .body(Body::from(playlist))
        .unwrap()
}

pub async fn hls_audio_playlist(
    Path((_session_id, _file_id, _track_id)): Path<(usize, usize, usize)>,
    axum::extract::State(_state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let playlist = format!(
        "#EXTM3U\n\
         #EXT-X-VERSION:3\n\
         #EXT-X-TARGETDURATION:10\n\
         #EXT-X-MEDIA-SEQUENCE:0\n\
         #EXTINF:10.0,\n\
         ../../segment/0\n\
         #EXT-X-ENDLIST\n"
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .body(Body::from(playlist))
        .unwrap()
}

pub async fn hls_segment(
    Path((session_id, file_id, segment_id)): Path<(usize, usize, usize)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let handle = match state.session.get(TorrentIdOrHash::Id(session_id)) {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, "Torrent not found").into_response(),
    };

    // Generate cache key
    let cache_key = format!("{}:{}:{}", session_id, file_id, segment_id);
    
    // Check cache
    {
        let cache = state.hls_cache.lock().await;
        if let Some(segment_path) = cache.get(&cache_key) {
            if segment_path.exists() {
                match tokio::fs::read(segment_path).await {
                    Ok(data) => {
                        return Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "video/mp2t")
                            .header(header::CACHE_CONTROL, "public, max-age=3600")
                            .body(Body::from(data))
                            .unwrap();
                    }
                    Err(_) => {}
                }
            }
        }
    }

    // Transcode segment on-the-fly
    // Calculate segment time range (10 seconds per segment)
    let segment_duration = 10;
    let start_time = segment_id * segment_duration;

    // Create a stream handle
    let mut stream = match handle.stream(file_id) {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create stream: {}", e)).into_response();
        }
    };
    
    println!("Transcoding segment {} starting at {}s", segment_id, start_time);

    // Spawn ffmpeg process
    let mut child = match Command::new("ffmpeg")
        .args(&[
            "-ss", &start_time.to_string(),
            "-t", &segment_duration.to_string(),
            "-i", "pipe:0",              // Read from stdin
            "-c:v", "libx264",           // Encode video to H.264
            "-preset", "ultrafast",      // Fast encoding
            "-crf", "23",                // Quality
            "-c:a", "aac",               // Encode audio to AAC
            "-b:a", "128k",              // Audio bitrate
            "-map", "0:v:0",             // Map video
            "-map", "0:a",               // Map all audio tracks
            "-f", "mpegts",              // MPEG-TS format for HLS
            "-movflags", "+faststart",
            "-avoid_negative_ts", "make_zero",
            "pipe:1"                     // Output to stdout
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to spawn ffmpeg: {}", e)).into_response();
        }
    };

    // Pipe torrent data to ffmpeg stdin
    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer
            loop {
                match stream.read(&mut buffer).await {
                    Ok(0) => break, // EOF
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

    // Read transcoded output
    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("FFmpeg execution failed: {}", e)).into_response();
        }
    };
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("FFmpeg stderr: {}", stderr);
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("FFmpeg failed: {}", stderr)).into_response();
    }

    let segment_data = output.stdout;
    println!("Successfully transcoded segment {}", segment_id);

    // Cache the segment
    if let Ok(temp_dir) = std::env::temp_dir().canonicalize() {
        let segment_path = temp_dir.join(format!("hls_seg_{}_{}_{}_{}.ts", 
            session_id, file_id, segment_id, chrono::Utc::now().timestamp()));
        
        if tokio::fs::write(&segment_path, &segment_data).await.is_ok() {
            let mut cache = state.hls_cache.lock().await;
            cache.insert(cache_key, segment_path);
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp2t")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(Body::from(segment_data))
        .unwrap()
}

async fn get_audio_tracks(_handle: &Arc<impl std::any::Any>, _file_id: usize) -> Result<Vec<usize>> {
    // TODO: Use ffprobe to detect actual audio tracks
    // For now, return multiple tracks to demonstrate functionality
    Ok(vec![0, 1])
}
