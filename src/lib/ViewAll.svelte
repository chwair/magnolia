<script>
import { onMount } from 'svelte';
import { getTrending, getPopularMovies, getPopularTV, getTopRatedMovies, getTopRatedTV, getNowPlaying, discoverTV, getImageUrl } from './tmdb.js';
import { myListStore } from './stores/listStore.js';

export let title = '';
export let type = 'movie';
export let category = 'popular';
export let genre = null;
export let customItems = null;
export let onClose = () => {};

let items = [];
let loading = true;
let error = null;
let page = 1;
let totalPages = 1;

$: myListItems = new Set($myListStore.map(item => `${item.id}-${item.media_type}`));

onMount(async () => {
  await loadItems();
  const handleMouseButton = (e) => {
    if (e.button === 3) { // Back button
      e.preventDefault();
      onClose();
    }
  };
  
  window.addEventListener('mouseup', handleMouseButton);
  
  return () => {
    window.removeEventListener('mouseup', handleMouseButton);
  };
});


async function loadItems() {
  loading = true;
  error = null;
  
  try {
    if (customItems) {
      items = customItems;
      loading = false;
      return;
    }

    let response;
    if (category === 'trending') {
      response = await getTrending(type === 'all' ? 'all' : type, 'day', page);
    } else if (category === 'popular') {
      if (genre) {
        response = await discoverTV(genre, page);
      } else {
        response = type === 'movie' ? await getPopularMovies(page) : await getPopularTV(page);
      }
    } else if (category === 'top_rated') {
      response = type === 'movie' ? await getTopRatedMovies(page) : await getTopRatedTV(page);
    } else if (category === 'now_playing') {
      response = await getNowPlaying(page);
    }

    if (response?.results) {
      // Ensure media_type is set for all items
      const resultsWithType = response.results.map(item => {
        if (!item.media_type) {
          item.media_type = type === 'tv' ? 'tv' : 'movie';
        }
        return item;
      });
      
      if (page === 1) {
        items = resultsWithType;
      } else {
        const existingIds = new Set(items.map(item => item.id));
        const newItems = resultsWithType.filter(item => !existingIds.has(item.id));
        items = [...items, ...newItems];
      }
      totalPages = response.total_pages || 1;
    }
  } catch (err) {
    error = err.message;
  }
  
  loading = false;
}

function loadMore() {
  if (page < totalPages && !loading) {
    page++;
    loadItems();
  }
}

function openDetail(item) {
  // Ensure media_type is set before dispatching
  if (!item.media_type) {
    item.media_type = type === 'tv' ? 'tv' : 'movie';
  }
  onClose();
  window.dispatchEvent(new CustomEvent('openMediaDetail', { detail: item }));
}

function isInMyList(item) {
  return myListItems.has(`${item.id}-${item.media_type}`);
}

function toggleMyList(event, item) {
  event.stopPropagation();
  // Ensure media_type is set before adding to list
  if (!item.media_type) {
    item.media_type = type === 'tv' ? 'tv' : 'movie';
  }
  console.log('📋 ViewAll: Toggle for:', item.title || item.name);
  myListStore.toggleItem(item);
}

function formatDate(dateString) {
  if (!dateString) return 'N/A';
  return new Date(dateString).getFullYear();
}

function getRatingColor(rating) {
  if (rating >= 9) return '#5fedd8';  // Aqua green (9-10)
  if (rating >= 8) return '#6bdb8f';  // Green (8-9)
  if (rating >= 7) return '#f5d95a';  // Yellow (7-8)
  if (rating >= 6) return '#ffa368';  // Orange (6-7)
  if (rating >= 5) return '#ff6b6b';  // Red (5-6)
  return '#d65db1';  // Purplish red (0-5)
}
</script>

<div class="view-all-overlay">
  <div class="view-all-container">
    <div class="view-all-header">
      <button class="btn-standard back-btn" on:click={onClose}>
        <i class="ri-arrow-left-line"></i>
        Back
      </button>
      <h1>{title}</h1>
      <div class="header-spacer"></div>
    </div>

    {#if loading && items.length === 0}
      <div class="loading">Loading...</div>
    {:else if error}
      <div class="error">Error: {error}</div>
    {:else}
      <div class="view-all-grid">
        {#each items as item (item.id)}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="grid-card" on:click={() => openDetail(item)}>
            {#if item.poster_path}
              <img src={getImageUrl(item.poster_path, 'w342')} alt={item.title || item.name} loading="lazy" />
            {:else}
              <div class="no-poster">
                <i class="ri-film-line"></i>
              </div>
            {/if}
            
            {#if item.vote_average > 0}
              <div class="grid-card-rating">
                <span class="rating-badge" style="background-color: {getRatingColor(item.vote_average)}">
                  {item.vote_average.toFixed(1)}
                </span>
              </div>
            {/if}
            
            <div class="grid-card-overlay">
              <div class="grid-card-info">
                <h3>{item.title || item.name}</h3>
                <div class="grid-card-meta">
                  <span class="year">{formatDate(item.release_date || item.first_air_date)}</span>
                  {#if item.vote_average > 0}
                    <span class="rating-badge" style="background-color: {getRatingColor(item.vote_average)}">
                      {item.vote_average.toFixed(1)}
                    </span>
                  {/if}
                </div>
              </div>
              
              <div class="grid-card-actions">
                <button class="action-btn" title="Play" on:click={(e) => { e.stopPropagation(); }}>
                  <i class="ri-play-fill"></i>
                </button>
                <button 
                  class="action-btn" 
                  title={myListItems.has(`${item.id}-${item.media_type}`) ? 'Remove from List' : 'Add to List'}
                  on:click={(e) => toggleMyList(e, item)}
                >
                  <i class="{myListItems.has(`${item.id}-${item.media_type}`) ? 'ri-check-line' : 'ri-add-line'}"></i>
                </button>
              </div>
            </div>
          </div>
        {/each}
      </div>

      {#if !customItems && page < totalPages}
        <div class="load-more-container">
          <button class="btn-standard load-more-btn" on:click={loadMore} disabled={loading}>
            {#if loading}
              Loading...
            {:else}
              Load More
            {/if}
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>
