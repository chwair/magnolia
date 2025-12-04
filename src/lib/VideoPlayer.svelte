<script>
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { MKVDemuxer } from "./mkvDemuxer.js";
  import { SubtitleRenderer } from "./subtitleRenderer.js";
  import { SRTSubtitleRenderer } from "./srtSubtitleRenderer.js";
  import { AudioPlayer } from "./audioPlayer.js";
  import { formatTime } from "./utils/timeUtils.js";
  import { fetchSubtitles, downloadSubtitle } from "./wyzieSubs.js";
  import { watchProgressStore } from "./stores/watchProgressStore.js";
  import { watchHistoryStore } from "./stores/watchHistoryStore.js";

  import { createEventDispatcher } from "svelte";

  export let src = "";
  export let metadata = null;
  export let title = "";
  export let handleId = null;
  export let fileIndex = null;
  export let magnetLink = null;
  export let initialTimestamp = 0;
  
  // Track when we should reset state (only on actual new sources, not internal src updates)
  let lastInitializedSrc = null;
  
  export let mediaId = null;
  export let mediaType = null;
  export let seasonNum = null;
  export let episodeNum = null;

  let loading = true;
  let loadingPhase = "initializing"; // "initializing" | "buffering" | "metadata" | "transcoding" | "demuxing" | "ready"
  let loadingStatus = {
    progress: 0,
    total: 0,
    speed: 0,
    peers: 0,
    status: "Initializing stream...",
    state: "checking",
    phaseProgress: 0, // 0-100 for current phase
  };
  let pollInterval;
  let needsAudioTranscoding = false; // Track if audio transcoding is required
  let metadataFetched = false; // Track if we've already fetched metadata during polling

  const dispatch = createEventDispatcher();

  const SEEK_TIME_SHORT = 5;
  const SEEK_TIME_LONG = 10;
  const CONTROLS_HIDE_TIMEOUT = 2000;
  const REFRESH_INTERVAL = 2000;

  let videoElement;
  let subtitleCanvas;
  let playing = false;
  let currentTime = 0;
  let duration = 0;
  let buffered = 0;
  let torrentBufferRanges = [];
  let volume = 1;
  let muted = false;
  let fullscreen = false;
  let wasMaximizedBeforeFullscreen = false;
  let showControls = true;
  let controlsTimeout;
  let isDragging = false;
  let progressBar;
  let videoContainer;

  let demuxer = null;
  let subtitleRenderer = null;
  let srtRenderer = null;
  let audioPlayer = null;
  let useMkvDemuxer = false;
  let demuxerInitialized = false;

  // Track/subtitle selection
  let showAudioMenu = false;
  let showSubtitleMenu = false;
  let showChaptersMenu = false;
  let selectedAudioTrack = 0;
  let audioTrackSwitchingSupported = false;
  let selectedSubtitleTrack = -1;
  let chapters = [];
  let externalSubtitles = [];
  let loadingExternalSubs = false;
  let loadingSubtitle = false;
  let loadingAudio = false;
  let lastSubtitleFetchKey = null;
  
  // Caches: in-memory cache for current session
  let subtitleCache = {};
  let audioCache = {};
  
  // Extract info hash from magnet link for stable caching
  function getStableCacheId() {
    if (magnetLink) {
      // Extract info hash from magnet link (xt=urn:btih:HASH)
      const match = magnetLink.match(/xt=urn:btih:([a-fA-F0-9]+)/);
      if (match && match[1]) {
        return match[1].toLowerCase();
      }
    }
    // Fallback to handleId if magnet link not available
    return handleId ? String(handleId) : '0';
  }
  
  // Load subtitle from Tauri filesystem cache
  async function loadCachedSubtitle(cacheId, fileIndex, trackIndex) {
    try {
      const result = await invoke('load_subtitle_cache', { 
        cacheId: cacheId, 
        fileIndex: Number(fileIndex), 
        trackIndex: Number(trackIndex) 
      });
      if (result) {
        console.log(`[Subtitle Cache] Loaded ${result.length} bytes from filesystem`);
        return result;
      }
      return null;
    } catch (error) {
      console.error('[Subtitle Cache] Failed to load from filesystem:', error);
      return null;
    }
  }
  
  // Save subtitle to Tauri filesystem cache
  async function saveCachedSubtitle(cacheId, fileIndex, trackIndex, data) {
    try {
      await invoke('save_subtitle_cache', { 
        cacheId: cacheId, 
        fileIndex: Number(fileIndex), 
        trackIndex: Number(trackIndex),
        data: data
      });
    } catch (error) {
      console.error('[Subtitle Cache] Failed to save to filesystem:', error);
    }
  }

  // Load audio from Tauri filesystem cache
  async function loadCachedAudio(cacheId, fileIndex, trackIndex) {
    try {
      const base64Data = await invoke('load_audio_cache', { 
        cacheId: cacheId, 
        fileIndex: Number(fileIndex), 
        trackIndex: Number(trackIndex) 
      });
      if (base64Data) {
        const audioBuffer = base64ToUint8Array(base64Data);
        console.log(`[Audio Cache] Loaded ${audioBuffer.length} bytes from filesystem`);
        return audioBuffer;
      }
      return null;
    } catch (error) {
      console.error('[Audio Cache] Failed to load from filesystem:', error);
      return null;
    }
  }
  
  // Helper to convert Uint8Array to base64
  function uint8ArrayToBase64(uint8Array) {
    let binary = '';
    const chunkSize = 8192;
    for (let i = 0; i < uint8Array.length; i += chunkSize) {
      const chunk = uint8Array.subarray(i, Math.min(i + chunkSize, uint8Array.length));
      binary += String.fromCharCode.apply(null, chunk);
    }
    return btoa(binary);
  }
  
  // Helper to convert base64 to Uint8Array
  function base64ToUint8Array(base64) {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
  }
  
  // Save audio to Tauri filesystem cache
  async function saveCachedAudio(cacheId, fileIndex, trackIndex, audioBuffer) {
    try {
      // Convert to base64 for efficient transfer (Array.from is too slow for large files)
      const base64Data = uint8ArrayToBase64(audioBuffer);
      await invoke('save_audio_cache', { 
        cacheId: cacheId, 
        fileIndex: Number(fileIndex), 
        trackIndex: Number(trackIndex),
        data: base64Data
      });
      console.log(`[Audio Cache] Saved ${audioBuffer.length} bytes to filesystem`);
    } catch (error) {
      console.error('[Audio Cache] Failed to save to filesystem:', error);
    }
  }

  // Smooth seeking
  let isSeeking = false;
  let seekPreviewTime = 0;
  let seekTimeout;
  let hoverTime = null;
  let hoverX = 0;

  // Visual indicators for shortcuts
  let showIndicator = false;
  let indicatorType = ""; // 'seek-forward', 'seek-backward', 'volume-up', 'volume-down'
  let indicatorValue = "";
  let indicatorTimeout;
  let progressTrackingInterval = null;
  let hasAddedToHistory = false;
  let hasSeekedToInitial = false;
  let skipSectionCheckInterval = null;

  // Skip section functionality
  const skipFilters = ['intro', 'op', 'opening', 'recap', 're-cap', 'eyecatch'];
  let currentSkipSection = null;
  let showSkipButton = false;
  let skipButtonTimeout = null;
  let skipTimeRemaining = 8;
  let skipTimerInterval = null;
  let skipTimerActive = false; // True during 8-second countdown
  let skipAnimationKey = 0; // Force animation restart
  let showNextEpisodeButton = false;

  $: if (videoElement && !useMkvDemuxer) {
    videoElement.volume = volume;
  }
  
  // Sync audio player volume when it exists (for extracted/transcoded audio tracks)
  $: if (audioPlayer && audioPlayer instanceof Audio && (selectedAudioTrack > 0 || needsAudioTranscoding)) {
    audioPlayer.volume = volume;
    audioPlayer.muted = muted;
  }
  
  // Mute video element when using extracted audio (track > 0) or transcoded audio
  $: if (videoElement && (selectedAudioTrack > 0 || needsAudioTranscoding) && audioPlayer) {
    videoElement.muted = true;
  } else if (videoElement && selectedAudioTrack === 0 && !needsAudioTranscoding) {
    // Unmute video element when using native audio (track 0) and no transcoding
    videoElement.muted = muted;
  }

  $: if (metadata?.chapters) {
    chapters = metadata.chapters;
  }

  $: seekChapter = chapters
    .filter((ch) => ch.start_time <= seekPreviewTime)
    .sort((a, b) => b.start_time - a.start_time)[0];

  // Initialize demuxer when src changes
  $: if (src && src !== lastInitializedSrc) {
    lastInitializedSrc = src;
    demuxerInitialized = false;
    hasSeekedToInitial = false;
    externalSubtitles = [];
    lastSubtitleFetchKey = null;
    selectedSubtitleTrack = -1;
    needsAudioTranscoding = false;
    metadataFetched = false;
    initializeDemuxer();
  }

  // Fetch external subtitles when media info is available
  $: {
    if (mediaId && mediaType) {
      const fetchKey = `${mediaId}-${mediaType}-${seasonNum}-${episodeNum}`;
      if (fetchKey !== lastSubtitleFetchKey) {
        loadExternalSubtitles(fetchKey);
      }
    }
  }

  async function loadExternalSubtitles(fetchKey) {
    if (loadingExternalSubs) return;
    loadingExternalSubs = true;
    lastSubtitleFetchKey = fetchKey;
    
    try {
      const subs = await fetchSubtitles(mediaId, mediaType, seasonNum, episodeNum);
      externalSubtitles = subs;
      console.log("Loaded external subtitles:", externalSubtitles.length);
    } catch (error) {
      console.error("Failed to load external subtitles:", error);
      externalSubtitles = [];
    } finally {
      loadingExternalSubs = false;
    }
  }

  async function initializeDemuxer() {
    if (demuxerInitialized) return; // Already initialized or intentionally skipped
    demuxerInitialized = true;

    console.log("initializeDemuxer called with src:", src);
    console.log("Metadata prop:", metadata);

    // Check if native audioTracks API is available (Safari/macOS)
    const hasNativeAudioTracks = videoElement && 'audioTracks' in videoElement;
    console.log("Native audioTracks API available:", hasNativeAudioTracks);

    // Check if video source is from torrent streaming - fetch metadata from backend
    if (src && src.includes('/torrents/') && src.includes('/stream/')) {
      console.log("Torrent stream detected, fetching metadata from backend");
      loadingPhase = "metadata";
      loadingStatus.status = "Extracting video metadata...";
      loadingStatus.phaseProgress = 20;
      
      // Extract session_id and file_id from URL
      // URL format: http://localhost:PORT/torrents/{session_id}/stream/{file_id}
      const urlMatch = src.match(/\/torrents\/(\d+)\/stream\/(\d+)/);
      if (urlMatch) {
        const sessionId = urlMatch[1];
        const fileId = urlMatch[2];
        const baseUrl = src.substring(0, src.indexOf('/torrents/'));
        const metadataUrl = `${baseUrl}/torrents/${sessionId}/metadata/${fileId}`;
        
        console.log("Fetching metadata from:", metadataUrl);
        
        try {
          loadingStatus.status = "Reading video container...";
          loadingStatus.phaseProgress = 40;
          
          const response = await fetch(metadataUrl);
          if (response.ok) {
            const fetchedMetadata = await response.json();
            console.log("Fetched metadata:", fetchedMetadata);
            
            loadingStatus.status = "Processing track information...";
            loadingStatus.phaseProgress = 60;
            
            // Preserve transcoded_audio_url from previous metadata if it exists
            const existingTranscodedUrl = metadata?.transcoded_audio_url;
            
            // Set metadata (merge with existing to preserve transcoded_audio_url)
            metadata = {
              ...fetchedMetadata,
              // Keep transcoded_audio_url if we already have it
              transcoded_audio_url: existingTranscodedUrl || fetchedMetadata.transcoded_audio_url
            };
            chapters = fetchedMetadata.chapters || [];
            
            console.log(`Found ${fetchedMetadata.audio_tracks.length} audio tracks`);
            console.log(`Found ${fetchedMetadata.subtitle_tracks.length} subtitle tracks`);
            console.log(`Found ${chapters.length} chapters`);
            console.log(`Needs audio transcoding: ${fetchedMetadata.needs_audio_transcoding}`);
            console.log(`Transcoded audio URL: ${metadata.transcoded_audio_url}`);
            
            // Initialize demuxer for subtitle extraction if there are subtitle tracks
            if (fetchedMetadata.subtitle_tracks.length > 0) {
              loadingPhase = "demuxing";
              loadingStatus.status = "Initializing subtitle demuxer...";
              loadingStatus.phaseProgress = 70;
              
              console.log("Initializing MKVDemuxer for subtitle extraction");
              try {
                demuxer = new MKVDemuxer();
                loadingStatus.status = "Loading demuxer...";
                loadingStatus.phaseProgress = 80;
                await demuxer.initialize(src);
                loadingStatus.phaseProgress = 95;
                console.log("MKVDemuxer initialized for subtitles");
              } catch (error) {
                console.error("Failed to initialize MKVDemuxer:", error);
                demuxer = null;
                // Don't fail loading, just continue without subtitle demuxer
              }
            }
            
            loadingPhase = "ready";
            loadingStatus.status = "Ready to play";
            loadingStatus.phaseProgress = 100;
            loading = false;
            // Now trigger autoplay after all loading is complete
            startAutoplay();
          } else {
            console.error("Failed to fetch metadata:", response.status, response.statusText);
            loadingPhase = "error";
            loadingStatus.status = "Error: Failed to load video metadata";
            loading = false;
          }
        } catch (error) {
          console.error("Error fetching metadata:", error);
          loadingPhase = "error";
          loadingStatus.status = "Error: " + (error.message || "Failed to load metadata");
          loading = false;
        }
      } else {
        console.error("Could not parse session_id and file_id from URL:", src);
        loadingPhase = "error";
        loadingStatus.status = "Error: Invalid stream URL";
        loading = false;
      }
      
      // Use native video element for playback
      useMkvDemuxer = false;
      if (videoElement) {
        videoElement.muted = false;
      }
      
      return;
    }

    // For non-torrent sources, use native video element
    console.log("Using native video element playback");
    useMkvDemuxer = false;
    loadingPhase = "ready";
    loadingStatus.status = "Ready to play";
    loadingStatus.phaseProgress = 100;
    loading = false;
    
    // Unmute video element to use native audio
    if (videoElement) {
      videoElement.muted = false;
    }
    
    // Trigger autoplay for non-torrent sources
    startAutoplay();
    
    // If native audioTracks API exists, build metadata from it
    if (hasNativeAudioTracks && videoElement.audioTracks && videoElement.audioTracks.length > 0) {
      console.log("Found", videoElement.audioTracks.length, "native audio tracks");
      metadata = {
        audio_tracks: Array.from(videoElement.audioTracks).map((track, index) => ({
          id: track.id,
          name: track.label || `Audio Track ${index + 1}`,
          language: track.language || 'und',
          enabled: track.enabled
        })),
        subtitle_tracks: videoElement.textTracks ? Array.from(videoElement.textTracks).map((track, index) => ({
          id: track.id,
          name: track.label || `Subtitle Track ${index + 1}`,
          language: track.language || 'und'
        })) : [],
        chapters: []
      };
      console.log("Native track metadata:", metadata);
    }
  }

  async function startAutoplay() {
    // Wait a tick for loading overlay to hide
    await new Promise(resolve => setTimeout(resolve, 50));
    
    if (videoElement) {
      try {
        // Check if we need to use transcoded audio
        const hasTranscodedAudio = metadata?.transcoded_audio_url || metadata?.needs_audio_transcoding;
        
        console.log("=== startAutoplay ===");
        console.log("metadata:", metadata);
        console.log("transcoded_audio_url:", metadata?.transcoded_audio_url);
        console.log("needs_audio_transcoding:", metadata?.needs_audio_transcoding);
        
        // Start muted to ensure autoplay works
        const wasMuted = videoElement.muted;
        videoElement.muted = true;
        await videoElement.play();
        console.log("Video started playing (muted)");
        
        // If we have transcoded audio, load it from disk via Tauri
        if (hasTranscodedAudio && handleId !== null) {
          console.log(">>> Setting up transcoded audio playback <<<");
          
          try {
            // Check if we have cached transcoded audio first
            const stableCacheId = getStableCacheId();
            const TRANSCODED_TRACK_INDEX = 9999; // Special index for transcoded audio
            
            let audioBuffer = null;
            let blobUrl = null;
            
            // Try to load from cache first
            const cachedAudio = await loadCachedAudio(stableCacheId, fileIndex, TRANSCODED_TRACK_INDEX);
            
            if (cachedAudio) {
              console.log(`[Transcoded Audio Cache] HIT - Loaded ${cachedAudio.length} bytes from disk`);
              audioBuffer = cachedAudio;
            } else {
              // Load transcoded audio bytes directly from Tauri
              console.log("[Transcoded Audio Cache] MISS - Loading from transcoder...");
              console.log("  handleId:", handleId, "fileIndex:", fileIndex);
              
              const audioData = await invoke("load_transcoded_audio", {
                sessionId: handleId,
                fileIndex: fileIndex
              });
              
              if (audioData) {
                console.log(`Loaded ${audioData.length} bytes of transcoded audio from transcoder`);
                audioBuffer = new Uint8Array(audioData);
                
                // Cache to disk for future use
                console.log(`[Transcoded Audio Cache] STORE - Saving ${audioBuffer.length} bytes to disk`);
                await saveCachedAudio(stableCacheId, fileIndex, TRANSCODED_TRACK_INDEX, audioBuffer);
              }
            }
            
            if (audioBuffer) {
              // Create blob URL
              const blob = new Blob([audioBuffer], { type: 'audio/aac' });
              blobUrl = URL.createObjectURL(blob);
              console.log("Created blob URL:", blobUrl);
              
              // Stop existing audio if any
              if (audioPlayer && audioPlayer instanceof Audio) {
                audioPlayer.pause();
                audioPlayer.src = '';
              }
              
              // Create new audio element
              console.log("Creating new Audio element for transcoded audio");
              audioPlayer = new Audio();
              audioPlayer.preload = 'auto';
              
              // Set source and wait for it to be ready
              audioPlayer.src = blobUrl;
              
              // Wait for audio to be ready before setting currentTime
              await new Promise((resolve, reject) => {
                const onCanPlay = () => {
                  audioPlayer.removeEventListener('canplay', onCanPlay);
                  audioPlayer.removeEventListener('error', onError);
                  console.log("Audio element ready (canplay event)");
                  resolve();
                };
                const onError = (e) => {
                  audioPlayer.removeEventListener('canplay', onCanPlay);
                  audioPlayer.removeEventListener('error', onError);
                  console.error("Audio element error:", e);
                  reject(new Error('Audio failed to load'));
                };
                audioPlayer.addEventListener('canplay', onCanPlay);
                audioPlayer.addEventListener('error', onError);
                
                // Timeout after 10 seconds
                setTimeout(() => {
                  audioPlayer.removeEventListener('canplay', onCanPlay);
                  audioPlayer.removeEventListener('error', onError);
                  console.log("Audio canplay timeout - proceeding anyway");
                  resolve();
                }, 10000);
              });
              
              // Now set volume and sync position
              audioPlayer.volume = volume;
              audioPlayer.muted = muted;
              
              // Keep video muted since we're using separate audio
              videoElement.muted = true;
              
              // Sync to current video position
              const syncTime = videoElement.currentTime;
              console.log("Syncing audio to video time:", syncTime);
              audioPlayer.currentTime = syncTime;
              
              // Start playing transcoded audio
              console.log("Calling audioPlayer.play()...");
              try {
                await audioPlayer.play();
                console.log("✓ Transcoded audio playback started successfully");
                console.log("  audioPlayer.paused:", audioPlayer.paused);
                console.log("  audioPlayer.currentTime:", audioPlayer.currentTime);
                console.log("  audioPlayer.duration:", audioPlayer.duration);
              } catch (playError) {
                console.error("✗ audioPlayer.play() failed:", playError);
                throw playError;
              }
              
              // Mark that we're using transcoded audio (so syncing works)
              selectedAudioTrack = 0; // Still track 0, but using transcoded version
              needsAudioTranscoding = true; // Keep this flag for syncing logic
            } else {
              console.warn("No transcoded audio data available");
              // Fall back to native audio
              setTimeout(() => {
                if (videoElement && !wasMuted) {
                  videoElement.muted = false;
                  console.log("Video unmuted (fallback - no data)");
                }
              }, 100);
            }
          } catch (audioErr) {
            console.error("✗ Failed to setup transcoded audio:", audioErr);
            // Fall back to native audio
            setTimeout(() => {
              if (videoElement && !wasMuted) {
                videoElement.muted = false;
                console.log("Video unmuted (error fallback)");
              }
            }, 100);
          }
        } else {
          console.log("No transcoded audio - using native video audio");
          // No transcoded audio - unmute video after play starts successfully
          setTimeout(() => {
            if (videoElement && !wasMuted) {
              videoElement.muted = false;
              console.log("Video unmuted");
            }
          }, 100);
        }
        
        playing = true;
        
        // Add to watch history on autoplay
        if (!hasAddedToHistory && mediaId && metadata && (metadata.title || metadata.name)) {
          const episodeData = seasonNum !== null && episodeNum !== null ? {
            season: seasonNum,
            episode: episodeNum,
            timestamp: Math.floor(currentTime),
          } : null;
          
          const itemToSave = {
            ...metadata,
            id: mediaId,
            media_type: mediaType
          };
          
          console.log("Adding to watch history (autoplay):", itemToSave.title || itemToSave.name);
          watchHistoryStore.addItem(itemToSave, episodeData);
          hasAddedToHistory = true;
        }
      } catch (err) {
        console.error("Autoplay failed:", err);
      }
    }
  }

  async function close() {
    console.log("close/back button called");
    if (handleId !== null) {
      try {
        await invoke("stop_stream", { handleId, deleteFiles: true });
      } catch (err) {
        console.error("Failed to stop stream:", err);
      }
    }
    dispatch("back"); // Emit back instead of close to return to media detail
  }

  async function startStreamProcess() {
    loading = true;
    loadingPhase = "initializing";
    loadingStatus.status = "Preparing torrent...";
    loadingStatus.phaseProgress = 0;
    try {
      await invoke("prepare_stream", { handleId, fileIndex });
      loadingPhase = "buffering";
      loadingStatus.status = "Connecting to peers...";
      pollInterval = setInterval(checkStreamStatus, 500);
    } catch (err) {
      console.error("Failed to prepare stream:", err);
      loadingStatus.status = "Error: " + err;
      loadingPhase = "error";
    }
  }

  async function checkStreamStatus() {
    try {
      const status = await invoke("get_stream_status", { handleId, fileIndex });
      console.log("Stream status:", status);

      // Determine status message based on state and progress
      let statusMessage = "Connecting to peers...";
      if (status.status === "transcoding" && status.transcode_progress !== null) {
        loadingPhase = "transcoding";
        statusMessage = "Transcoding audio...";
      } else if (status.peers > 0) {
        if (status.progress_bytes > 0 && status.total_bytes > 0) {
          const percent = Math.round((status.progress_bytes / status.total_bytes) * 100);
          const mbDownloaded = (status.progress_bytes / 1024 / 1024).toFixed(1);
          const speedMbs = status.download_speed > 0 ? (status.download_speed / 1024 / 1024).toFixed(1) : 0;
          statusMessage = `Buffering video... ${percent}% (${speedMbs} MB/s)`;
        } else {
          statusMessage = `Connected to ${status.peers} peer${status.peers > 1 ? 's' : ''}, waiting for data...`;
        }
      }

      loadingStatus = {
        progress: status.progress_bytes,
        total: status.total_bytes,
        speed: status.download_speed,
        peers: status.peers,
        status: statusMessage,
        state: status.state,
        phaseProgress: status.status === "transcoding" 
          ? (status.transcode_progress ?? 0)
          : status.total_bytes > 0 ? Math.min((status.progress_bytes / status.total_bytes) * 100, 100) : 0,
        transcodeProgress: status.transcode_progress,
      };

      if (status.status === "ready" && status.stream_info) {
        // If we haven't checked metadata yet, fetch it now to see if transcoding is needed
        if (!metadataFetched && !needsAudioTranscoding && status.stream_info.metadata === null) {
          metadataFetched = true; // Only fetch once
          // Metadata not in stream_info yet - fetch it from the metadata endpoint
          const baseUrl = status.stream_info.url.substring(0, status.stream_info.url.indexOf('/torrents/'));
          const urlMatch = status.stream_info.url.match(/\/torrents\/(\d+)\/stream\/(\d+)/);
          if (urlMatch) {
            const sessionId = urlMatch[1];
            const fileId = urlMatch[2];
            const metadataUrl = `${baseUrl}/torrents/${sessionId}/metadata/${fileId}`;
            
            console.log("Fetching metadata to check if transcoding is needed:", metadataUrl);
            try {
              const response = await fetch(metadataUrl);
              if (response.ok) {
                const fetchedMetadata = await response.json();
                console.log("Fetched metadata:", fetchedMetadata);
                console.log("needs_audio_transcoding:", fetchedMetadata.needs_audio_transcoding);
                
                if (fetchedMetadata.needs_audio_transcoding) {
                  needsAudioTranscoding = true;
                  console.log("Audio transcoding is required, will wait for completion");
                }
              }
            } catch (err) {
              console.error("Error fetching metadata:", err);
            }
          }
        }
        
        // Check if transcoding is still needed but not complete
        // Use the metadata from stream_info, OR the tracked needsAudioTranscoding flag
        const metadataTranscoding = status.stream_info.metadata?.needs_audio_transcoding;
        const hasTranscodedUrl = status.stream_info.metadata?.transcoded_audio_url;
        
        // If metadata says transcoding needed, remember this
        if (metadataTranscoding === true) {
          needsAudioTranscoding = true;
        }
        
        // If we know transcoding is needed (from metadata or previous check) and we don't have the transcoded URL
        if (needsAudioTranscoding && !hasTranscodedUrl) {
          // Still waiting for transcoding, don't proceed yet
          console.log("Transcoding needed but not complete, continuing to poll...");
          console.log("  needsAudioTranscoding:", needsAudioTranscoding);
          console.log("  hasTranscodedUrl:", hasTranscodedUrl);
          console.log("  transcode_progress:", status.transcode_progress);
          loadingPhase = "transcoding";
          loadingStatus.status = status.transcode_progress !== null && status.transcode_progress !== undefined
            ? "Transcoding audio..."
            : "Waiting for download to complete for transcoding...";
          loadingStatus.phaseProgress = status.transcode_progress ?? 0;
          loadingStatus.transcodeProgress = status.transcode_progress;
          return; // Keep polling
        }
        
        clearInterval(pollInterval);
        loadingPhase = "metadata";
        loadingStatus.status = "Loading video metadata...";
        loadingStatus.phaseProgress = 0;
        src = status.stream_info.url;
        // Merge metadata if provided
        if (status.stream_info.metadata) {
          console.log("Setting metadata from stream_info:", status.stream_info.metadata);
          console.log("  transcoded_audio_url:", status.stream_info.metadata.transcoded_audio_url);
          metadata = status.stream_info.metadata;
        }
        // Don't autoplay here - wait for loading to complete in initializeDemuxer
      }
    } catch (err) {
      console.error("Error checking status:", err);
    }
  }

  function togglePlay() {
    console.log("togglePlay called");
    const usingTranscodedAudio = needsAudioTranscoding && audioPlayer instanceof Audio;
    
    if (playing) {
      videoElement.pause();
      // Pause extracted/transcoded audio if active (HTML5 Audio)
      if ((selectedAudioTrack > 0 || usingTranscodedAudio) && audioPlayer && audioPlayer instanceof Audio) {
        audioPlayer.pause();
      }
      playing = false;
    } else {
      if (videoElement.paused) {
        videoElement.play();
        // Play extracted/transcoded audio if active (HTML5 Audio)
        if ((selectedAudioTrack > 0 || usingTranscodedAudio) && audioPlayer && audioPlayer instanceof Audio) {
          audioPlayer.play();
        }
        playing = true;
        
        // Add to watch history on first play (only if metadata is valid)
        if (!hasAddedToHistory && mediaId && metadata && (metadata.title || metadata.name)) {
          const episodeData = seasonNum !== null && episodeNum !== null ? {
            season: seasonNum,
            episode: episodeNum,
            timestamp: Math.floor(currentTime),
          } : null;
          
          // Ensure metadata has correct id and media_type
          const itemToSave = {
            ...metadata,
            id: mediaId,
            media_type: mediaType
          };
          
          console.log("Adding to watch history:", itemToSave.title || itemToSave.name, "ID:", mediaId, "Type:", mediaType, episodeData);
          watchHistoryStore.addItem(itemToSave, episodeData);
          hasAddedToHistory = true;
        }
      } else {
        videoElement.pause();
        playing = false;
      }
    }
  }

  function startDrag(e) {
    console.log("startDrag called");
    e.preventDefault();
    isSeeking = true;
    updateSeekPreview(e);
    document.body.style.userSelect = "none";
  }

  function handleDrag(e) {
    if (isSeeking) {
      e.preventDefault();
      updateSeekPreview(e);
    }
  }

  function stopDrag(e) {
    if (isSeeking) {
      e.preventDefault();
      isSeeking = false;
      document.body.style.userSelect = "";

      if (videoElement && duration && isFinite(seekPreviewTime) && isFinite(duration)) {
        videoElement.currentTime = seekPreviewTime;
        currentTime = seekPreviewTime;

        if (useMkvDemuxer) {
          if (demuxer) demuxer.seek(seekPreviewTime, selectedAudioTrack);
          if (audioPlayer) audioPlayer.seek(seekPreviewTime);
        }
        
        // Sync HTML5 Audio when using extracted audio (track > 0) OR transcoded audio
        const usingTranscodedAudio = needsAudioTranscoding && audioPlayer instanceof Audio;
        if ((selectedAudioTrack > 0 || usingTranscodedAudio) && audioPlayer && audioPlayer instanceof Audio) {
          audioPlayer.currentTime = seekPreviewTime;
        }
        
        // Reset skip button state if we left the section
        if (currentSkipSection) {
          const stillInSection = seekPreviewTime >= currentSkipSection.start_time && seekPreviewTime < currentSkipSection.end_time;
          if (!stillInSection) {
            currentSkipSection = null;
            showSkipButton = false;
            skipTimerActive = false;
            if (skipButtonTimeout) {
              clearTimeout(skipButtonTimeout);
              skipButtonTimeout = null;
            }
            if (skipTimerInterval) {
              clearInterval(skipTimerInterval);
              skipTimerInterval = null;
            }
          }
        }
      }
    }
  }

  function updateSeekPreview(e) {
    if (!progressBar || !duration) return;
    const rect = progressBar.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = Math.max(0, Math.min(1, x / rect.width));
    seekPreviewTime = percentage * duration;

    // Smooth visual update without actually seeking
    currentTime = seekPreviewTime;
  }

  function handleProgressHover(e) {
    if (!progressBar || !duration || isSeeking) return;
    const rect = progressBar.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = Math.max(0, Math.min(1, x / rect.width));
    hoverTime = percentage * duration;
    hoverX = x;
  }

  function handleProgressLeave() {
    if (!isSeeking) {
      hoverTime = null;
    }
  }

  function seek(e) {
    if (!progressBar || !duration || !isFinite(duration)) return;
    const rect = progressBar.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = Math.max(0, Math.min(1, x / rect.width));
    const seekTime = percentage * duration;

    if (videoElement && isFinite(seekTime)) {
      videoElement.currentTime = seekTime;

      // Sync HTML5 Audio when using extracted audio (track > 0) or transcoded audio
      const usingTranscodedAudio = needsAudioTranscoding && audioPlayer instanceof Audio;
      if ((selectedAudioTrack > 0 || usingTranscodedAudio) && audioPlayer && audioPlayer instanceof Audio) {
        audioPlayer.currentTime = seekTime;
      }
      
      // Reset skip button state on manual seek if we left the section
      if (currentSkipSection) {
        const stillInSection = seekTime >= currentSkipSection.start_time && seekTime < currentSkipSection.end_time;
        if (!stillInSection) {
          currentSkipSection = null;
          showSkipButton = false;
          skipTimerActive = false;
          if (skipButtonTimeout) {
            clearTimeout(skipButtonTimeout);
            skipButtonTimeout = null;
          }
          if (skipTimerInterval) {
            clearInterval(skipTimerInterval);
            skipTimerInterval = null;
          }
        }
      }
    }
  }

  function toggleMute() {
    muted = !muted;
    // Mute/unmute will be synced via reactive statements
  }

  function changeVolume(e) {
    volume = parseFloat(e.target.value);
    if (volume > 0) {
      muted = false;
      // HTML5 Audio syncs via reactive statements, no setMuted method
    }
    // Volume will be synced via reactive statements
  }

  async function toggleFullscreen() {
    const container = videoElement.closest(".video-player");
    const appWindow = getCurrentWindow();

    if (!fullscreen) {
      try {
        // Check if window is maximized and unmaximize it before entering fullscreen
        const isMaximized = await appWindow.isMaximized();
        wasMaximizedBeforeFullscreen = isMaximized;
        if (isMaximized) {
          await appWindow.unmaximize();
          // Wait for the window state to fully update
          await new Promise((resolve) => setTimeout(resolve, 200));
        }

        // Request fullscreen
        if (container.requestFullscreen) {
          await container.requestFullscreen();
        } else if (container.webkitRequestFullscreen) {
          await container.webkitRequestFullscreen();
        }
      } catch (err) {
        console.error("Fullscreen error:", err);
      }
    } else {
      try {
        if (document.exitFullscreen) {
          await document.exitFullscreen();
        } else if (document.webkitExitFullscreen) {
          await document.webkitExitFullscreen();
        }
        
        // Restore maximize state after exiting fullscreen
        if (wasMaximizedBeforeFullscreen) {
          await new Promise((resolve) => setTimeout(resolve, 200));
          await appWindow.maximize();
          wasMaximizedBeforeFullscreen = false;
        }
      } catch (err) {
        console.error("Exit fullscreen error:", err);
      }
    }
  }

  function handleTimeUpdate() {
    if (videoElement) {
      if (isFinite(videoElement.currentTime)) {
        currentTime = videoElement.currentTime;
      } else {
        console.warn("Video currentTime is not finite:", videoElement.currentTime);
      }
      
      if (isFinite(videoElement.duration)) {
        if (duration !== videoElement.duration) {
          console.log("Duration updated:", videoElement.duration);
        }
        duration = videoElement.duration;
      } else {
        console.warn("Video duration is not finite:", videoElement.duration);
      }
    }

    if (useMkvDemuxer && subtitleRenderer) {
      subtitleRenderer.updateTime(currentTime);
    }

    if (videoElement.buffered.length > 0) {
      buffered = videoElement.buffered.end(videoElement.buffered.length - 1);
    }
    
    // Sync HTML5 Audio playback with video (for extracted/transcoded audio tracks)
    const usingTranscodedAudio = needsAudioTranscoding && audioPlayer instanceof Audio;
    if ((selectedAudioTrack > 0 || usingTranscodedAudio) && audioPlayer && audioPlayer instanceof Audio && playing) {
      const drift = Math.abs(audioPlayer.currentTime - videoElement.currentTime);
      // Resync if drift exceeds 200ms
      if (drift > 0.2) {
        console.log(`Audio drift detected: ${drift.toFixed(3)}s, resyncing...`);
        audioPlayer.currentTime = videoElement.currentTime;
      }
    }
    
    // Check for skip sections in chapters
    checkSkipSections();
    
    // Track progress periodically (every 10 seconds)
    if (playing && !loading && currentTime > 0 && mediaId && mediaType) {
      if (!progressTrackingInterval) {
        progressTrackingInterval = setInterval(() => {
          if (playing && !loading && currentTime > 0 && mediaId && mediaType) {
            const progressData = {
              currentTimestamp: Math.floor(currentTime),
            };
            
            // Add season/episode for TV shows
            if (seasonNum !== null && episodeNum !== null) {
              progressData.currentSeason = seasonNum;
              progressData.currentEpisode = episodeNum;
            }
            
            watchProgressStore.updateProgress(mediaId, mediaType, progressData);
          }
        }, 10000);
      }
    } else if (progressTrackingInterval) {
      // Clear interval when not playing
      clearInterval(progressTrackingInterval);
      progressTrackingInterval = null;
    }
  }

  function checkSkipSections() {
    if (!chapters || chapters.length === 0 || !duration) return;

    // Find current chapter (using start_time property)
    let currentChapter = null;
    for (let i = 0; i < chapters.length; i++) {
      const chapter = chapters[i];
      const nextChapter = chapters[i + 1];
      const chapterStart = chapter.start_time;
      const chapterEnd = nextChapter ? nextChapter.start_time : duration;
      
      if (currentTime >= chapterStart && currentTime < chapterEnd) {
        currentChapter = { ...chapter, end_time: chapterEnd };
        break;
      }
    }

    // Check for skippable section
    if (currentChapter && currentChapter.title) {
      const chapterTitle = currentChapter.title.toLowerCase();
      const isSkippable = skipFilters.some(filter => chapterTitle.includes(filter));

      if (isSkippable && currentSkipSection?.title !== currentChapter.title) {
        // New skippable section detected
        console.log('Skip section detected:', currentChapter.title);
        currentSkipSection = currentChapter;
        showSkipButton = true;
        skipTimerActive = true;
        skipTimeRemaining = 8;
        skipAnimationKey++; // Force animation restart
        
        // Clear existing timers
        if (skipButtonTimeout) clearTimeout(skipButtonTimeout);
        if (skipTimerInterval) clearInterval(skipTimerInterval);
        
        // Start countdown timer
        skipTimerInterval = setInterval(() => {
          skipTimeRemaining--;
          if (skipTimeRemaining <= 0) {
            clearInterval(skipTimerInterval);
            skipTimerInterval = null;
            skipTimerActive = false;
          }
        }, 1000);
        
        // Auto-hide button after 8 seconds (only if controls aren't shown)
        skipButtonTimeout = setTimeout(() => {
          if (!showControls) {
            showSkipButton = false;
          }
          skipButtonTimeout = null;
        }, 8000);
      }
    } else if (currentSkipSection) {
      // Left the skip section - always hide button
      currentSkipSection = null;
      showSkipButton = false;
      skipTimerActive = false;
      if (skipButtonTimeout) {
        clearTimeout(skipButtonTimeout);
        skipButtonTimeout = null;
      }
      if (skipTimerInterval) {
        clearInterval(skipTimerInterval);
        skipTimerInterval = null;
      }
    }

    // Check for next episode button (ending section)
    if (duration > 0 && seasonNum !== null && episodeNum !== null) {
      const endingThreshold = duration * 0.85; // Last 15% of video
      const lastChapter = chapters[chapters.length - 1];
      
      // Check if last chapter is in ending section AND duration is less than 15% of total
      if (lastChapter && lastChapter.start_time >= endingThreshold) {
        const lastChapterDuration = duration - lastChapter.start_time;
        const isShortEnding = lastChapterDuration <= (duration * 0.15);
        
        if (isShortEnding && currentTime >= lastChapter.start_time) {
          // In the short ending section
          if (!showNextEpisodeButton) {
            showNextEpisodeButton = true;
          }
        } else if (showNextEpisodeButton && currentTime < lastChapter.start_time) {
          showNextEpisodeButton = false;
        }
      } else if (showNextEpisodeButton) {
        showNextEpisodeButton = false;
      }
    }
  }

  function skipSection() {
    if (currentSkipSection && videoElement) {
      videoElement.currentTime = currentSkipSection.end_time;
      showSkipButton = false;
      skipTimerActive = false;
      currentSkipSection = null;
      if (skipButtonTimeout) clearTimeout(skipButtonTimeout);
      if (skipTimerInterval) clearInterval(skipTimerInterval);
    }
  }

  async function goToNextEpisode() {
    if (seasonNum === null || episodeNum === null) return;

    const nextEpisode = episodeNum + 1;
    
    // Check if next episode torrent is tracked
    const trackedTorrent = await invoke('get_saved_selection', {
      showId: Number(mediaId),
      season: seasonNum,
      episode: nextEpisode
    });

    if (trackedTorrent) {
      // Navigate to next episode with same torrent details
      // Need to extract handleId from magnet link or we need to start a new torrent
      // For now, just go to media details to select torrent
      window.location.href = `#/media/${mediaType}/${mediaId}?season=${seasonNum}&episode=${nextEpisode}`;
    } else {
      // Go back to media details to select torrent for next episode
      window.location.href = `#/media/${mediaType}/${mediaId}?season=${seasonNum}&episode=${nextEpisode}`;
    }
  }

  function handleLoadedMetadata() {
    console.log("=== handleLoadedMetadata called ===");
    duration = videoElement.duration;
    
    // Check if audio track switching is supported
    audioTrackSwitchingSupported = videoElement && 'audioTracks' in videoElement && 
                                   videoElement.audioTracks && videoElement.audioTracks.length > 1;
    console.log("Audio track switching supported:", audioTrackSwitchingSupported, 
                "(tracks:", videoElement.audioTracks?.length || 0, ")");
    
    console.log("Duration:", duration);
    console.log("Initial timestamp:", initialTimestamp);
    console.log("Has seeked:", hasSeekedToInitial);
    console.log("Current time before seek:", videoElement.currentTime);

    // Seek to initial timestamp if provided and not already seeked
    if (initialTimestamp > 0 && !hasSeekedToInitial && isFinite(initialTimestamp) && isFinite(duration)) {
      console.log("✓ Attempting to seek to initial timestamp:", initialTimestamp);
      videoElement.currentTime = Math.min(initialTimestamp, duration);
      hasSeekedToInitial = true;
      console.log("Immediately after setting currentTime:", videoElement.currentTime);
      // Verify seek after a short delay
      setTimeout(() => {
        console.log("Seek verification - Current time:", videoElement.currentTime, "Target was:", initialTimestamp, "Difference:", Math.abs(videoElement.currentTime - initialTimestamp));
      }, 100);
    } else if (initialTimestamp > 0) {
      console.log("✗ Skipping seek - already seeked to initial timestamp");
    } else {
      console.log("✗ No initial timestamp to seek to (initialTimestamp:", initialTimestamp, ")");
    }

    // SubtitlesOctopus automatically manages canvas size based on video dimensions
  }

  function handleFullscreenChange() {
    fullscreen = !!(
      document.fullscreenElement || document.webkitFullscreenElement
    );
  }

  // formatTime moved to src/lib/utils/timeUtils.js

  function handleMouseMove() {
    if (!showControls) {
      showControls = true;
      window.dispatchEvent(new CustomEvent("videoControlsVisibility", { detail: { visible: true } }));
    }
    clearTimeout(controlsTimeout);
    controlsTimeout = setTimeout(() => {
      if (playing && !isSeeking) {
        showControls = false;
        window.dispatchEvent(new CustomEvent("videoControlsVisibility", { detail: { visible: false } }));
      }
    }, CONTROLS_HIDE_TIMEOUT);
  }

  async function selectAudioTrack(index) {
    console.log("Selecting audio track:", index);
    const previousTrack = selectedAudioTrack;
    selectedAudioTrack = index;
    loadingAudio = true;

    try {
      // Track 0 is always the default native audio - just unmute video and stop AudioPlayer
      if (index === 0) {
        console.log("✓ Switching to default (native) audio track");
        
        // Stop and cleanup HTML5 Audio if active
        if (audioPlayer && audioPlayer instanceof Audio) {
          audioPlayer.pause();
          audioPlayer.src = '';
          audioPlayer = null;
        }
        
        // Unmute video element
        if (videoElement) {
          videoElement.muted = false;
        }
        
        showAudioMenu = false;
        loadingAudio = false;
        return;
      }

      // For other tracks, try native switching first
      if (videoElement && 'audioTracks' in videoElement && videoElement.audioTracks && videoElement.audioTracks.length > 0) {
        console.log("Attempting native audioTracks API, available tracks:", videoElement.audioTracks.length);
        try {
          for (let i = 0; i < videoElement.audioTracks.length; i++) {
            videoElement.audioTracks[i].enabled = false;
          }
          if (videoElement.audioTracks[index]) {
            videoElement.audioTracks[index].enabled = true;
            console.log(`✓ Switched to native audio track ${index}`);
            showAudioMenu = false;
            loadingAudio = false;
            return;
          }
        } catch (error) {
          console.error('Error switching native audio track:', error);
        }
      }

      // Native switching not available - extract and cache audio to disk
      console.log("⚠️ Native audioTracks API not available, using audio file caching");
      
      if (!demuxer) {
        throw new Error('Demuxer not available for audio extraction');
      }

      if (!metadata?.audio_tracks?.[index]) {
        throw new Error(`Audio track ${index} not found in metadata`);
      }

      // Check cache first
      const stableCacheId = getStableCacheId();
      let audioBlobUrl = null;

      // Determine MIME type based on codec
      const trackInfo = metadata.audio_tracks[index];
      const codec = trackInfo.codec.toLowerCase();
      let mimeType = 'audio/webm';
      
      if (codec === 'aac') {
        mimeType = 'audio/aac';
      } else if (codec === 'mp3' || codec === 'mp3float') {
        mimeType = 'audio/mpeg';
      } else if (codec === 'opus') {
        mimeType = 'audio/ogg; codecs=opus';
      } else if (codec === 'vorbis') {
        mimeType = 'audio/ogg; codecs=vorbis';
      } else if (codec === 'flac') {
        mimeType = 'audio/flac';
      }
      
      console.log(`Audio codec: ${codec}, using MIME type: ${mimeType}`);

      // Check filesystem cache
      const cachedAudio = await loadCachedAudio(stableCacheId, fileIndex, index);
      
      if (cachedAudio) {
        console.log(`[Audio Cache] Filesystem HIT - Loaded from disk`);
        // Create blob from cached data
        const blob = new Blob([cachedAudio], { type: mimeType });
        audioBlobUrl = URL.createObjectURL(blob);
      } else {
        console.log(`[Audio Cache] MISS - Extracting and saving audio`);
        // Extract raw audio packets
        const audioData = await demuxer.extractAudioTrack(index);
        
        if (!audioData) {
          throw new Error('No audio data extracted');
        }
        
        // Concatenate all packet data into single buffer
        const totalSize = audioData.packets.reduce((sum, p) => sum + p.data.byteLength, 0);
        const audioBuffer = new Uint8Array(totalSize);
        let offset = 0;
        for (const packet of audioData.packets) {
          audioBuffer.set(packet.data, offset);
          offset += packet.data.byteLength;
        }
        
        console.log(`[Audio Cache] STORE - Saving ${audioBuffer.length} bytes to disk`);
        // Save to filesystem
        await saveCachedAudio(stableCacheId, fileIndex, index, audioBuffer);
        
        // Create blob for playback
        const blob = new Blob([audioBuffer], { type: mimeType });
        audioBlobUrl = URL.createObjectURL(blob);
      }

      // Stop existing audio if any
      if (audioPlayer && audioPlayer instanceof Audio) {
        audioPlayer.pause();
        audioPlayer.src = '';
      } else {
        // Create HTML5 Audio element for playback
        audioPlayer = new Audio();
        audioPlayer.preload = 'auto';
      }
      
      // Mute video element's audio
      if (videoElement) {
        videoElement.muted = true;
      }

      // Set up audio element
      audioPlayer.src = audioBlobUrl;
      audioPlayer.volume = videoElement?.volume || 1;
      audioPlayer.muted = muted;
      
      // Sync with video position
      if (videoElement) {
        audioPlayer.currentTime = videoElement.currentTime;
        
        // Sync playback
        if (!videoElement.paused) {
          await audioPlayer.play();
        }
      }

      console.log(`✓ Switched to cached audio track ${index} using HTML5 Audio`);
    } catch (error) {
      console.error("Failed to switch audio track:", error);
      selectedAudioTrack = previousTrack;
    } finally {
      loadingAudio = false;
      showAudioMenu = false;
    }
  }

  async function selectSubtitle(track, trackIndex) {
    selectedSubtitleTrack = trackIndex;
    loadingSubtitle = true;

    try {
      // Hide any previously active subtitle renderers
      if (subtitleRenderer) {
        subtitleRenderer.hide();
      }
      if (srtRenderer) {
        srtRenderer.hide();
      }

      // Handle external subtitles from Wyzie
      if (track.source === "wyzie") {
        const subtitleText = await downloadSubtitle(track.url);
        
        if (!srtRenderer && videoElement) {
          srtRenderer = new SRTSubtitleRenderer(videoElement);
        }
        
        if (srtRenderer) {
          srtRenderer.parseSRT(subtitleText);
          srtRenderer.show();
        }
      } else if (demuxer) {
        // Use stable cache ID based on magnet link info hash
        const stableCacheId = getStableCacheId();
        const cacheKey = `${stableCacheId}-${fileIndex}-${trackIndex}`;
        let subtitleData = subtitleCache[cacheKey];
        
        console.log(`[Subtitle Cache] Using stable cache ID: ${stableCacheId}`);
        
        if (subtitleData) {
          console.log(`[Subtitle Cache] Memory HIT - Using cached subtitle for key: ${cacheKey}`);
        } else {
          // Check filesystem cache
          subtitleData = await loadCachedSubtitle(stableCacheId, fileIndex, trackIndex);
          
          if (subtitleData) {
            console.log(`[Subtitle Cache] Filesystem HIT - Loaded from disk for key: ${cacheKey}`);
            // Store in memory cache for faster access
            subtitleCache[cacheKey] = subtitleData;
          } else {
            console.log(`[Subtitle Cache] MISS - Extracting subtitle for key: ${cacheKey}`);
            // Extract subtitle from demuxer
            subtitleData = await demuxer.extractSubtitleTrack(trackIndex);
            
            if (!subtitleData) {
              throw new Error('No subtitle data extracted');
            }
            
            console.log(`[Subtitle Cache] STORE - Cached ${subtitleData.length} bytes for key: ${cacheKey}`);
            // Cache in memory and filesystem
            subtitleCache[cacheKey] = subtitleData;
            await saveCachedSubtitle(stableCacheId, fileIndex, trackIndex, subtitleData);
          }
        }
        
        // Determine codec from track info
        const subtitleTrack = demuxer.subtitleTracks[trackIndex];
        const codec = subtitleTrack?.codec?.toLowerCase() || 'ass';
        console.log(`[Subtitle] Loading subtitle with codec: ${codec}`);
        
        // Use SRT renderer for SRT/SUB files
        if (codec === 'srt' || codec === 'subrip' || codec === 'sub') {
          // Hide ASS renderer if active
          if (subtitleRenderer) {
            subtitleRenderer.hide();
          }
          
          if (!srtRenderer && videoElement) {
            srtRenderer = new SRTSubtitleRenderer(videoElement);
          }
          
          if (srtRenderer) {
            srtRenderer.parseSRT(subtitleData);
            srtRenderer.show();
          }
        } else {
          // Hide SRT renderer if active
          if (srtRenderer) {
            srtRenderer.hide();
          }
          
          // Use SubtitlesOctopus for ASS/SSA
          // Create renderer if not exists, or use existing one (it will reinitialize internally)
          if (!subtitleRenderer) {
            subtitleRenderer = new SubtitleRenderer(null, videoElement);
          }
          
          // loadSubtitleTrack will reinitialize the octopus instance for proper track switching
          await subtitleRenderer.loadSubtitleTrack(subtitleData, codec);
          subtitleRenderer.show();
          
          // Seek backward slightly to fix ASS subtitle stutter
          if (videoElement && videoElement.currentTime > 0.25) {
            const currentTime = videoElement.currentTime;
            videoElement.currentTime = currentTime - 0.25;
            console.log(`[Subtitle] Seeked back 0.25s to fix stutter (${currentTime} → ${currentTime - 0.25})`);
          }
        }
      }
    } catch (error) {
      console.error("Failed to load subtitle:", error);
    } finally {
      loadingSubtitle = false;
      showSubtitleMenu = false;
    }
  }

  function disableSubtitles() {
    selectedSubtitleTrack = -1;

    if (subtitleRenderer) {
      subtitleRenderer.hide();
    }
    
    if (srtRenderer) {
      srtRenderer.hide();
    }

    showSubtitleMenu = false;
  }

  function jumpToChapter(startTime) {
    if (videoElement && isFinite(startTime) && isFinite(duration)) {
      videoElement.currentTime = Math.min(startTime, duration);
      console.log(`Jumped to chapter at ${formatTime(startTime)}`);
      
      if (useMkvDemuxer) {
        if (demuxer) demuxer.seek(startTime, selectedAudioTrack);
        if (audioPlayer) audioPlayer.seek(startTime);
      }
    }
    showChaptersMenu = false;
  }

  function showShortcutIndicator(type, value) {
    showIndicator = true;
    indicatorType = type;
    indicatorValue = value;

    clearTimeout(indicatorTimeout);
    indicatorTimeout = setTimeout(() => {
      showIndicator = false;
    }, 800);
  }

  function handleKeyPress(event) {
    // Don't handle if user is typing in an input
    if (
      event.target.tagName === "INPUT" ||
      event.target.tagName === "TEXTAREA"
    ) {
      return;
    }

    switch (event.key.toLowerCase()) {
      case " ":
      case "p":
        event.preventDefault();
        togglePlay();
        showShortcutIndicator("pause", playing ? "Pause" : "Play");
        break;
      case "arrowleft":
        event.preventDefault();
        if (videoElement && isFinite(videoElement.currentTime)) {
          const newTime = Math.max(
            0,
            videoElement.currentTime - SEEK_TIME_SHORT,
          );
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime, selectedAudioTrack);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator("seek-backward", "-5s");
        }
        break;
      case "arrowright":
        event.preventDefault();
        if (videoElement && isFinite(videoElement.currentTime) && isFinite(duration)) {
          const newTime = Math.min(
            duration,
            videoElement.currentTime + SEEK_TIME_SHORT,
          );
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime, selectedAudioTrack);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator("seek-forward", "+5s");
        }
        break;
      case "j":
        event.preventDefault();
        if (videoElement && isFinite(videoElement.currentTime)) {
          const newTime = Math.max(
            0,
            videoElement.currentTime - SEEK_TIME_LONG,
          );
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime, selectedAudioTrack);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator("seek-backward", "-10s");
        }
        break;
      case "l":
        event.preventDefault();
        if (videoElement && isFinite(videoElement.currentTime) && isFinite(duration)) {
          const newTime = Math.min(
            duration,
            videoElement.currentTime + SEEK_TIME_LONG,
          );
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime, selectedAudioTrack);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator("seek-forward", "+10s");
        }
        break;
      case "arrowup":
        event.preventDefault();
        volume = Math.min(1, volume + 0.1);
        if (volume > 0 && muted) {
          muted = false;
        }
        showShortcutIndicator("volume-up", `${Math.round(volume * 100)}%`);
        break;
      case "arrowdown":
        event.preventDefault();
        volume = Math.max(0, volume - 0.1);
        showShortcutIndicator("volume-down", `${Math.round(volume * 100)}%`);
        break;
      case "enter":
        event.preventDefault();
        // Skip section if button is visible
        if (showSkipButton && currentSkipSection) {
          skipSection();
        } else if (showNextEpisodeButton) {
          goToNextEpisode();
        }
        break;
    }
  }

  onMount(() => {
    console.log("VideoPlayer mounted");
    document.addEventListener("fullscreenchange", handleFullscreenChange);
    document.addEventListener("webkitfullscreenchange", handleFullscreenChange);
    window.addEventListener("mousemove", handleDrag);
    window.addEventListener("mouseup", stopDrag);
    window.addEventListener("keydown", handleKeyPress);

    // Initialize SRT subtitle renderer for non-demuxer playback
    if (videoElement) {
      srtRenderer = new SRTSubtitleRenderer(videoElement);
      srtRenderer.initialize();
    }

    // Periodic check for skip section state (catches edge cases like keyboard seeking)
    skipSectionCheckInterval = setInterval(() => {
      if (currentSkipSection && videoElement) {
        const stillInSection = videoElement.currentTime >= currentSkipSection.start_time && 
                               videoElement.currentTime < currentSkipSection.end_time;
        if (!stillInSection) {
          currentSkipSection = null;
          showSkipButton = false;
          skipTimerActive = false;
          if (skipButtonTimeout) {
            clearTimeout(skipButtonTimeout);
            skipButtonTimeout = null;
          }
          if (skipTimerInterval) {
            clearInterval(skipTimerInterval);
            skipTimerInterval = null;
          }
        }
      }
    }, 500);

    if (handleId !== null && fileIndex !== null) {
      startStreamProcess();
    } else {
      loading = false;
    }
  });

  onDestroy(() => {
    clearInterval(pollInterval);
    if (progressTrackingInterval) {
      clearInterval(progressTrackingInterval);
    }
    if (skipButtonTimeout) {
      clearTimeout(skipButtonTimeout);
    }
    if (skipTimerInterval) {
      clearInterval(skipTimerInterval);
    }
    if (skipSectionCheckInterval) {
      clearInterval(skipSectionCheckInterval);
    }
    skipTimerActive = false;
    document.removeEventListener("fullscreenchange", handleFullscreenChange);
    document.removeEventListener(
      "webkitfullscreenchange",
      handleFullscreenChange,
    );
    window.removeEventListener("mousemove", handleDrag);
    window.removeEventListener("mouseup", stopDrag);
    window.removeEventListener("keydown", handleKeyPress);
    clearTimeout(controlsTimeout);
    clearTimeout(indicatorTimeout);

    // Cleanup DASH instance
    if (videoElement && videoElement.dashInstance) {
      videoElement.dashInstance.reset();
      videoElement.dashInstance = null;
    }

    if (demuxer) {
      demuxer.destroy();
      demuxer = null;
    }

    if (subtitleRenderer) {
      subtitleRenderer.dispose();
      subtitleRenderer = null;
    }

    if (audioPlayer) {
      // HTML5 Audio cleanup
      if (audioPlayer instanceof Audio) {
        audioPlayer.pause();
        audioPlayer.src = '';
      } else {
        audioPlayer.dispose();
      }
      audioPlayer = null;
    }

    if (srtRenderer) {
      srtRenderer.dispose();
      srtRenderer = null;
    }
  });
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  class="video-player"
  bind:this={videoContainer}
  on:mousemove={handleMouseMove}
  class:fullscreen
  class:hide-cursor={!showControls && playing}
>
  <!-- svelte-ignore a11y-media-has-caption -->
  <video
    bind:this={videoElement}
    {src}
    on:timeupdate={handleTimeUpdate}
    on:loadedmetadata={handleLoadedMetadata}
    on:play={() => (playing = true)}
    on:pause={() => (playing = false)}
    on:click={togglePlay}
  />

  {#if loading}
    <div class="loading-overlay">
      <div class="loading-content">
        <!-- Phase indicator -->
        <div class="loading-phases">
          <div class="loading-phase" class:active={loadingPhase === 'initializing'} class:complete={['buffering', 'metadata', 'transcoding', 'demuxing', 'ready'].includes(loadingPhase)}>
            <div class="phase-icon">
              {#if ['buffering', 'metadata', 'transcoding', 'demuxing', 'ready'].includes(loadingPhase)}
                <i class="ri-check-line"></i>
              {:else}
                <span>1</span>
              {/if}
            </div>
            <span class="phase-label">Initialize</span>
          </div>
          <div class="phase-connector" class:complete={['buffering', 'metadata', 'transcoding', 'demuxing', 'ready'].includes(loadingPhase)}></div>
          <div class="loading-phase" class:active={loadingPhase === 'buffering'} class:complete={['metadata', 'transcoding', 'demuxing', 'ready'].includes(loadingPhase)}>
            <div class="phase-icon">
              {#if ['metadata', 'transcoding', 'demuxing', 'ready'].includes(loadingPhase)}
                <i class="ri-check-line"></i>
              {:else}
                <span>2</span>
              {/if}
            </div>
            <span class="phase-label">Buffer</span>
          </div>
          <div class="phase-connector" class:complete={['metadata', 'transcoding', 'demuxing', 'ready'].includes(loadingPhase)}></div>
          <div class="loading-phase" class:active={loadingPhase === 'metadata'} class:complete={['transcoding', 'demuxing', 'ready'].includes(loadingPhase)}>
            <div class="phase-icon">
              {#if ['transcoding', 'demuxing', 'ready'].includes(loadingPhase)}
                <i class="ri-check-line"></i>
              {:else}
                <span>3</span>
              {/if}
            </div>
            <span class="phase-label">Metadata</span>
          </div>
          <div class="phase-connector" class:complete={['transcoding', 'demuxing', 'ready'].includes(loadingPhase)}></div>
          <div class="loading-phase" class:active={loadingPhase === 'transcoding'} class:complete={['demuxing', 'ready'].includes(loadingPhase)}>
            <div class="phase-icon">
              {#if ['demuxing', 'ready'].includes(loadingPhase)}
                <i class="ri-check-line"></i>
              {:else}
                <span>4</span>
              {/if}
            </div>
            <span class="phase-label">Transcode</span>
          </div>
          <div class="phase-connector" class:complete={['demuxing', 'ready'].includes(loadingPhase)}></div>
          <div class="loading-phase" class:active={loadingPhase === 'demuxing'} class:complete={loadingPhase === 'ready'}>
            <div class="phase-icon">
              {#if loadingPhase === 'ready'}
                <i class="ri-check-line"></i>
              {:else}
                <span>5</span>
              {/if}
            </div>
            <span class="phase-label">Prepare</span>
          </div>
        </div>

        <div class="loading-status">{loadingStatus.status}</div>
        
        <!-- Progress bar -->
        <div class="loading-progress">
          {#if loadingPhase === 'buffering' && loadingStatus.total > 0}
            <!-- Determinate progress bar for buffering with actual progress -->
            <div class="progress-bar-loading">
              <div 
                class="progress-fill"
                style="width: {(loadingStatus.progress / loadingStatus.total) * 100}%"
              ></div>
            </div>
            <div class="loading-stats">
              <span>{(loadingStatus.progress / 1024 / 1024).toFixed(1)} MB / {(loadingStatus.total / 1024 / 1024).toFixed(1)} MB</span>
              {#if loadingStatus.speed > 0}
                <span class="speed-stat">{(loadingStatus.speed / 1024 / 1024).toFixed(1)} MB/s</span>
              {/if}
            </div>
          {:else if loadingPhase === 'transcoding' && loadingStatus.transcodeProgress !== undefined}
            <!-- Determinate progress bar for transcoding -->
            <div class="progress-bar-loading">
              <div 
                class="progress-fill"
                style="width: {loadingStatus.transcodeProgress}%"
              ></div>
            </div>
            <div class="loading-stats">
              <span>{loadingStatus.transcodeProgress.toFixed(0)}% complete</span>
            </div>
          {:else}
            <!-- Indeterminate progress bar for other phases -->
            <div class="progress-bar-loading indeterminate">
              <div class="progress-fill-indeterminate"></div>
            </div>
          {/if}
        </div>
        
        <!-- Peer count only during buffering with actual progress -->
        {#if loadingPhase === 'buffering' && loadingStatus.total > 0 && loadingStatus.peers > 0}
          <div class="loading-stats peer-stats">
            <span class="peer-stat">
              <i class="ri-group-line"></i>
              {loadingStatus.peers} peer{loadingStatus.peers !== 1 ? 's' : ''}
            </span>
          </div>
        {/if}
        
        <button class="cancel-loading-btn" on:click={close}>
          <i class="ri-close-line"></i>
          Cancel
        </button>
      </div>
    </div>
  {/if}

  <div class="player-header" class:visible={showControls}>
    <button class="back-btn" on:click={close}>
      <i class="ri-arrow-left-line"></i>
    </button>
    <div class="player-title">{title}</div>
  </div>

  <!-- SubtitlesOctopus automatically creates and manages its own canvas as a sibling of the video element -->

  <!-- Keyboard shortcut indicator -->
  {#if showIndicator}
    <div class="shortcut-indicator {indicatorType}">
      <div class="indicator-icon">
        {#if indicatorType === "seek-forward"}
          <i class="ri-arrow-right-line"></i>
        {:else if indicatorType === "seek-backward"}
          <i class="ri-arrow-left-line"></i>
        {:else if indicatorType === "volume-up"}
          <i class="ri-volume-up-line"></i>
        {:else if indicatorType === "volume-down"}
          <i class="ri-volume-down-line"></i>
        {:else if indicatorType === "pause"}
          <i class="ri-{playing ? 'play' : 'pause'}-fill"></i>
        {/if}
      </div>
      <div class="indicator-value">{indicatorValue}</div>
    </div>
  {/if}

  <!-- Skip Section Button -->
  {#if chapters && chapters.length > 0 && currentSkipSection && (skipTimerActive || showControls)}
    <button class="skip-button" on:click={skipSection}>
      <span class="skip-text">Skip {currentSkipSection.title}</span>
      <kbd class="skip-kbd">
        <i class="ri-corner-down-left-line"></i>
      </kbd>
      {#if skipTimerActive}
        {#key skipAnimationKey}
          <div class="skip-timer">
            <svg class="skip-timer-spinner" viewBox="0 0 20 20">
              <circle
                class="skip-timer-circle"
                cx="10"
                cy="10"
                r="8"
              />
            </svg>
          </div>
        {/key}
      {/if}
    </button>
  {/if}

  <!-- Next Episode Button -->
  {#if chapters && chapters.length > 0 && showNextEpisodeButton && seasonNum !== null && episodeNum !== null}
    <button class="skip-button next-episode" on:click={goToNextEpisode}>
      <span class="skip-text">Next Episode</span>
      <kbd class="skip-kbd">
        <i class="ri-corner-down-left-line"></i>
      </kbd>
    </button>
  {/if}

  <div class="controls" class:visible={showControls}>
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
      class="progress-bar"
      bind:this={progressBar}
      on:mousedown={startDrag}
      on:mousemove={handleProgressHover}
      on:mouseleave={handleProgressLeave}
    >
      <div
        class="progress-buffered"
        style="width: {(buffered / duration) * 100}%"
      ></div>
      <div
        class="progress-filled"
        style="width: {((isSeeking ? seekPreviewTime : currentTime) /
          duration) *
          100}%; transition: {isSeeking ? 'none' : 'width 0.1s linear'}"
      >
        <div class="progress-handle"></div>
      </div>

      <!-- Chapter markers -->
      {#if chapters && chapters.length > 0}
        {#each chapters as chapter}
          {#if chapter.start_time > 0}
            <div
              class="chapter-marker"
              style="left: {(chapter.start_time / duration) * 100}%"
              title="{formatTime(chapter.start_time)} - {chapter.title ||
                `Chapter ${chapter.index + 1}`}"
            ></div>
          {/if}
        {/each}
      {/if}

      <!-- Hover preview tooltip -->
      {#if hoverTime !== null && !isSeeking}
        {@const hoverChapter = chapters
          .filter((ch) => ch.start_time <= hoverTime)
          .sort((a, b) => b.start_time - a.start_time)[0]}
        <div class="time-tooltip" style="left: {hoverX}px">
          <div class="tooltip-time">{formatTime(hoverTime)}</div>
          {#if hoverChapter}
            <div class="tooltip-chapter">
              {hoverChapter.title || `Chapter ${hoverChapter.index + 1}`}
            </div>
          {/if}
        </div>
      {/if}

      <!-- Seeking preview tooltip -->
      {#if isSeeking}
        {@const seekChapterMatch = chapters
          .filter((ch) => ch.start_time <= seekPreviewTime)
          .sort((a, b) => b.start_time - a.start_time)[0]}
        <div
          class="time-tooltip"
          style="left: {(seekPreviewTime / duration) *
            progressBar?.getBoundingClientRect().width || 0}px"
        >
          <div class="tooltip-time">{formatTime(seekPreviewTime)}</div>
          {#if seekChapterMatch}
            <div class="tooltip-chapter">
              {seekChapterMatch.title ||
                `Chapter ${seekChapterMatch.index + 1}`}
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <div class="control-buttons">
      <button on:click={togglePlay} class="play-btn">
        <i class={playing ? "ri-pause-fill" : "ri-play-fill"}></i>
      </button>

      <span class="time"
        >{formatTime(currentTime)} / {formatTime(duration)}</span
      >

      <div class="volume-control">
        <button on:click={toggleMute} class="volume-btn">
          {#if muted || volume === 0}
            <i class="ri-volume-mute-fill"></i>
          {:else if volume < 0.5}
            <i class="ri-volume-down-fill"></i>
          {:else}
            <i class="ri-volume-up-fill"></i>
          {/if}
        </button>
        <div class="volume-slider-wrapper">
          <div class="volume-slider-track">
            <div
              class="volume-slider-fill"
              style="height: {volume * 100}%"
            ></div>
            <div
              class="volume-slider-thumb"
              style="bottom: {volume * 100}%"
            ></div>
          </div>
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={volume}
            on:input={changeVolume}
            orient="vertical"
            class="volume-slider-input"
          />
        </div>
      </div>

      {#if metadata?.audio_tracks && metadata.audio_tracks.length > 1}
        <div class="player-track-menu-container">
          <button
            on:click={() => {
              showAudioMenu = !showAudioMenu;
              if (showAudioMenu) {
                showSubtitleMenu = false;
                showChaptersMenu = false;
              }
            }}
            class="audio-btn"
          >
            <i class="ri-music-2-line"></i>
          </button>
          {#if showAudioMenu}
            <div class="player-track-dropdown audio-menu">
              {#each metadata.audio_tracks as track, i}
                <button
                  class="player-track-option"
                  class:active={selectedAudioTrack === i}
                  disabled={loadingAudio}
                  on:click={() => selectAudioTrack(i)}
                >
                  <span class="player-track-info">
                    {#if track.language}
                      <span class="player-track-lang"
                        >{track.language.toUpperCase()}</span
                      >
                    {:else}
                      Track {i + 1}
                    {/if}
                    {#if track.name}
                      <span class="player-track-detail">({track.name})</span>
                    {/if}
                  </span>
                  {#if loadingAudio && selectedAudioTrack === i}
                    <div class="loading-spinner-small"></div>
                  {:else if track.codec}
                    <span class="player-track-badge">{track.codec}</span>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      {#if (metadata?.subtitle_tracks && metadata.subtitle_tracks.length > 0) || externalSubtitles.length > 0}
        <div class="player-track-menu-container">
          <button
            on:click={() => {
              showSubtitleMenu = !showSubtitleMenu;
              if (showSubtitleMenu) {
                showAudioMenu = false;
                showChaptersMenu = false;
              }
            }}
            class="subtitle-btn"
          >
            <i class="ri-closed-captioning-line"></i>
          </button>
          {#if showSubtitleMenu}
            <div class="player-track-dropdown subtitle-menu">
              <button
                class="player-track-option"
                class:active={selectedSubtitleTrack === -1}
                on:click={disableSubtitles}
              >
                <span class="player-track-info">Off</span>
              </button>
              {#if metadata?.subtitle_tracks && metadata.subtitle_tracks.length > 0}
                {#each metadata.subtitle_tracks as track, i}
                  <button
                    class="player-track-option"
                    class:active={selectedSubtitleTrack === i}
                    on:click={() => selectSubtitle(track, i)}
                    disabled={loadingSubtitle}
                  >
                    <span class="player-track-info">
                      <span class="player-track-lang">{track.language ? track.language.toUpperCase() : `Subtitle ${i + 1}`}</span>
                      {#if track.name}
                        <span class="player-track-detail">{track.name}</span>
                      {/if}
                    </span>
                    <span class="player-track-badge">{track.codec || 'MKV'}</span>
                    {#if loadingSubtitle && selectedSubtitleTrack === i}
                      <span class="loading-spinner-small"></span>
                    {/if}
                  </button>
                {/each}
              {/if}
              {#if externalSubtitles.length > 0}
                {#each externalSubtitles as track, i}
                  <button
                    class="player-track-option"
                    class:active={selectedSubtitleTrack === (metadata?.subtitle_tracks?.length || 0) + i}
                    on:click={() => selectSubtitle(track, (metadata?.subtitle_tracks?.length || 0) + i)}
                    disabled={loadingSubtitle}
                  >
                    <span class="player-track-info">
                      <span class="player-track-lang">{track.display}</span>
                      {#if track.isHearingImpaired}
                        <span class="player-track-detail">HI</span>
                      {/if}
                    </span>
                    <span class="player-track-badge">WYZIE</span>
                    {#if loadingSubtitle && selectedSubtitleTrack === (metadata?.subtitle_tracks?.length || 0) + i}
                      <span class="loading-spinner-small"></span>
                    {/if}
                  </button>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/if}

      {#if chapters && chapters.length > 0}
        <div class="player-track-menu-container">
          <button
            on:click={() => {
              showChaptersMenu = !showChaptersMenu;
              if (showChaptersMenu) {
                showAudioMenu = false;
                showSubtitleMenu = false;
              }
            }}
            class="chapters-btn"
          >
            <i class="ri-list-check"></i>
          </button>
          {#if showChaptersMenu}
            <div class="player-track-dropdown chapters-menu">
              {#each chapters as chapter}
                <button
                  class="player-track-option"
                  on:click={() => jumpToChapter(chapter.start_time)}
                >
                  <span class="chapter-time"
                    >{formatTime(chapter.start_time)}</span
                  >
                  <span class="chapter-title"
                    >{chapter.title || `Chapter ${chapter.index + 1}`}</span
                  >
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      <button on:click={toggleFullscreen} class="fullscreen-btn">
        <i class={fullscreen ? "ri-fullscreen-exit-line" : "ri-fullscreen-line"}
        ></i>
      </button>
    </div>
  </div>
</div>

<!-- styles moved to src/styles/main.css -->
