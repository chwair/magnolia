<script>
  import { searchMulti, getImageUrl } from './tmdb.js';
  import { onMount } from 'svelte';
  
  export let searchActive = false;
  let searchQuery = '';
  let searchResults = [];
  let searching = false;
  let searchTimeout;
  let searchWrapper;
  let searchInput;
  let selectedIndex = -1;
  let recentSearches = [];
  let showRecent = false;
  let searchResultsContainer;
  
  $: searchActive = searchQuery.length > 0 || showRecent;
  
  $: if (selectedIndex >= 0 && searchResultsContainer) {
    scrollToSelected();
  }
  
  function scrollToSelected() {
    const items = searchResultsContainer?.querySelectorAll('.search-result-item');
    if (items && items[selectedIndex]) {
      const item = items[selectedIndex];
      const container = searchResultsContainer;
      const itemTop = item.offsetTop;
      const itemBottom = itemTop + item.offsetHeight;
      const containerTop = container.scrollTop;
      const containerBottom = containerTop + container.clientHeight;
      
      if (itemTop < containerTop) {
        container.scrollTop = itemTop;
      } else if (itemBottom > containerBottom) {
        container.scrollTop = itemBottom - container.clientHeight;
      }
    }
  }
  
  onMount(() => {
    const saved = localStorage.getItem('recentSearches');
    if (saved) {
      recentSearches = JSON.parse(saved);
    }

    const handleKeyDown = (e) => {
      if ((e.ctrlKey || e.metaKey) && (e.key === 'k' || e.key === 'K')) {
        e.preventDefault();
        searchInput?.focus();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  });
  
  async function performSearch() {
    if (searchQuery.length < 2) {
      searchResults = [];
      selectedIndex = -1;
      return;
    }
    
    searching = true;
    try {
      const response = await searchMulti(searchQuery);
      searchResults = response.results
        .filter(item => item.media_type !== 'person')
        .sort((a, b) => (b.popularity || 0) - (a.popularity || 0))
        .slice(0, 10);
      selectedIndex = -1;
    } catch (err) {
      console.error('Search error:', err);
      searchResults = [];
    }
    searching = false;
  }
  
  function handleInput() {
    showRecent = false;
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(performSearch, 150);
  }

  function handleFocus() {
    if (searchQuery.length === 0 && recentSearches.length > 0) {
      showRecent = true;
    } else if (searchQuery.length >= 2) {
      performSearch();
    }
  }

  function handleKeyDown(e) {
    const itemCount = searchResults.length || recentSearches.length;
    
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, itemCount - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, -1);
    } else if (e.key === 'Enter' && selectedIndex >= 0) {
      e.preventDefault();
      const item = showRecent ? recentSearches[selectedIndex] : searchResults[selectedIndex];
      openDetail(item);
    } else if (e.key === 'Escape') {
      searchActive = false;
      showRecent = false;
      searchInput?.blur();
    }
  }
  
  function handleClickOutside(event) {
    if (searchWrapper && !searchWrapper.contains(event.target)) {
      searchActive = false;
      showRecent = false;
    }
  }
  
  function getMediaType(item) {
    if (item.media_type === 'movie') return 'Movie';
    if (item.media_type === 'tv') return 'TV Show';
    return 'Unknown';
  }
  
  function formatRating(rating) {
    return rating ? rating.toFixed(1) : 'N/A';
  }
  
  function getRatingColor(rating) {
    if (rating >= 9) return '#00ff00';
    if (rating >= 8) return '#8bc34a';
    if (rating >= 7) return '#ffc107';
    if (rating >= 6) return '#ff9800';
    if (rating >= 5) return '#ff5722';
    return '#f44336';
  }

  function openDetail(item) {
    const exists = recentSearches.findIndex(s => s.id === item.id && s.media_type === item.media_type);
    if (exists >= 0) {
      recentSearches.splice(exists, 1);
    }
    recentSearches.unshift(item);
    recentSearches = recentSearches.slice(0, 5);
    localStorage.setItem('recentSearches', JSON.stringify(recentSearches));

    window.dispatchEvent(new CustomEvent('openMediaDetail', { detail: item }));
    searchActive = false;
    showRecent = false;
    selectedIndex = -1;
  }
</script>

<svelte:window on:click={handleClickOutside} />

<div class="search-wrapper" class:expanded={searchActive} bind:this={searchWrapper}>
  <div class="search-container">
    <input 
      type="text" 
      placeholder="Search..." 
      bind:value={searchQuery}
      bind:this={searchInput}
      on:input={handleInput}
      on:focus={handleFocus}
      on:keydown={handleKeyDown}
      class="search-input"
    />
    {#if !searchActive && searchQuery.length === 0}
      <div class="search-shortcut">
        <kbd>Ctrl</kbd>+<kbd>K</kbd>
      </div>
    {/if}
  </div>
  {#if searchActive}
    <div class="search-results" bind:this={searchResultsContainer}>
      {#if showRecent && recentSearches.length > 0}
        <div class="results-header">Recent Searches</div>
        {#each recentSearches as result, index}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div 
            class="search-result-item" 
            class:selected={selectedIndex === index}
            on:click={() => openDetail(result)}
          >
            {#if result.poster_path || result.profile_path}
              <img src={getImageUrl(result.poster_path || result.profile_path, 'w92')} alt={result.title || result.name} />
            {:else}
              <div class="no-image">?</div>
            {/if}
            <div class="result-info">
              <div class="result-title">{result.title || result.name}</div>
              <div class="result-meta">
                <span>{getMediaType(result)}</span>
                {#if result.release_date || result.first_air_date}
                  <span class="separator">•</span>
                  <span>{(result.release_date || result.first_air_date).split('-')[0]}</span>
                {/if}
                {#if result.vote_average}
                  <span class="rating-badge" style="background-color: {getRatingColor(result.vote_average)}">
                    {formatRating(result.vote_average)}
                  </span>
                {/if}
              </div>
            </div>
          </div>
        {/each}
      {:else if searching}
        <div class="results-placeholder">Searching...</div>
      {:else if searchResults.length > 0}
        {#each searchResults as result, index}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div 
            class="search-result-item" 
            class:selected={selectedIndex === index}
            on:click={() => openDetail(result)}
          >
            {#if result.poster_path || result.profile_path}
              <img src={getImageUrl(result.poster_path || result.profile_path, 'w92')} alt={result.title || result.name} />
            {:else}
              <div class="no-image">?</div>
            {/if}
            <div class="result-info">
              <div class="result-title">{result.title || result.name}</div>
              <div class="result-meta">
                <span>{getMediaType(result)}</span>
                {#if result.release_date || result.first_air_date}
                  <span class="separator">•</span>
                  <span>{(result.release_date || result.first_air_date).split('-')[0]}</span>
                {/if}
                {#if result.vote_average}
                  <span class="rating-badge" style="background-color: {getRatingColor(result.vote_average)}">
                    {formatRating(result.vote_average)}
                  </span>
                {/if}
              </div>
            </div>
          </div>
        {/each}
      {:else if searchQuery.length >= 2}
        <div class="results-placeholder">No results found</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .search-wrapper {
    flex: 1;
    max-width: 700px;
    position: relative;
  }

  .search-wrapper.expanded {
    max-width: 700px;
  }

  .search-container {
    position: relative;
  }

  .search-input {
    width: 100%;
    padding: 8px 16px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 24px;
    color: rgba(255, 255, 255, 0.87);
    font-size: 12px;
    font-family: inherit;
    outline: none;
    transition: all 0.3s ease;
    box-shadow: 
      inset -2px -2px 4px rgba(255, 255, 255, 0.02),
      inset 2px 2px 4px rgba(0, 0, 0, 0.3),
      -2px -2px 4px rgba(255, 255, 255, 0.02),
      2px 2px 4px rgba(0, 0, 0, 0.4);
  }

  .search-wrapper.expanded .search-input {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.1);
    border-radius: 16px 16px 0 0;
    box-shadow: 
      inset -2px -2px 4px rgba(255, 255, 255, 0.03),
      inset 2px 2px 4px rgba(0, 0, 0, 0.4),
      -4px -4px 12px rgba(255, 255, 255, 0.03),
      4px 4px 16px rgba(0, 0, 0, 0.6);
  }

  .search-input::placeholder {
    color: rgba(255, 255, 255, 0.3);
  }

  .search-input:focus {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
    box-shadow: 
      inset -2px -2px 4px rgba(255, 255, 255, 0.03),
      inset 2px 2px 4px rgba(0, 0, 0, 0.4),
      -3px -3px 8px rgba(255, 255, 255, 0.03),
      3px 3px 8px rgba(0, 0, 0, 0.5);
  }

  .search-results {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: rgba(10, 10, 10, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-top: none;
    border-radius: 0 0 16px 16px;
    max-height: 400px;
    overflow-y: auto;
    backdrop-filter: blur(20px);
    box-shadow: 
      -4px 4px 12px rgba(255, 255, 255, 0.02),
      4px 4px 20px rgba(0, 0, 0, 0.6);
    animation: slideDown 0.3s ease;
    z-index: 1001;
  }

  @keyframes slideDown {
    from {
      opacity: 0;
      transform: translateY(-10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .results-placeholder {
    color: rgba(255, 255, 255, 0.4);
    font-size: 14px;
    text-align: center;
    margin: 0;
  }

  .search-results::-webkit-scrollbar {
    width: 8px;
  }

  .search-results::-webkit-scrollbar-track {
    background: rgba(255, 255, 255, 0.02);
    border-radius: 4px;
  }

  .search-results::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
  }

  .search-results::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.15);
  }
</style>
