<script>
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { MKVDemuxer } from "./mkvDemuxer.js";
  import { SubtitleRenderer } from "./subtitleRenderer.js";
  import { AudioPlayer } from "./audioPlayer.js";
  import { formatTime } from "./utils/timeUtils.js";

  import { createEventDispatcher } from "svelte";

  export let src = "";
  export let metadata = null;
  export let title = "";
  export let handleId = null;
  export let fileIndex = null;

  let loading = true;
  let loadingStatus = {
    progress: 0,
    total: 0,
    speed: 0,
    peers: 0,
    status: "Initializing...",
    state: "checking",
  };
  let pollInterval;

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
  let audioPlayer = null;
  let useMkvDemuxer = false;

  // Track/subtitle selection
  let showAudioMenu = false;
  let showSubtitleMenu = false;
  let showChaptersMenu = false;
  let selectedAudioTrack = 0;
  let selectedSubtitleTrack = -1;
  let chapters = [];

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

  $: if (videoElement) {
    videoElement.volume = volume;
    videoElement.muted = muted;
  }

  $: if (metadata?.chapters) {
    chapters = metadata.chapters;
  }

  $: seekChapter = chapters
    .filter((ch) => ch.start_time <= seekPreviewTime)
    .sort((a, b) => b.start_time - a.start_time)[0];

  // Initialize demuxer when src changes
  $: if (src && !demuxer) {
    initializeDemuxer();
  }

  async function initializeDemuxer() {
    if (demuxer) return; // Already initialized

    console.log("initializeDemuxer called with src:", src);
    console.log("Metadata prop:", metadata);

    useMkvDemuxer = true;

    if (!useMkvDemuxer) return;

    console.log("Using frontend demuxer for MKV file");

    if (videoElement) {
      videoElement.muted = true;
    }

    try {
      demuxer = new MKVDemuxer();

      const info = await demuxer.initialize(src);

      console.log("Demuxer ready with metadata:", info);
      duration = info.duration;

      metadata = {
        audio_tracks: info.audioTracks || [],
        subtitle_tracks: info.subtitleTracks || [],
        chapters: info.chapters || [],
      };

      console.log("Frontend extracted metadata:", metadata);

      if (info.audioTracks && info.audioTracks.length > 0) {
        console.log(
          "Initializing audio player for",
          info.audioTracks.length,
          "tracks",
        );
        audioPlayer = new AudioPlayer();
        await audioPlayer.initialize();
        audioPlayer.setVolume(volume);
        audioPlayer.setMuted(muted);

        const firstAudioTrack = info.audioTracks[0];
        audioPlayer.setTrack(firstAudioTrack.id, firstAudioTrack);
        selectedAudioTrack = 0;
        
        // Start audio player immediately and ensure context is active
        audioPlayer.isPlaying = true;
        if (audioPlayer.audioContext.state === 'suspended') {
          await audioPlayer.audioContext.resume();
        }
      }

      if (
        subtitleCanvas &&
        info.subtitleTracks &&
        info.subtitleTracks.length > 0
      ) {
        console.log(
          "Initializing subtitle renderer for",
          info.subtitleTracks.length,
          "tracks",
        );
        subtitleRenderer = new SubtitleRenderer(subtitleCanvas, videoElement);
        await subtitleRenderer.initialize();
      }

      demuxer.onVideoSamples = (samples) => {
        // Video samples handled by video element
      };

      demuxer.onAudioSamples = (trackId, samples) => {
        if (audioPlayer && audioPlayer.currentTrackId === trackId) {
          audioPlayer.decodeAndScheduleAudio(samples);
        }
      };

      console.log("Starting demuxer extraction...");
      demuxer.startExtracting();
    } catch (error) {
      console.error("Failed to initialize MKV demuxer:", error);
      console.error("Error details:", error.message, error.stack);
      useMkvDemuxer = false;
    }
  }

  async function close() {
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
    try {
      await invoke("prepare_stream", { handleId, fileIndex });
      pollInterval = setInterval(checkStreamStatus, 500);
    } catch (err) {
      console.error("Failed to prepare stream:", err);
      loadingStatus.status = "Error: " + err;
    }
  }

  async function checkStreamStatus() {
    try {
      const status = await invoke("get_stream_status", { handleId, fileIndex });
      console.log("Stream status:", status);

      loadingStatus = {
        progress: status.progress_bytes,
        total: status.total_bytes,
        speed: status.download_speed,
        peers: status.peers,
        status: status.status,
        state: status.state,
      };

      if (status.status === "ready" && status.stream_info) {
        clearInterval(pollInterval);
        src = status.stream_info.url;
        // Merge metadata if provided
        if (status.stream_info.metadata) {
          metadata = status.stream_info.metadata;
        }
        loading = false;
        // Auto play - start muted to bypass autoplay restrictions, then unmute
        setTimeout(async () => {
          if (videoElement) {
            try {
              // Start muted to ensure autoplay works
              const wasMuted = videoElement.muted;
              videoElement.muted = true;
              await videoElement.play();
              // Unmute after play starts successfully
              setTimeout(() => {
                if (videoElement && !wasMuted) {
                  videoElement.muted = false;
                }
              }, 100);
              playing = true;
            } catch (err) {
              console.error("Autoplay failed:", err);
              // If autoplay fails, user will need to click play
            }
          }
        }, 100);
      }
    } catch (err) {
      console.error("Error checking status:", err);
    }
  }

  async function togglePlay() {
    if (useMkvDemuxer && audioPlayer) {
      if (playing) {
        videoElement.pause();
        audioPlayer.pause();
        playing = false;
      } else {
        videoElement.play();
        audioPlayer.play();
        playing = true;
      }
    } else {
      if (videoElement.paused) {
        videoElement.play();
        playing = true;
      } else {
        videoElement.pause();
        playing = false;
      }
    }
  }

  function startDrag(e) {
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

      if (videoElement && duration) {
        videoElement.currentTime = seekPreviewTime;
        currentTime = seekPreviewTime;

        if (useMkvDemuxer) {
          if (demuxer) demuxer.seek(seekPreviewTime);
          if (audioPlayer) audioPlayer.seek(seekPreviewTime);
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
    if (!progressBar || !duration) return;
    const rect = progressBar.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = Math.max(0, Math.min(1, x / rect.width));
    const seekTime = percentage * duration;

    videoElement.currentTime = seekTime;

    if (useMkvDemuxer) {
      if (demuxer) {
        demuxer.seek(seekTime);
      }
      if (audioPlayer) {
        audioPlayer.seek(seekTime);
      }
    }
  }

  function toggleMute() {
    muted = !muted;
    if (audioPlayer) {
      audioPlayer.setMuted(muted);
    }
  }

  function changeVolume(e) {
    volume = parseFloat(e.target.value);
    if (volume > 0) muted = false;
    if (audioPlayer) {
      audioPlayer.setVolume(volume);
    }
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
    currentTime = videoElement.currentTime;

    if (useMkvDemuxer && subtitleRenderer) {
      subtitleRenderer.updateTime(currentTime);
    }

    if (videoElement.buffered.length > 0) {
      buffered = videoElement.buffered.end(videoElement.buffered.length - 1);
    }
  }

  function handleLoadedMetadata() {
    duration = videoElement.duration;

    if (useMkvDemuxer && subtitleRenderer && subtitleCanvas) {
      const width = videoElement.videoWidth || videoElement.clientWidth;
      const height = videoElement.videoHeight || videoElement.clientHeight;
      subtitleRenderer.resize(width, height);
    }
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
    selectedAudioTrack = index;

    if (useMkvDemuxer && audioPlayer && demuxer) {
      const audioTrack = demuxer.audioTracks[index];
      if (audioTrack) {
        audioPlayer.stop();
        audioPlayer.setTrack(audioTrack.id, audioTrack);

        demuxer.onAudioSamples = (trackId, samples) => {
          if (trackId === audioTrack.id) {
            audioPlayer.decodeAndScheduleAudio(samples);
          }
        };

        if (playing) {
          audioPlayer.play();
        }
      }
    } else {
      console.warn(
        "Audio track switching not fully implemented for HTTP streams",
      );
    }

    showAudioMenu = false;
  }

  async function selectSubtitle(track, trackIndex) {
    console.log("Selecting subtitle track:", track);
    selectedSubtitleTrack = trackIndex;

    if (useMkvDemuxer && subtitleRenderer && track.data) {
      try {
        await subtitleRenderer.loadSubtitleTrack(track.data, track.codec);
        subtitleRenderer.show();
      } catch (error) {
        console.error("Failed to load subtitle track:", error);
      }
    } else {
      while (videoElement.textTracks.length > 0) {
        const trackElement = videoElement.querySelector("track");
        if (trackElement) trackElement.remove();
      }
    }

    showSubtitleMenu = false;
  }

  function disableSubtitles() {
    selectedSubtitleTrack = -1;

    if (useMkvDemuxer && subtitleRenderer) {
      subtitleRenderer.hide();
    } else if (videoElement) {
      const tracks = videoElement.querySelectorAll("track");
      tracks.forEach((track) => track.remove());
    }

    showSubtitleMenu = false;
  }

  function jumpToChapter(startTime) {
    if (videoElement) {
      videoElement.currentTime = startTime;

      if (useMkvDemuxer) {
        if (demuxer) demuxer.seek(startTime);
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
        if (videoElement) {
          const newTime = Math.max(
            0,
            videoElement.currentTime - SEEK_TIME_SHORT,
          );
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator("seek-backward", "-5s");
        }
        break;
      case "arrowright":
        event.preventDefault();
        if (videoElement) {
          const newTime = Math.min(
            duration,
            videoElement.currentTime + SEEK_TIME_SHORT,
          );
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator("seek-forward", "+5s");
        }
        break;
      case "j":
        event.preventDefault();
        if (videoElement) {
          const newTime = Math.max(
            0,
            videoElement.currentTime - SEEK_TIME_LONG,
          );
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator("seek-backward", "-10s");
        }
        break;
      case "l":
        event.preventDefault();
        if (videoElement) {
          const newTime = Math.min(
            duration,
            videoElement.currentTime + SEEK_TIME_LONG,
          );
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator("seek-forward", "+10s");
        }
        break;
      case "arrowup":
        event.preventDefault();
        volume = Math.min(1, volume + 0.1);
        showShortcutIndicator("volume-up", `${Math.round(volume * 100)}%`);
        break;
      case "arrowdown":
        event.preventDefault();
        volume = Math.max(0, volume - 0.1);
        showShortcutIndicator("volume-down", `${Math.round(volume * 100)}%`);
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

    if (handleId !== null && fileIndex !== null) {
      startStreamProcess();
    } else {
      loading = false;
    }
  });

  onDestroy(() => {
    clearInterval(pollInterval);
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

    if (demuxer) {
      demuxer.destroy();
      demuxer = null;
    }

    if (subtitleRenderer) {
      subtitleRenderer.dispose();
      subtitleRenderer = null;
    }

    if (audioPlayer) {
      audioPlayer.dispose();
      audioPlayer = null;
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
        <div class="spinner"></div>
        <div class="loading-status">{loadingStatus.status}</div>
        {#if loadingStatus.total > 0}
          <div class="loading-progress">
            <div class="progress-bar-loading">
              <div 
                class="progress-fill"
                style="width: {(loadingStatus.progress / loadingStatus.total) * 100}%"
              ></div>
            </div>
            <div class="loading-stats">
              <span>{(loadingStatus.progress / 1024 / 1024).toFixed(1)} MB / {(loadingStatus.total / 1024 / 1024).toFixed(1)} MB</span>
            </div>
          </div>
        {:else}
          <div class="loading-stats">
            <span>{loadingStatus.peers} peers</span>
          </div>
        {/if}
        <button class="cancel-loading-btn" on:click={close}>Cancel</button>
      </div>
    </div>
  {/if}

  <div class="player-header" class:visible={showControls}>
    <button class="back-btn" on:click={close}>
      <i class="ri-arrow-left-line"></i>
    </button>
    <div class="player-title">{title}</div>
  </div>

  {#if useMkvDemuxer}
    <canvas
      bind:this={subtitleCanvas}
      class="subtitle-canvas"
      style="display: none;"
    ></canvas>
  {/if}

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
        <div class="menu-wrapper">
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
            <div class="menu audio-menu">
              {#each metadata.audio_tracks as track, i}
                <button
                  class="menu-item"
                  class:active={selectedAudioTrack === i}
                  on:click={() => selectAudioTrack(i)}
                >
                  <span class="track-label">
                    {#if track.language}
                      <span class="language-code"
                        >{track.language.toUpperCase()}</span
                      >
                    {:else}
                      Track {i + 1}
                    {/if}
                    {#if track.name}
                      <span class="track-name">({track.name})</span>
                    {/if}
                  </span>
                  {#if track.codec}
                    <span class="codec">{track.codec}</span>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      {#if metadata?.subtitle_tracks && metadata.subtitle_tracks.length > 0}
        <div class="menu-wrapper">
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
            <div class="menu subtitle-menu">
              <button
                class="menu-item"
                class:active={selectedSubtitleTrack === -1}
                on:click={disableSubtitles}
              >
                Off
              </button>
              {#each metadata.subtitle_tracks as track, i}
                <button
                  class="menu-item"
                  class:active={selectedSubtitleTrack === i}
                  on:click={() => selectSubtitle(track, i)}
                >
                  <span class="track-label">
                    {#if track.language}
                      <span class="language-code"
                        >{track.language.toUpperCase()}</span
                      >
                    {:else}
                      Subtitle {i + 1}
                    {/if}
                    {#if track.name}
                      <span class="track-name">({track.name})</span>
                    {/if}
                  </span>
                  {#if track.codec}
                    <span class="codec">{track.codec}</span>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      {#if chapters && chapters.length > 0}
        <div class="menu-wrapper">
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
            <div class="menu chapters-menu">
              {#each chapters as chapter}
                <button
                  class="menu-item"
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
