<script>
  import { onMount, onDestroy } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { invoke } from '@tauri-apps/api/core';
  import { MKVDemuxer } from './mkvDemuxer.js';
  import { SubtitleRenderer } from './subtitleRenderer.js';
  import { AudioPlayer } from './audioPlayer.js';
  
  export let src = '';
  export let metadata = null;
  export let handleId = null;
  export let fileIndex = null;
  
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
  let indicatorType = ''; // 'seek-forward', 'seek-backward', 'volume-up', 'volume-down'
  let indicatorValue = '';
  let indicatorTimeout;
  
  $: if (videoElement) {
    videoElement.volume = volume;
    videoElement.muted = muted;
  }
  
  $: if (metadata?.chapters) {
    chapters = metadata.chapters;
  }
  
  $: seekChapter = chapters.filter(ch => ch.start_time <= seekPreviewTime)
    .sort((a, b) => b.start_time - a.start_time)[0];
  
  // Initialize demuxer when src changes
  $: if (src && !demuxer) {
    initializeDemuxer();
  }
  
  async function initializeDemuxer() {
    if (demuxer) return; // Already initialized
    
    console.log('initializeDemuxer called with src:', src);
    console.log('Metadata prop:', metadata);
    
    useMkvDemuxer = true;
    
    if (!useMkvDemuxer) return;
    
    console.log('Using frontend demuxer for MKV file');
    
    if (videoElement) {
      videoElement.muted = true;
    }
    
    try {
      demuxer = new MKVDemuxer();
      
      const info = await demuxer.initialize(src);
      
      console.log('Demuxer ready with metadata:', info);
      duration = info.duration;
      
      metadata = {
        audio_tracks: info.audioTracks || [],
        subtitle_tracks: info.subtitleTracks || [],
        chapters: info.chapters || []
      };
      
      console.log('Frontend extracted metadata:', metadata);
      
      if (info.audioTracks && info.audioTracks.length > 0) {
        console.log('Initializing audio player for', info.audioTracks.length, 'tracks');
        audioPlayer = new AudioPlayer();
        await audioPlayer.initialize();
        audioPlayer.setVolume(volume);
        audioPlayer.setMuted(muted);
        
        const firstAudioTrack = info.audioTracks[0];
        audioPlayer.setTrack(firstAudioTrack.id, firstAudioTrack);
        selectedAudioTrack = 0;
      }
      
      if (subtitleCanvas && info.subtitleTracks && info.subtitleTracks.length > 0) {
        console.log('Initializing subtitle renderer for', info.subtitleTracks.length, 'tracks');
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
      
      console.log('Starting demuxer extraction...');
      demuxer.startExtracting();
    } catch (error) {
      console.error('Failed to initialize MKV demuxer:', error);
      console.error('Error details:', error.message, error.stack);
      useMkvDemuxer = false;
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
    document.body.style.userSelect = 'none';
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
      document.body.style.userSelect = '';
      
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
    const container = videoElement.closest('.video-player');
    const appWindow = getCurrentWindow();
    
    if (!fullscreen) {
      try {
        // Check if window is maximized and unmaximize it before entering fullscreen
        const isMaximized = await appWindow.isMaximized();
        if (isMaximized) {
          await appWindow.unmaximize();
          // Wait for the window state to fully update
          await new Promise(resolve => setTimeout(resolve, 200));
        }
        
        // Request fullscreen
        if (container.requestFullscreen) {
          await container.requestFullscreen();
        } else if (container.webkitRequestFullscreen) {
          await container.webkitRequestFullscreen();
        }
      } catch (err) {
        console.error('Fullscreen error:', err);
      }
    } else {
      try {
        if (document.exitFullscreen) {
          await document.exitFullscreen();
        } else if (document.webkitExitFullscreen) {
          await document.webkitExitFullscreen();
        }
      } catch (err) {
        console.error('Exit fullscreen error:', err);
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
    fullscreen = !!(document.fullscreenElement || document.webkitFullscreenElement);
  }
  
  function formatTime(seconds) {
    if (!seconds || isNaN(seconds)) return '0:00';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    }
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
  
  function handleMouseMove() {
    showControls = true;
    clearTimeout(controlsTimeout);
    controlsTimeout = setTimeout(() => {
      if (playing && !isSeeking) showControls = false;
    }, 3000);
  }
  
  function handleMouseLeave() {
    if (!isSeeking && playing) {
      showControls = false;
    }
  }
  
  async function selectAudioTrack(index) {
    console.log('Selecting audio track:', index);
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
      console.warn('Audio track switching not fully implemented for HTTP streams');
    }
    
    showAudioMenu = false;
  }
  
  async function selectSubtitle(track, trackIndex) {
    console.log('Selecting subtitle track:', track);
    selectedSubtitleTrack = trackIndex;
    
    if (useMkvDemuxer && subtitleRenderer && track.data) {
      try {
        await subtitleRenderer.loadSubtitleTrack(track.data, track.codec);
        subtitleRenderer.show();
      } catch (error) {
        console.error('Failed to load subtitle track:', error);
      }
    } else {
      while (videoElement.textTracks.length > 0) {
        const trackElement = videoElement.querySelector('track');
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
      const tracks = videoElement.querySelectorAll('track');
      tracks.forEach(track => track.remove());
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
    if (event.target.tagName === 'INPUT' || event.target.tagName === 'TEXTAREA') {
      return;
    }

    switch(event.key.toLowerCase()) {
      case ' ':
      case 'p':
        event.preventDefault();
        togglePlay();
        showShortcutIndicator('pause', playing ? 'Pause' : 'Play');
        break;
      case 'arrowleft':
        event.preventDefault();
        if (videoElement) {
          const newTime = Math.max(0, videoElement.currentTime - 5);
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator('seek-backward', '-5s');
        }
        break;
      case 'arrowright':
        event.preventDefault();
        if (videoElement) {
          const newTime = Math.min(duration, videoElement.currentTime + 5);
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator('seek-forward', '+5s');
        }
        break;
      case 'j':
        event.preventDefault();
        if (videoElement) {
          const newTime = Math.max(0, videoElement.currentTime - 10);
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator('seek-backward', '-10s');
        }
        break;
      case 'l':
        event.preventDefault();
        if (videoElement) {
          const newTime = Math.min(duration, videoElement.currentTime + 10);
          videoElement.currentTime = newTime;
          if (useMkvDemuxer) {
            if (demuxer) demuxer.seek(newTime);
            if (audioPlayer) audioPlayer.seek(newTime);
          }
          showShortcutIndicator('seek-forward', '+10s');
        }
        break;
      case 'arrowup':
        event.preventDefault();
        volume = Math.min(1, volume + 0.1);
        showShortcutIndicator('volume-up', `${Math.round(volume * 100)}%`);
        break;
      case 'arrowdown':
        event.preventDefault();
        volume = Math.max(0, volume - 0.1);
        showShortcutIndicator('volume-down', `${Math.round(volume * 100)}%`);
        break;
    }
  }
  
  onMount(() => {
    console.log('VideoPlayer mounted');
    document.addEventListener('fullscreenchange', handleFullscreenChange);
    document.addEventListener('webkitfullscreenchange', handleFullscreenChange);
    window.addEventListener('mousemove', handleDrag);
    window.addEventListener('mouseup', stopDrag);
    window.addEventListener('keydown', handleKeyPress);
  });
  
  onDestroy(() => {
    document.removeEventListener('fullscreenchange', handleFullscreenChange);
    document.removeEventListener('webkitfullscreenchange', handleFullscreenChange);
    window.removeEventListener('mousemove', handleDrag);
    window.removeEventListener('mouseup', stopDrag);
    window.removeEventListener('keydown', handleKeyPress);
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
  on:mouseleave={handleMouseLeave}
  class:fullscreen
  class:hide-cursor={!showControls && playing}
>
  <!-- svelte-ignore a11y-media-has-caption -->
  <video
    bind:this={videoElement}
    {src}
    on:timeupdate={handleTimeUpdate}
    on:loadedmetadata={handleLoadedMetadata}
    on:play={() => playing = true}
    on:pause={() => playing = false}
    on:click={togglePlay}
  />
  
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
        {#if indicatorType === 'seek-forward'}
          <i class="ri-arrow-right-line"></i>
        {:else if indicatorType === 'seek-backward'}
          <i class="ri-arrow-left-line"></i>
        {:else if indicatorType === 'volume-up'}
          <i class="ri-volume-up-line"></i>
        {:else if indicatorType === 'volume-down'}
          <i class="ri-volume-down-line"></i>
        {:else if indicatorType === 'pause'}
          <i class="ri-{playing ? 'pause' : 'play'}-fill"></i>
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
      <div class="progress-buffered" style="width: {(buffered / duration) * 100}%"></div>
      <div class="progress-filled" style="width: {(isSeeking ? seekPreviewTime : currentTime) / duration * 100}%; transition: {isSeeking ? 'none' : 'width 0.1s linear'}">
        <div class="progress-handle"></div>
      </div>
      
      <!-- Chapter markers -->
      {#if chapters && chapters.length > 0}
        {#each chapters as chapter}
          {#if chapter.start_time > 0}
            <div 
              class="chapter-marker" 
              style="left: {(chapter.start_time / duration) * 100}%"
              title="{formatTime(chapter.start_time)} - {chapter.title || `Chapter ${chapter.index + 1}`}"
            ></div>
          {/if}
        {/each}
      {/if}
      
      <!-- Hover preview tooltip -->
      {#if hoverTime !== null && !isSeeking}
        {@const hoverChapter = chapters.filter(ch => ch.start_time <= hoverTime).sort((a, b) => b.start_time - a.start_time)[0]}
        <div class="time-tooltip" style="left: {hoverX}px">
          <div class="tooltip-time">{formatTime(hoverTime)}</div>
          {#if hoverChapter}
            <div class="tooltip-chapter">{hoverChapter.title || `Chapter ${hoverChapter.index + 1}`}</div>
          {/if}
        </div>
      {/if}
      
      <!-- Seeking preview tooltip -->
      {#if isSeeking}
        {@const seekChapterMatch = chapters.filter(ch => ch.start_time <= seekPreviewTime).sort((a, b) => b.start_time - a.start_time)[0]}
        <div class="time-tooltip" style="left: {((seekPreviewTime / duration) * progressBar?.getBoundingClientRect().width) || 0}px">
          <div class="tooltip-time">{formatTime(seekPreviewTime)}</div>
          {#if seekChapterMatch}
            <div class="tooltip-chapter">{seekChapterMatch.title || `Chapter ${seekChapterMatch.index + 1}`}</div>
          {/if}
        </div>
      {/if}
    </div>
    
    <div class="control-buttons">
      <button on:click={togglePlay} class="play-btn">
        <i class="{playing ? 'ri-pause-fill' : 'ri-play-fill'}"></i>
      </button>
      
      <span class="time">{formatTime(currentTime)} / {formatTime(duration)}</span>
      
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
            <div class="volume-slider-fill" style="height: {volume * 100}%"></div>
            <div class="volume-slider-thumb" style="bottom: {volume * 100}%"></div>
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
          <button on:click={() => {
            showAudioMenu = !showAudioMenu;
            if (showAudioMenu) {
              showSubtitleMenu = false;
              showChaptersMenu = false;
            }
          }} class="audio-btn">
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
                      <span class="language-code">{track.language.toUpperCase()}</span>
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
          <button on:click={() => {
            showSubtitleMenu = !showSubtitleMenu;
            if (showSubtitleMenu) {
              showAudioMenu = false;
              showChaptersMenu = false;
            }
          }} class="subtitle-btn">
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
                      <span class="language-code">{track.language.toUpperCase()}</span>
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
          <button on:click={() => {
            showChaptersMenu = !showChaptersMenu;
            if (showChaptersMenu) {
              showAudioMenu = false;
              showSubtitleMenu = false;
            }
          }} class="chapters-btn">
            <i class="ri-list-check"></i>
          </button>
          {#if showChaptersMenu}
            <div class="menu chapters-menu">
              {#each chapters as chapter}
                <button class="menu-item" on:click={() => jumpToChapter(chapter.start_time)}>
                  <span class="chapter-time">{formatTime(chapter.start_time)}</span>
                  <span class="chapter-title">{chapter.title || `Chapter ${chapter.index + 1}`}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
      
      <button on:click={toggleFullscreen} class="fullscreen-btn">
        <i class="{fullscreen ? 'ri-fullscreen-exit-line' : 'ri-fullscreen-line'}"></i>
      </button>
    </div>
  </div>
</div>


<style>
  .video-player {
    position: relative;
    width: 100%;
    background: var(--bg-primary);
    border-radius: var(--border-radius-lg);
    overflow: hidden;
    cursor: default;
    box-shadow: var(--shadow-highlight), var(--shadow-depth);
    font-family: 'Geist Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    isolation: isolate;
  }

  .video-player.hide-cursor {
    cursor: none;
  }

  .video-player.hide-cursor video {
    cursor: none;
  }

  .video-player.fullscreen {
    border-radius: 0;
    width: 100vw;
    height: 100vh;
  }

  .video-player.fullscreen video {
    width: 100%;
    height: 100%;
    object-fit: contain;
    border-radius: 0;
  }

  video {
    width: 100%;
    height: auto;
    display: block;
    cursor: pointer;
    border-radius: var(--border-radius-lg);
  }

  .subtitle-canvas {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 1;
    border-radius: var(--border-radius-lg);
  }

  .fullscreen .subtitle-canvas {
    border-radius: 0;
  }

  .shortcut-indicator {
    position: absolute;
    top: 50%;
    left: 32px;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 12px 20px;
    pointer-events: none;
    z-index: 1000;
    animation: indicatorSlideInLeft 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .shortcut-indicator.seek-forward {
    left: auto;
    right: 32px;
    animation: indicatorSlideInRight 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .shortcut-indicator.pause,
  .shortcut-indicator.volume-up,
  .shortcut-indicator.volume-down {
    top: 32px;
    left: 50%;
    transform: translateX(-50%);
    animation: indicatorSlideInTop 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  @keyframes indicatorSlideInLeft {
    from {
      opacity: 0;
      transform: translateY(-50%) translateX(-20px);
    }
    to {
      opacity: 1;
      transform: translateY(-50%) translateX(0);
    }
  }

  @keyframes indicatorSlideInRight {
    from {
      opacity: 0;
      transform: translateY(-50%) translateX(20px);
    }
    to {
      opacity: 1;
      transform: translateY(-50%) translateX(0);
    }
  }

  @keyframes indicatorSlideInTop {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(-20px);
    }
    to {
      opacity: 1;
      transform: translateX(-50%) translateY(0);
    }
  }

  .indicator-icon {
    font-size: 24px;
    color: rgba(255, 255, 255, 0.9);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .indicator-icon i {
    font-size: 24px;
  }

  .indicator-value {
    font-family: 'Geist Sans Variable', sans-serif;
    font-size: 16px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    letter-spacing: 0.02em;
  }

  .controls {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: linear-gradient(to top, 
      rgba(0, 0, 0, 0.95) 0%, 
      rgba(0, 0, 0, 0.85) 20%, 
      rgba(0, 0, 0, 0.7) 40%, 
      rgba(0, 0, 0, 0.5) 60%, 
      rgba(0, 0, 0, 0.3) 80%, 
      transparent 100%);
    padding: var(--spacing-3xl) var(--spacing-xl) var(--spacing-lg);
    padding-top: 100px;
    opacity: 0;
    transition: opacity 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    pointer-events: none;
  }

  .controls.visible {
    opacity: 1;
    pointer-events: auto;
  }

  .progress-bar {
    width: 100%;
    height: 6px;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 3px;
    cursor: pointer;
    margin-bottom: var(--spacing-lg);
    position: relative;
    transition: height 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    overflow: visible;
  }

  .progress-bar:hover {
    height: 8px;
  }

  .progress-bar:hover .progress-handle {
    opacity: 1;
    transform: scale(1);
  }

  .progress-buffered {
    position: absolute;
    height: 100%;
    background: rgba(255, 255, 255, 0.25);
    border-radius: 3px;
    transition: width 0.3s ease;
    pointer-events: none;
  }

  .progress-filled {
    position: absolute;
    height: 100%;
    background: var(--accent-color);
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    pointer-events: none;
    min-width: 0;
  }

  .progress-handle {
    width: 14px;
    height: 14px;
    background: white;
    border-radius: 50%;
    margin-right: -7px;
    opacity: 0;
    transform: scale(0.8);
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4), 0 0 0 2px var(--accent-color);
    flex-shrink: 0;
  }

  .progress-bar:hover .progress-handle {
    opacity: 1;
    transform: scale(1);
  }

  .control-buttons {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
    color: var(--text-primary);
  }

  button {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: var(--text-primary);
    cursor: pointer;
    padding: var(--spacing-sm);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }

  button:hover {
    background: rgba(255, 255, 255, 0.15);
    border-color: rgba(255, 255, 255, 0.25);
    transform: scale(1.05);
  }

  button:active {
    transform: scale(0.95);
  }

  button i {
    font-size: 20px;
    line-height: 1;
  }

  .time {
    font-size: 13px;
    color: var(--text-primary);
    margin-left: var(--spacing-sm);
    font-weight: 500;
    font-family: 'Geist Mono Variable', monospace;
    letter-spacing: 0.5px;
  }

  .volume-control {
    position: relative;
    display: flex;
    align-items: center;
    margin-left: auto;
  }

  .volume-slider-wrapper {
    position: absolute;
    bottom: calc(100% - 8px);
    left: 50%;
    transform: translateX(-50%);
    background: rgba(0, 0, 0, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 14px;
    padding: 14px 10px 22px 10px;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    z-index: 100;
  }

  .volume-control:hover .volume-slider-wrapper,
  .volume-slider-wrapper:hover {
    opacity: 1;
    pointer-events: auto;
  }

  .volume-slider-track {
    position: relative;
    width: 6px;
    height: 100px;
    background: rgba(255, 255, 255, 0.2);
    border-radius: 3px;
    pointer-events: none;
  }

  .volume-slider-fill {
    position: absolute;
    bottom: 0;
    left: 0;
    width: 100%;
    background: var(--accent-color);
    border-radius: 3px;
    pointer-events: none;
  }

  .volume-slider-thumb {
    position: absolute;
    left: 50%;
    transform: translate(-50%, 50%);
    width: 14px;
    height: 14px;
    background: #ffffff;
    border-radius: 50%;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4), 0 0 0 2px var(--accent-color);
    pointer-events: none;
  }

  .volume-slider-input {
    position: absolute;
    top: 0;
    left: 50%;
    transform: translateX(-50%);
    width: 34px;
    height: 100px;
    opacity: 0;
    cursor: pointer;
    -webkit-appearance: slider-vertical;
    appearance: slider-vertical;
    writing-mode: bt-lr;
  }

  .time {
    height: 6px;
  }

  /* Fullscreen-specific adjustments */
  .fullscreen .controls {
    padding: var(--spacing-3xl) var(--spacing-3xl) var(--spacing-2xl);
  }

  .fullscreen .control-buttons > button,
  .fullscreen .volume-control > button,
  .fullscreen .menu-wrapper > button {
    width: 48px;
    height: 48px;
  }

  .fullscreen button i {
    font-size: 24px;
  }

  .fullscreen .menu-item i {
    font-size: inherit;
  }

  .fullscreen .time {
    font-size: 15px;
  }

  .fullscreen .progress-bar {
    height: 8px;
    margin-bottom: var(--spacing-xl);
  }

  .fullscreen .progress-bar:hover {
    height: 10px;
  }

  .fullscreen .progress-handle {
    width: 16px;
    height: 16px;
    margin-right: -8px;
  }

  .fullscreen .menu {
    max-height: 400px;
    font-size: 15px;
    min-width: 350px;
  }

  .fullscreen .menu-item {
    padding: var(--spacing-lg) var(--spacing-xl);
  }

  /* Menu styles */
  .menu-wrapper {
    position: relative;
  }

  .menu {
    position: absolute;
    bottom: 100%;
    right: 0;
    margin-bottom: var(--spacing-md);
    background: rgba(10, 10, 10, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: var(--border-radius-md);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    box-shadow: var(--shadow-highlight), var(--shadow-depth);
    min-width: 240px;
    max-height: 300px;
    overflow-y: auto;
    z-index: 100;
    animation: slideUp 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .menu-item {
    width: 100%;
    padding: var(--spacing-md) var(--spacing-lg);
    background: transparent;
    border: none;
    border-radius: 0;
    color: var(--text-primary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition: background 0.2s;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-sm);
    min-height: 40px;
    height: auto;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  .menu-item:hover {
    background: rgba(255, 255, 255, 0.1);
    transform: none;
    border-color: transparent;
  }

  .menu-item .codec {
    font-size: 11px;
    color: var(--text-secondary);
    font-family: 'Geist Mono Variable', monospace;
    flex-shrink: 0;
  }

  .chapter-time {
    font-family: 'Geist Mono Variable', monospace;
    color: var(--text-secondary);
    font-size: 11px;
    flex-shrink: 0;
  }

  .chapter-title {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .menu::-webkit-scrollbar {
    width: 6px;
  }

  .menu::-webkit-scrollbar-track {
    background: rgba(255, 255, 255, 0.05);
  }

  .menu::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.2);
    border-radius: 3px;
  }

  .menu::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.3);
  }

  /* Chapter markers on timeline */
  .chapter-marker {
    position: absolute;
    top: -2px;
    bottom: -2px;
    width: 2px;
    background: rgba(255, 255, 255, 0.6);
    border-radius: 1px;
    pointer-events: none;
    z-index: 2;
  }

  /* Time tooltip */
  .time-tooltip {
    position: absolute;
    bottom: 100%;
    transform: translateX(-50%);
    margin-bottom: 12px;
    background: rgba(0, 0, 0, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 8px;
    padding: 8px 12px;
    pointer-events: none;
    white-space: nowrap;
    z-index: 10;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  }

  .tooltip-time {
    font-family: 'Geist Mono Variable', monospace;
    font-size: 13px;
    color: white;
    font-weight: 600;
    text-align: center;
  }

  .tooltip-chapter {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.7);
    margin-top: 4px;
    text-align: center;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Active menu item */
  .menu-item.active {
    background: rgba(168, 85, 247, 0.2);
    border-left: 3px solid var(--accent-color);
  }

  .menu-item.active:hover {
    background: rgba(168, 85, 247, 0.3);
  }

  .track-label {
    flex: 1;
    text-align: left;
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }

  .language-code {
    font-family: 'Geist Mono Variable', 'Geist Mono', monospace;
    font-weight: 600;
    letter-spacing: 0.05em;
    flex-shrink: 0;
  }

  .track-name {
    color: var(--text-secondary);
    font-size: 11px;
  }
</style>

