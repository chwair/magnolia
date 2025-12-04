fn main() {
    // Download FFmpeg binaries during build if they don't exist
    ffmpeg_sidecar::download::auto_download().expect("Failed to download FFmpeg");
    
    tauri_build::build()
}
