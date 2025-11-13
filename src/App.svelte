<script>
  import { onMount } from 'svelte';
  import TitleBar from './lib/TitleBar.svelte';
  import MediaCarousel from './lib/MediaCarousel.svelte';
  import MediaDetail from './lib/MediaDetail.svelte';
  import ViewAll from './lib/ViewAll.svelte';
  import RecommendationsCarousel from './lib/RecommendationsCarousel.svelte';
  import TorrentDebug from './lib/TorrentDebug.svelte';
  import { myListStore } from './lib/stores/listStore.js';

  let searchActive = false;
  let selectedMedia = null;
  let viewAllData = null;
  let titleBarAccentColor = null;
  let mediaHistory = [];
  let historyIndex = -1;
  let showTorrentDebug = false;
  
  $: myList = $myListStore;

  onMount(() => {

    window.addEventListener('openMediaDetail', (e) => {
      openMedia(e.detail);
    });

    window.addEventListener('updateTitleBarColor', (e) => {
      titleBarAccentColor = e.detail.color;
    });

    window.addEventListener('viewAll', (e) => {
      viewAllData = e.detail;
    });

    window.addEventListener('mouseup', (e) => {
      if (e.button === 3) { // Back button
        e.preventDefault();
        navigateBack();
      } else if (e.button === 4) { // Forward button
        e.preventDefault();
        navigateForward();
      }
    });

    const handleKeyDown = (e) => {
      if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;

      switch(e.key) {
        case 'Escape':
          if (viewAllData) {
            e.preventDefault();
            viewAllData = null;
          } else if (selectedMedia) {
            e.preventDefault();
            closeDetail();
          } else if (showTorrentDebug) {
            e.preventDefault();
            showTorrentDebug = false;
          }
          break;
        case 'ArrowLeft':
          if (selectedMedia && (e.altKey || e.metaKey)) {
            e.preventDefault();
            navigateBack();
          }
          break;
        case 'ArrowRight':
          if (selectedMedia && (e.altKey || e.metaKey)) {
            e.preventDefault();
            navigateForward();
          }
          break;
        case 'Home':
          if (!selectedMedia) {
            e.preventDefault();
            window.scrollTo({ top: 0, behavior: 'smooth' });
          }
          break;
        case 'End':
          if (!selectedMedia) {
            e.preventDefault();
            window.scrollTo({ top: document.body.scrollHeight, behavior: 'smooth' });
          }
          break;
        case 'D':
          // Press Shift+D to toggle debug interface
          if (e.shiftKey && !selectedMedia && !viewAllData) {
            e.preventDefault();
            showTorrentDebug = !showTorrentDebug;
          }
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  });

  function openMedia(media) {
    if (selectedMedia?.id !== media.id) {
      historyIndex++;
      mediaHistory = [...mediaHistory.slice(0, historyIndex), media];
    }
    selectedMedia = media;
  }

  function navigateBack() {
    if (historyIndex > 0) {
      historyIndex--;
      selectedMedia = mediaHistory[historyIndex];
    } else if (historyIndex === 0) {
      historyIndex = -1;
      selectedMedia = null;
      titleBarAccentColor = null;
    }
  }

  function navigateForward() {
    if (historyIndex < mediaHistory.length - 1) {
      historyIndex++;
      selectedMedia = mediaHistory[historyIndex];
    }
  }

  function closeDetail() {
    selectedMedia = null;
    titleBarAccentColor = null;
    historyIndex = -1;
    mediaHistory = [];
  }


</script>

<main>
  <TitleBar bind:searchActive accentColor={titleBarAccentColor} />
  
  {#if showTorrentDebug}
    <div class="torrent-debug-container">
      <TorrentDebug />
      <button class="close-debug" on:click={() => showTorrentDebug = false}>
        ✕ Close Debug (Shift+D)
      </button>
    </div>
  {:else}
    <div class="app-content" class:blur-overlay={searchActive}>
      <div class="content">
        <RecommendationsCarousel />
        <MediaCarousel title="Trending Now" type="all" category="trending" />
        {#if myList.length > 0}
          <MediaCarousel title="My List" type="custom" customItems={myList} />
        {/if}
        <MediaCarousel title="Popular Movies" type="movie" category="popular" />
        <MediaCarousel title="Popular TV Shows" type="tv" category="popular" />
        <MediaCarousel title="Top Rated Movies" type="movie" category="top_rated" />
        <MediaCarousel title="Top Rated TV Shows" type="tv" category="top_rated" />
        <MediaCarousel title="Now Playing" type="movie" category="now_playing" />
      </div>

      {#if selectedMedia}
        <MediaDetail media={selectedMedia} onClose={closeDetail} />
      {/if}

      {#if viewAllData}
        <ViewAll 
          title={viewAllData.title}
          type={viewAllData.type}
          category={viewAllData.category}
          genre={viewAllData.genre}
          customItems={viewAllData.customItems}
          onClose={() => viewAllData = null}
        />
      {/if}
    </div>
  {/if}
</main>

<style>
  :global(html) {
    background: transparent;
    overflow: hidden;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
  }

  main {
    width: 100vw;
    height: 100vh;
    background: #0a0a0a;
    border-radius: 8px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .app-content {
    flex: 1;
    position: relative;
    overflow: hidden;
  }

  .content {
    width: 100%;
    height: 100%;
    overflow-y: auto;
    padding-top: var(--titlebar-height);
  }

  .torrent-debug-container {
    flex: 1;
    overflow-y: auto;
    padding-top: var(--titlebar-height);
    position: relative;
  }

  .close-debug {
    position: fixed;
    top: calc(var(--titlebar-height) + 1rem);
    right: 1rem;
    padding: 0.75rem 1.5rem;
    background: rgba(0, 0, 0, 0.8);
    border: 2px solid #6366f1;
    border-radius: 8px;
    color: white;
    font-weight: 600;
    cursor: pointer;
    backdrop-filter: blur(10px);
    z-index: 1000;
    transition: all 0.2s;
  }

  .close-debug:hover {
    background: #6366f1;
    transform: scale(1.05);
  }
</style>
