<script>
import { onMount, onDestroy } from 'svelte';
import { getMovieDetails, getTVDetails, getSeasonDetails, getMovieCredits, getTVCredits, getMovieRecommendations, getTVRecommendations, getImageUrl } from './tmdb.js';
import { myListStore } from './stores/listStore.js';

export let media = null;
export let onClose = () => {};

let details = null;
let loading = true;
let backdropColor = '#1a1a1a';
let prominentColor = '#1a1a1a';
let textColor = '#ffffff';
let selectedSeason = null;
let seasonDetails = null;
let allSeasonsData = {};
let selectedEpisode = null;
let credits = null;
let recommendations = [];
let activeTab = '';
let availableTabs = [];
let viewMode = 'list'; // 'list' or 'grid'

$: isInMyList = media && $myListStore.some(item => 
  item.id === media.id && item.media_type === media.media_type
);
$: {
  if (media) console.log('🎥 MediaDetail: isInMyList updated for', media.title || media.name, ':', isInMyList);
}

$: if (media) {
  loadDetails();
  selectedSeason = null;
  seasonDetails = null;
  allSeasonsData = {};
  selectedEpisode = null;
  recommendations = [];
}

$: if (details) {
  availableTabs = [];
  if (details.seasons && details.seasons.length > 0) {
    availableTabs.push('seasons');
  }
  availableTabs.push('cast');
  availableTabs.push('details');
  
  if (details.seasons && details.seasons.length > 0) {
    activeTab = 'seasons';
    // Skip specials (season 0) when selecting default
    const firstSeason = details.seasons.find(s => s.season_number > 0);
    if (firstSeason) {
      selectedSeason = firstSeason.season_number;
    }
    loadAllSeasons();
  } else {
    activeTab = 'cast';
  }
  
  loadRecommendations();
}

$: if (selectedSeason && details) {
  loadSeasonDetails();
}

onMount(() => {
  const handleKeyDown = (e) => {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
    if (e.key === 'Tab' && availableTabs.length > 0) {
      e.preventDefault();
      const currentIndex = availableTabs.indexOf(activeTab);
      if (e.shiftKey) {
        activeTab = availableTabs[(currentIndex - 1 + availableTabs.length) % availableTabs.length];
      } else {
        activeTab = availableTabs[(currentIndex + 1) % availableTabs.length];
      }
    }
    if (e.key >= '1' && e.key <= '9') {
      const index = parseInt(e.key) - 1;
      if (index < availableTabs.length) {
        e.preventDefault();
        activeTab = availableTabs[index];
      }
    }
    
    if (e.key === 'Escape' && selectedEpisode) {
      e.preventDefault();
      e.stopPropagation();
      selectedEpisode = null;
    }
  };
  
  window.addEventListener('keydown', handleKeyDown);
  
  return () => {
    window.removeEventListener('keydown', handleKeyDown);
  };
});

async function loadDetails() {
  loading = true;
  try {
    if (media.media_type === 'movie' || media.title) {
      details = await getMovieDetails(media.id);
      credits = await getMovieCredits(media.id);
    } else {
      details = await getTVDetails(media.id);
      credits = await getTVCredits(media.id);
    }
    
    if (details.backdrop_path) {
      await extractColors(getImageUrl(details.backdrop_path, 'w300'));
    }
  } catch (err) {
    console.error('Error loading details:', err);
  }
  loading = false;
}

async function loadSeasonDetails() {
  try {
    seasonDetails = await getSeasonDetails(details.id, selectedSeason);
  } catch (err) {
    console.error('Error loading season:', err);
  }
}

async function loadAllSeasons() {
  if (!details || !details.seasons) return;
  
  try {
    const seasonPromises = details.seasons
      .filter(s => s.season_number > 0)
      .map(async (season) => {
        const data = await getSeasonDetails(details.id, season.season_number);
        return { seasonNumber: season.season_number, data };
      });
    
    const results = await Promise.all(seasonPromises);
    const seasonsData = {};
    results.forEach(({ seasonNumber, data }) => {
      seasonsData[seasonNumber] = data;
    });
    allSeasonsData = seasonsData;
  } catch (err) {
    console.error('Error loading all seasons:', err);
  }
}

async function loadRecommendations() {
  try {
    let response;
    if (media.media_type === 'movie' || media.title) {
      response = await getMovieRecommendations(media.id);
    } else {
      response = await getTVRecommendations(media.id);
    }
    recommendations = response?.results?.slice(0, 10) || [];
  } catch (err) {
    console.error('Error loading recommendations:', err);
  }
}

async function extractColors(imageUrl) {
  try {
    const img = new Image();
    img.crossOrigin = 'Anonymous';
    img.src = imageUrl;
    await new Promise((resolve, reject) => {
      img.onload = resolve;
      img.onerror = reject;
    });

    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    canvas.width = img.width;
    canvas.height = img.height;
    ctx.drawImage(img, 0, 0);

    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    
  // Build a histogram of mid-range brightness pixels to find a dominant accent color
    const colorMap = {};
    for (let i = 0; i < imageData.length; i += 4 * 10) {
      const r = imageData[i];
      const g = imageData[i + 1];
      const b = imageData[i + 2];
      const brightness = (r + g + b) / 3;
      
      if (brightness > 40 && brightness < 200) {
        const colorKey = `${Math.floor(r/20)*20},${Math.floor(g/20)*20},${Math.floor(b/20)*20}`;
        colorMap[colorKey] = (colorMap[colorKey] || 0) + 1;
      }
    }
    
    let maxCount = 0;
    let dominantColor = null;
    for (const [color, count] of Object.entries(colorMap)) {
      if (count > maxCount) {
        maxCount = count;
        dominantColor = color;
      }
    }
    
    if (dominantColor) {
      const [r, g, b] = dominantColor.split(',').map(Number);
      prominentColor = `rgb(${r}, ${g}, ${b})`;
      
      const hexColor = rgbToHex(r, g, b);
      window.dispatchEvent(new CustomEvent('updateTitleBarColor', { detail: { color: hexColor } }));
      textColor = '#ffffff';
    }

    // Build a muted backdrop by averaging mid-range brightness values
    let br = 0, bg = 0, bb = 0, count = 0;
    for (let i = 0; i < imageData.length; i += 4 * 10) {
      const red = imageData[i];
      const green = imageData[i + 1];
      const blue = imageData[i + 2];
      const brightness = (red + green + blue) / 3;

      if (brightness > 30 && brightness < 180) {
        br += red;
        bg += green;
        bb += blue;
        count++;
      }
    }

    if (count > 0) {
      br = Math.floor((br / count) * 0.3);
      bg = Math.floor((bg / count) * 0.3);
      bb = Math.floor((bb / count) * 0.3);
      backdropColor = `rgb(${br}, ${bg}, ${bb})`;
    }
  } catch (err) {
    console.error('Color extraction failed:', err);
  }
}

function rgbToHex(r, g, b) {
  return '#' + [r, g, b].map(x => {
    const hex = x.toString(16);
    return hex.length === 1 ? '0' + hex : hex;
  }).join('');
}

function toggleMyList() {
  console.log('🎥 MediaDetail: Toggle for:', media.title || media.name, 'Current state:', isInMyList);
  myListStore.toggleItem(media);
}

function formatRuntime(minutes) {
  if (!minutes) return 'N/A';
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return `${hours}h ${mins}m`;
}

function formatDate(dateStr) {
  if (!dateStr) return 'N/A';
  return new Date(dateStr).toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' });
}

function formatMoney(amount) {
  if (!amount) return 'N/A';
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 0 }).format(amount);
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

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="detail-overlay">
  <div class="detail-container" style="--backdrop-color: {backdropColor}; --prominent-color: {prominentColor}; --text-color: {textColor}">
    <button class="close-btn btn-standard" on:click={onClose} title="Back">
      <i class="ri-arrow-left-line"></i>
    </button>

    {#if loading}
      <div class="loading">Loading...</div>
    {:else if details}
      <div class="detail-backdrop">
        {#if details.backdrop_path}
          <img src={getImageUrl(details.backdrop_path, 'w1280')} alt="Backdrop" />
        {/if}
      </div>

      <div class="detail-content">
        <div class="detail-header">
          <div class="detail-poster">
            {#if details.poster_path}
              <img src={getImageUrl(details.poster_path, 'w500')} alt={details.title || details.name} />
            {/if}
          </div>

          <div class="detail-info-wrapper">
            <div class="detail-info">
              <h1 class="detail-title">{details.title || details.name}</h1>
              {#if details.tagline}
                <p class="detail-tagline">"{details.tagline}"</p>
              {/if}
              
              <div class="detail-meta">
                <div class="rating-box" style="background-color: {getRatingColor(details.vote_average)}">
                  {details.vote_average.toFixed(1)}
                </div>
                {#if details.content_ratings?.results?.length || details.release_dates?.results?.length}
                  <span class="age-rating">
                    {#if details.content_ratings}
                      {details.content_ratings.results.find(r => r.iso_3166_1 === 'US')?.rating || 'NR'}
                    {:else if details.release_dates}
                      {details.release_dates.results.find(r => r.iso_3166_1 === 'US')?.release_dates?.[0]?.certification || 'NR'}
                    {/if}
                  </span>
                {/if}
                <span>{formatDate(details.release_date || details.first_air_date)}</span>
                {#if details.runtime}
                  <span>•</span>
                  <span>{formatRuntime(details.runtime)}</span>
                {/if}
                {#if details.number_of_seasons}
                  <span>•</span>
                  <span>{details.number_of_seasons} Season{details.number_of_seasons > 1 ? 's' : ''}</span>
                {/if}
              </div>

              {#if details.genres && details.genres.length > 0}
                <div class="detail-genres">
                  {#each details.genres as genre}
                    <span class="genre-tag">{genre.name}</span>
                  {/each}
                </div>
              {/if}

              <p class="detail-overview">{details.overview}</p>

              <div class="detail-actions">
                <button class="btn-standard primary">
                  <i class="ri-play-fill"></i>
                  Play
                </button>
                <button class="btn-standard" on:click={toggleMyList}>
                  <i class="{isInMyList ? 'ri-check-line' : 'ri-add-line'}"></i>
                  {isInMyList ? 'In My List' : 'My List'}
                </button>
              </div>
            </div>
          </div>
        </div>

        <div class="detail-tabs">
          {#if details.seasons && details.seasons.length > 0}
            <button class="tab-btn" class:active={activeTab === 'seasons'} on:click={() => activeTab = 'seasons'}>
              Seasons
            </button>
          {/if}
          <button class="tab-btn" class:active={activeTab === 'cast'} on:click={() => activeTab = 'cast'}>
            Cast & Crew
          </button>
          <button class="tab-btn" class:active={activeTab === 'details'} on:click={() => activeTab = 'details'}>
            Details
          </button>
        </div>

        <div class="tab-content">
          {#if activeTab === 'seasons' && details.seasons && details.seasons.length > 0}
            <div class="seasons-header">
              {#if viewMode === 'list'}
                <div class="season-selector">
                  {#each details.seasons.filter(s => s.season_number > 0) as season}
                    <button 
                      class="season-btn" 
                      class:active={selectedSeason === season.season_number}
                      on:click={() => selectedSeason = season.season_number}
                    >
                      Season {season.season_number}
                    </button>
                  {/each}
                </div>
              {/if}
              
              <div class="view-toggle">
                <button 
                  class="toggle-btn" 
                  class:active={viewMode === 'list'}
                  on:click={() => viewMode = 'list'}
                  title="List View"
                >
                  <i class="ri-list-check"></i>
                </button>
                <button 
                  class="toggle-btn" 
                  class:active={viewMode === 'grid'}
                  on:click={() => viewMode = 'grid'}
                  title="Grid View"
                >
                  <i class="ri-grid-fill"></i>
                </button>
              </div>
            </div>

            {#if seasonDetails}
              {#if viewMode === 'list'}
                <div class="episodes-grid">
                  {#each seasonDetails.episodes as episode}
                    <!-- svelte-ignore a11y-click-events-have-key-events -->
                    <!-- svelte-ignore a11y-no-static-element-interactions -->
                    <div 
                      class="episode-card" 
                      class:selected={selectedEpisode?.episode_number === episode.episode_number}
                      on:click={() => selectedEpisode = episode}
                    >
                      {#if episode.still_path}
                        <img src={getImageUrl(episode.still_path, 'w300')} alt={episode.name} />
                      {:else}
                        <div class="episode-placeholder">
                          <i class="ri-film-line"></i>
                        </div>
                      {/if}
                      <div class="episode-info">
                        <div class="episode-header">
                          <span class="episode-number">E{episode.episode_number}</span>
                          {#if episode.vote_average}
                            <span class="episode-rating" style="background-color: {getRatingColor(episode.vote_average)}">
                              {episode.vote_average.toFixed(1)}
                            </span>
                          {/if}
                        </div>
                        <h4 class="episode-title">{episode.name}</h4>
                        <p class="episode-date">{formatDate(episode.air_date)}</p>
                        {#if episode.runtime}
                          <p class="episode-runtime">{episode.runtime}min</p>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              {:else}
                <div class="episodes-heatmap">
                  <div class="heatmap-grid">
                    {#each details.seasons.filter(s => s.season_number > 0) as season}
                      <div class="heatmap-row">
                        <div class="season-label">S{season.season_number}</div>
                        <div class="episodes-row">
                          {#if allSeasonsData[season.season_number]?.episodes}
                            {#each allSeasonsData[season.season_number].episodes as episode}
                              <!-- svelte-ignore a11y-click-events-have-key-events -->
                              <!-- svelte-ignore a11y-no-static-element-interactions -->
                              <div 
                                class="heatmap-cell loaded" 
                                style="background-color: {episode.vote_average ? getRatingColor(episode.vote_average) : 'rgba(255, 255, 255, 0.08)'}"
                                on:click={() => {
                                  selectedSeason = season.season_number;
                                  selectedEpisode = episode;
                                }}
                                data-tooltip="{episode.name} - {episode.vote_average ? episode.vote_average.toFixed(1) : 'N/A'}"
                              >
                                {episode.vote_average ? episode.vote_average.toFixed(1) : '—'}
                              </div>
                            {/each}
                          {:else}
                            {#if season.episode_count}
                              {#each Array(season.episode_count) as _, episodeIndex}
                                <div class="heatmap-cell loading-cell">
                                  <div class="loading-spinner"></div>
                                </div>
                              {/each}
                            {/if}
                          {/if}
                        </div>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}

            {:else if selectedSeason}
              <div class="loading">Loading episodes...</div>
            {/if}
          {/if}

          {#if activeTab === 'cast' && credits}
            <div class="cast-grid">
              {#each credits.cast.slice(0, 20) as person}
                <div class="cast-card">
                  {#if person.profile_path}
                    <img src={getImageUrl(person.profile_path, 'w185')} alt={person.name} />
                  {:else}
                    <div class="cast-placeholder">
                      <i class="ri-user-line"></i>
                    </div>
                  {/if}
                  <div class="cast-info">
                    <h4>{person.name}</h4>
                    <p>{person.character}</p>
                  </div>
                </div>
              {/each}
            </div>
          {/if}

          {#if activeTab === 'details'}
            <div class="detail-grid">
              {#if details.status}
                <div class="detail-item">
                  <span class="label">Status</span>
                  <span class="value">{details.status}</span>
                </div>
              {/if}
              {#if details.budget}
                <div class="detail-item">
                  <span class="label">Budget</span>
                  <span class="value">{formatMoney(details.budget)}</span>
                </div>
              {/if}
              {#if details.revenue}
                <div class="detail-item">
                  <span class="label">Revenue</span>
                  <span class="value">{formatMoney(details.revenue)}</span>
                </div>
              {/if}
              {#if details.original_language}
                <div class="detail-item">
                  <span class="label">Language</span>
                  <span class="value">{details.original_language.toUpperCase()}</span>
                </div>
              {/if}
              {#if details.vote_count}
                <div class="detail-item">
                  <span class="label">Votes</span>
                  <span class="value">{details.vote_count.toLocaleString()}</span>
                </div>
              {/if}
              {#if details.production_companies && details.production_companies.length > 0}
                <div class="detail-item full-width">
                  <span class="label">Production</span>
                  <span class="value">{details.production_companies.map(c => c.name).join(', ')}</span>
                </div>
              {/if}
              {#if details.networks && details.networks.length > 0}
                <div class="detail-item full-width">
                  <span class="label">Network</span>
                  <span class="value">{details.networks.map(n => n.name).join(', ')}</span>
                </div>
              {/if}
              {#if details.created_by && details.created_by.length > 0}
                <div class="detail-item full-width">
                  <span class="label">Created By</span>
                  <span class="value">{details.created_by.map(c => c.name).join(', ')}</span>
                </div>
              {/if}
            </div>
          {/if}
        </div>

        {#if recommendations.length > 0}
          <div class="recommendations-section">
            <h2 class="section-title">You May Also Like</h2>
            <div class="recommendations-grid">
              {#each recommendations as rec}
                <!-- svelte-ignore a11y-click-events-have-key-events -->
                <!-- svelte-ignore a11y-no-static-element-interactions -->
                <div class="recommendation-card" on:click={() => window.dispatchEvent(new CustomEvent('openMediaDetail', { detail: rec }))}>
                  {#if rec.poster_path}
                    <img src={getImageUrl(rec.poster_path, 'w342')} alt={rec.title || rec.name} />
                  {:else}
                    <div class="poster-placeholder">
                      <i class="ri-film-line"></i>
                    </div>
                  {/if}
                  <div class="recommendation-info">
                    <h4>{rec.title || rec.name}</h4>
                    {#if rec.vote_average}
                      <span class="rec-rating" style="background-color: {getRatingColor(rec.vote_average)}">
                        {rec.vote_average.toFixed(1)}
                      </span>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="keyboard-shortcuts">
    <div class="shortcut-item">
      <kbd>ESC</kbd>
      <span>Close</span>
    </div>
    <div class="shortcut-item">
      <kbd>Tab</kbd>
      <span>Switch Tab</span>
    </div>
    <div class="shortcut-item">
      <kbd>1</kbd><kbd>2</kbd><kbd>3</kbd>
      <span>Go to Tab</span>
    </div>
  </div>
</div>

{#if selectedEpisode}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="episode-modal-overlay" on:click={() => selectedEpisode = null}>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="episode-modal" on:click={(e) => e.stopPropagation()}>
      <div class="episode-modal-header">
        <h3>Episode {selectedEpisode.episode_number}: {selectedEpisode.name}</h3>
        <button class="btn-standard close-modal-btn" on:click={() => selectedEpisode = null}>
          <i class="ri-close-line"></i>
        </button>
      </div>
      
      <div class="episode-modal-content">
        {#if selectedEpisode.still_path}
          <div class="episode-modal-still">
            <img src={getImageUrl(selectedEpisode.still_path, 'w780')} alt={selectedEpisode.name} />
            <div class="episode-play-overlay">
              <button class="play-episode-btn">
                <i class="ri-play-fill"></i>
              </button>
            </div>
          </div>
        {/if}

        <div class="episode-modal-meta">
          <div class="episode-modal-stat">
            <span class="stat-label">Rating</span>
            <div class="stat-value">
              {#if selectedEpisode.vote_average && selectedEpisode.vote_average > 0}
                <span class="rating-badge" style="background-color: {getRatingColor(selectedEpisode.vote_average)}">
                  {selectedEpisode.vote_average.toFixed(1)}
                </span>
                {#if selectedEpisode.vote_count}
                  <span class="vote-count">({selectedEpisode.vote_count})</span>
                {/if}
              {:else}
                <span class="no-rating">N/A</span>
              {/if}
            </div>
          </div>
          
          <div class="episode-modal-stat">
            <span class="stat-label">Air Date</span>
            <span class="stat-value">{formatDate(selectedEpisode.air_date)}</span>
          </div>
          
          {#if selectedEpisode.runtime}
            <div class="episode-modal-stat">
              <span class="stat-label">Runtime</span>
              <span class="stat-value">{selectedEpisode.runtime} min</span>
            </div>
          {/if}
        </div>

        <div class="episode-modal-overview">
          <h4>Overview</h4>
          <p>{selectedEpisode.overview || 'No overview available.'}</p>
        </div>
      </div>
    </div>
  </div>
{/if}
