<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import VideoPlayer from "./VideoPlayer.svelte";

  let magnetLink = "";
  let torrents = [];
  let selectedTorrent = null;
  let selectedFileIndex = null;
  let streamUrl = "";
  let streamMetadata = null;
  let loading = false;
  let error = "";
  let downloadDir = "";

  const REFRESH_INTERVAL = 2000;

  onMount(async () => {
    await loadTorrents();
    await loadDownloadDir();
    // Refresh torrents periodically
    const interval = setInterval(loadTorrents, REFRESH_INTERVAL);
    return () => clearInterval(interval);
  });

  async function loadDownloadDir() {
    try {
      downloadDir = await invoke("get_download_dir");
    } catch (err) {
      console.error("Failed to get download dir:", err);
    }
  }

  async function loadTorrents() {
    try {
      torrents = await invoke("list_torrents");
    } catch (err) {
      console.error("Failed to load torrents:", err);
    }
  }

  async function addTorrent() {
    if (!magnetLink.trim()) {
      error = "Please enter a magnet link or torrent URL";
      return;
    }

    loading = true;
    error = "";

    try {
      const handleId = await invoke("add_torrent", { magnetOrUrl: magnetLink });
      console.log("Added torrent with handle:", handleId);

      magnetLink = "";
      await loadTorrents();

      // Auto-select the new torrent
      const torrentInfo = await invoke("get_torrent_info", { handleId });
      selectedTorrent = torrentInfo;
    } catch (err) {
      error = `Failed to add torrent: ${err}`;
      console.error("Full error:", err);
      console.error("Error details:", JSON.stringify(err, null, 2));
    } finally {
      loading = false;
    }
  }

  async function selectTorrent(torrent) {
    try {
      const torrentInfo = await invoke("get_torrent_info", {
        handleId: torrent.handle_id,
      });
      selectedTorrent = torrentInfo;
      selectedFileIndex = null;
      streamUrl = "";
    } catch (err) {
      error = `Failed to get torrent info: ${err}`;
      console.error(err);
    }
  }

  async function startStream(fileIndex) {
    if (selectedTorrent === null) return;

    loading = true;
    error = "";

    try {
      const streamInfo = await invoke("start_stream", {
        handleId: selectedTorrent.handle_id,
        fileIndex: fileIndex,
      });

      streamUrl = streamInfo.url;
      streamMetadata = streamInfo.metadata;
      selectedFileIndex = fileIndex;
      console.log("Stream started:", streamInfo);
      console.log("Stream URL:", streamUrl);

      // For MKV files, metadata will be extracted by the frontend demuxer
      const isMkvFile =
        streamUrl.toLowerCase().includes(".mkv") ||
        selectedTorrent.files[fileIndex]?.name.toLowerCase().endsWith(".mkv");

      if (isMkvFile) {
        console.log(
          "MKV file detected - metadata will be extracted by frontend demuxer",
        );
        streamMetadata = null;
      } else if (streamInfo.metadata) {
        console.log("Backend metadata:", streamInfo.metadata);
        console.log("Audio tracks:", streamInfo.metadata.audio_tracks);
        console.log("Subtitle tracks:", streamInfo.metadata.subtitle_tracks);
        console.log("Chapters:", streamInfo.metadata.chapters);
      }
    } catch (err) {
      error = `Failed to start stream: ${err}`;
      console.error(err);
    } finally {
      loading = false;
    }
  }

  async function pauseTorrent(handleId) {
    try {
      await invoke("pause_torrent", { handleId });
      await loadTorrents();
    } catch (err) {
      error = `Failed to pause torrent: ${err}`;
    }
  }

  async function resumeTorrent(handleId) {
    try {
      await invoke("resume_torrent", { handleId });
      await loadTorrents();
    } catch (err) {
      error = `Failed to resume torrent: ${err}`;
    }
  }

  async function removeTorrent(handleId, deleteFiles = false) {
    try {
      await invoke("remove_torrent", { handleId, deleteFiles });
      if (selectedTorrent?.handle_id === handleId) {
        selectedTorrent = null;
        streamUrl = "";
      }
      await loadTorrents();
    } catch (err) {
      error = `Failed to remove torrent: ${err}`;
    }
  }

  function formatBytes(bytes) {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + " " + sizes[i];
  }

  function formatSpeed(mbps) {
    return `${mbps.toFixed(2)} MB/s`;
  }

  function formatTime(seconds) {
    if (!seconds || isNaN(seconds)) return "0:00";
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }
    return `${m}:${s.toString().padStart(2, "0")}`;
  }
</script>

<div class="torrent-debug">
  <div class="debug-header">
    <h1>Torrent Streaming Debug</h1>
    <p class="download-path">Download Directory: <code>{downloadDir}</code></p>
  </div>

  {#if error}
    <div class="error-banner">
      {error}
      <button on:click={() => (error = "")}>✕</button>
    </div>
  {/if}

  <div class="add-torrent-section">
    <h2>Add Torrent</h2>
    <div class="input-group">
      <input
        type="text"
        bind:value={magnetLink}
        placeholder="Paste magnet link or torrent URL"
        on:keydown={(e) => e.key === "Enter" && addTorrent()}
      />
      <button on:click={addTorrent} disabled={loading}>
        {loading ? "Adding..." : "Add"}
      </button>
    </div>
  </div>

  <div class="torrents-section">
    <h2>Active Torrents ({torrents.length})</h2>
    {#if torrents.length === 0}
      <div class="no-torrents">
        <p>No active torrents</p>
        <p class="hint">Add a magnet link above to start</p>
      </div>
    {:else}
      <div class="torrents-list">
        {#each torrents as torrent}
          <div
            class="torrent-item"
            class:selected={selectedTorrent?.handle_id === torrent.handle_id}
          >
            <div
              class="torrent-header"
              role="button"
              tabindex="0"
              on:click={() => selectTorrent(torrent)}
              on:keydown={(e) => {
                if (e.key === "Enter" || e.key === " ") selectTorrent(torrent);
              }}
            >
              <div class="torrent-info-row">
                <div class="torrent-name">
                  {torrent.name}
                  {#if torrent.state === 'checking'}
                    <span class="status-badge checking">Checksumming</span>
                  {:else if torrent.is_paused}
                    <span class="status-badge paused">Paused</span>
                  {:else if torrent.progress < 1}
                    <span class="status-badge streaming">Buffering</span>
                  {:else if torrent.progress >= 100}
                    <span class="status-badge complete">Complete</span>
                  {:else}
                    <span class="status-badge streaming">Downloading</span>
                  {/if}
                </div>
                <div class="torrent-size">{formatBytes(torrent.size)}</div>
              </div>
              <div class="torrent-stats">
                {#if torrent.state === 'checking'}
                  <span class="status-text">Checksumming files...</span>
                {:else}
                  <span class="progress">{torrent.progress.toFixed(1)}%</span>
                  <span class="speed"
                    >↓ {formatSpeed(torrent.download_speed)}</span
                  >
                  <span class="speed">↑ {formatSpeed(torrent.upload_speed)}</span>
                  <span class="peers">{torrent.peers} peers</span>
                {/if}
              </div>
            </div>
            <div class="torrent-progress">
              <div
                class="progress-bar"
                style="width: {torrent.progress}%"
              ></div>
            </div>
            <div class="torrent-meta">
              <span class="file-count"
                >{torrent.files.length} MKV file{torrent.files.length !== 1
                  ? "s"
                  : ""}</span
              >
            </div>
            <div class="torrent-actions">
              {#if torrent.is_paused}
                <button
                  class="btn-small"
                  on:click={() => resumeTorrent(torrent.handle_id)}
                  >Resume</button
                >
              {:else}
                <button
                  class="btn-small"
                  on:click={() => pauseTorrent(torrent.handle_id)}>Pause</button
                >
              {/if}
              <button
                class="btn-small danger"
                on:click={() => removeTorrent(torrent.handle_id, false)}
                >Remove</button
              >
              <button
                class="btn-small danger"
                on:click={() => removeTorrent(torrent.handle_id, true)}
                >Delete Files</button
              >
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
  {#if selectedTorrent}
    <div class="files-section">
      <h2>MKV Files in "{selectedTorrent.name}"</h2>
      {#if selectedTorrent.files.length === 0}
        <div class="no-files">
          <p>No MKV files found in this torrent</p>
          <p class="hint">Only .mkv video files are supported</p>
        </div>
      {:else}
        <div class="files-list">
          {#each selectedTorrent.files as file}
            <div
              class="file-item"
              class:streaming={selectedFileIndex === file.index}
            >
              <div class="file-info">
                <span class="file-icon"></span>
                <div class="file-details">
                  <span class="file-name">{file.name}</span>
                  <span class="file-path">{file.path}</span>
                  <span class="file-size">{formatBytes(file.size)}</span>
                </div>
              </div>
              <button
                class="btn-stream"
                on:click={() => startStream(file.index)}
                disabled={loading}
              >
                {loading && selectedFileIndex === file.index
                  ? "Loading..."
                  : selectedFileIndex === file.index
                    ? "Streaming"
                    : "Start Stream"}
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  {#if streamUrl}
    <div class="stream-section">
      <VideoPlayer src={streamUrl} metadata={streamMetadata} />
    </div>
  {/if}
</div>

<style>
  .torrent-debug {
    padding: var(--spacing-2xl);
    max-width: 1400px;
    margin: 0 auto;
    font-family:
      "Geist Sans",
      system-ui,
      -apple-system,
      BlinkMacSystemFont,
      "Segoe UI",
      Roboto,
      Oxygen,
      Ubuntu,
      Cantarell,
      sans-serif;
    color: var(--text-primary);
    background: var(--bg-primary);
    min-height: 100vh;
  }

  .debug-header h1 {
    margin: 0 0 var(--spacing-sm) 0;
    font-size: 2rem;
    color: var(--text-primary);
  }

  .download-path {
    margin: 0;
    font-size: 0.875rem;
    color: var(--text-secondary);
  }

  .download-path code {
    background: rgba(255, 255, 255, 0.1);
    padding: 0.25rem 0.5rem;
    border-radius: var(--border-radius-sm);
    font-family: "Geist Mono Variable", monospace;
  }

  .error-banner {
    background: rgba(239, 68, 68, 0.1);
    color: #fca5a5;
    padding: var(--spacing-lg);
    border-radius: var(--border-radius-lg);
    margin: var(--spacing-lg) 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  .error-banner button {
    background: none;
    border: none;
    color: #fca5a5;
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0 0.5rem;
  }

  .add-torrent-section {
    background: var(--bg-secondary);
    padding: var(--spacing-2xl);
    border-radius: var(--border-radius-lg);
    margin: var(--spacing-2xl) 0;
    border: 1px solid rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }

  .add-torrent-section h2 {
    margin: 0 0 var(--spacing-lg) 0;
    font-size: 1.25rem;
  }

  .input-group {
    display: flex;
    gap: var(--spacing-sm);
  }

  .input-group input {
    flex: 1;
    padding: 0.75rem;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: var(--border-radius-pill);
    color: var(--text-primary);
    font-size: 0.9rem;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: var(--shadow-highlight), var(--shadow-depth);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }

  .input-group input:focus {
    outline: none;
    border-color: var(--accent-color);
    box-shadow:
      var(--shadow-highlight),
      var(--shadow-depth),
      0 0 30px color-mix(in srgb, var(--accent-color) 20%, transparent);
  }

  .input-group input::placeholder {
    color: var(--text-tertiary);
  }

  .input-group button {
    padding: 0.75rem var(--spacing-2xl);
    background: var(--accent-color);
    border: none;
    border-radius: var(--border-radius-pill);
    color: white;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: var(--shadow-highlight), var(--shadow-depth);
  }

  .input-group button:hover:not(:disabled) {
    background: var(--accent-dark);
    transform: translateY(-2px);
    box-shadow:
      var(--shadow-highlight),
      0 8px 20px rgba(0, 0, 0, 0.4);
  }

  .input-group button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .torrents-section {
    margin: var(--spacing-2xl) 0;
  }

  .torrents-section h2 {
    margin: 0 0 var(--spacing-lg) 0;
    font-size: 1.25rem;
  }

  .torrents-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
  }

  .torrent-item {
    background: var(--bg-secondary);
    border: 2px solid transparent;
    border-radius: var(--border-radius-lg);
    padding: var(--spacing-lg);
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: var(--shadow-highlight), var(--shadow-depth);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }

  .torrent-item.selected {
    border-color: var(--accent-color);
    background: color-mix(
      in srgb,
      var(--accent-color) 10%,
      var(--bg-secondary)
    );
  }

  .torrent-header {
    cursor: pointer;
    margin-bottom: var(--spacing-sm);
  }

  .torrent-info-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--spacing-sm);
  }

  .torrent-name {
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    flex: 1;
  }

  .torrent-size {
    font-size: 0.875rem;
    color: var(--text-secondary);
    font-weight: normal;
  }

  .status-badge {
    font-size: 0.75rem;
    padding: 0.25rem 0.5rem;
    border-radius: var(--border-radius-sm);
    font-weight: 600;
  }

  .status-badge.streaming {
    background: rgba(16, 185, 129, 0.2);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.4);
  }

  .status-badge.checking {
    background: rgba(59, 130, 246, 0.2);
    color: #3b82f6;
    border: 1px solid rgba(59, 130, 246, 0.4);
  }

  .status-badge.paused {
    background: rgba(245, 158, 11, 0.2);
    color: #f59e0b;
    border: 1px solid rgba(245, 158, 11, 0.4);
  }

  .status-badge.complete {
    background: color-mix(in srgb, var(--accent-color) 20%, transparent);
    color: var(--accent-light);
    border: 1px solid color-mix(in srgb, var(--accent-color) 40%, transparent);
  }

  .torrent-meta {
    font-size: 0.813rem;
    color: var(--text-secondary);
    margin: var(--spacing-sm) 0;
  }

  .file-count {
    display: inline-block;
  }

  .no-torrents {
    text-align: center;
    padding: 3rem var(--spacing-2xl);
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    border-radius: var(--border-radius-lg);
    border: 2px dashed rgba(255, 255, 255, 0.1);
  }

  .no-torrents p {
    margin: var(--spacing-sm) 0;
    font-size: 1.125rem;
  }

  .no-torrents .hint {
    font-size: 0.875rem;
    color: var(--text-tertiary);
  }

  .torrent-stats {
    display: flex;
    gap: var(--spacing-lg);
    font-size: 0.875rem;
    color: var(--text-secondary);
  }

  .status-text {
    color: var(--text-secondary);
    font-style: italic;
  }

  .torrent-progress {
    height: 4px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: var(--border-radius-sm);
    overflow: hidden;
    margin: var(--spacing-sm) 0;
  }

  .progress-bar {
    height: 100%;
    background: linear-gradient(
      90deg,
      var(--accent-color),
      var(--accent-light)
    );
    transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .torrent-actions {
    display: flex;
    gap: var(--spacing-sm);
    margin-top: var(--spacing-sm);
  }

  .btn-small {
    padding: 0.375rem 0.75rem;
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: var(--border-radius-md);
    color: var(--text-primary);
    font-size: 0.813rem;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: var(--shadow-highlight), var(--shadow-depth);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }

  .btn-small:hover {
    background: rgba(255, 255, 255, 0.2);
    transform: translateY(-2px);
  }

  .btn-small.danger {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.4);
  }

  .btn-small.danger:hover {
    background: rgba(239, 68, 68, 0.3);
  }

  .files-section {
    margin: var(--spacing-2xl) 0;
    background: var(--bg-secondary);
    padding: var(--spacing-2xl);
    border-radius: var(--border-radius-lg);
    border: 1px solid rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }

  .files-section h2 {
    margin: 0 0 var(--spacing-lg) 0;
    font-size: 1.25rem;
  }

  .files-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }

  .file-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem;
    background: var(--bg-tertiary);
    border: 1px solid transparent;
    border-radius: var(--border-radius-md);
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .file-item:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .file-item.streaming {
    border-color: #10b981;
    background: rgba(16, 185, 129, 0.1);
  }

  .file-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    flex: 1;
  }

  .file-name {
    font-size: 0.9rem;
    color: var(--text-primary);
  }

  .file-size {
    font-size: 0.813rem;
    color: var(--text-secondary);
  }

  .btn-stream {
    padding: 0.5rem var(--spacing-lg);
    background: #10b981;
    border: none;
    border-radius: var(--border-radius-md);
    color: white;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: var(--shadow-highlight), var(--shadow-depth);
  }

  .btn-stream:hover:not(:disabled) {
    background: #059669;
    transform: translateY(-2px);
  }

  .btn-stream:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .stream-section {
    margin: var(--spacing-2xl) 0;
    background: var(--bg-secondary);
    padding: var(--spacing-2xl);
    border-radius: var(--border-radius-lg);
    border: 1px solid rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }
</style>
