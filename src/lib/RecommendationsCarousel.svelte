<script>
import { onMount } from 'svelte';
import { getMovieRecommendations, getTVRecommendations, getImageUrl } from './tmdb.js';
import { myListStore } from './stores/listStore.js';

let allRecommendations = [];
let displayedRecommendations = [];
let currentIndex = 0;
let loading = true;
let backdropColor = '#1a1a1a';
let prominentColor = '#1a1a1a';
let textColor = '#ffffff';
let isTransitioning = false;

$: myList = $myListStore;
$: myListItems = new Set(myList.map(item => `${item.id}-${item.media_type}`));
$: {
  if (currentItem) {
    const inList = myListItems.has(`${currentItem.id}-${currentItem.media_type}`);
    console.log('🎬 Recommendations: Button state for', currentItem.title || currentItem.name, ':', inList);
  }
}

$: currentItem = displayedRecommendations[currentIndex];

$: if (currentItem?.backdrop_path) {
  extractColors(currentItem.backdrop_path);
}

$: if (myList) {
  loadRecommendations();
}

onMount(() => {
  loadRecommendations();
});


async function loadRecommendations() {
  if (myList.length === 0) {
    allRecommendations = [];
    displayedRecommendations = [];
    loading = false;
    return;
  }

  loading = true;
  const recsMap = new Map();
  const myListIds = new Set(myList.map(item => `${item.id}-${item.media_type}`));

  const fetchPromises = myList.slice(0, 5).map(async (item) => {
    try {
      const isMovie = item.media_type === 'movie';
      const recommendationsResponse = isMovie 
        ? await getMovieRecommendations(item.id) 
        : await getTVRecommendations(item.id);

      const results = recommendationsResponse?.results || [];

      results.forEach(rec => {
        const key = `${rec.id}-${item.media_type}`;
        if (myListIds.has(key)) return;
        
        if (!rec.media_type) {
          rec.media_type = item.media_type;
        }
        
        if (!recsMap.has(key) || recsMap.get(key).vote_average < rec.vote_average) {
          recsMap.set(key, rec);
        }
      });
    } catch (err) {
      console.error('Error fetching recommendations:', err);
    }
  });

  await Promise.all(fetchPromises);

  allRecommendations = Array.from(recsMap.values())
    .filter(rec => rec.vote_average > 0)
    .sort((a, b) => b.vote_average - a.vote_average);

  shuffleRecommendations();
  loading = false;
}

function shuffleRecommendations() {
  const shuffled = [...allRecommendations].sort(() => Math.random() - 0.5);
  displayedRecommendations = shuffled.slice(0, 10);
  currentIndex = 0;
}

async function extractColors(backdropPath) {
  try {
    const imageUrl = getImageUrl(backdropPath, 'w300');
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
      
      textColor = '#ffffff';
    }
    
  // Derive a muted backdrop by averaging pixels within a limited brightness range
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

function navigateRecommendation(direction) {
  if (isTransitioning) return;
  isTransitioning = true;
  
  if (direction === 'next') {
    currentIndex = (currentIndex + 1) % displayedRecommendations.length;
  } else {
    currentIndex = (currentIndex - 1 + displayedRecommendations.length) % displayedRecommendations.length;
  }
  
  setTimeout(() => {
    isTransitioning = false;
  }, 500);
}

function goToIndex(index) {
  if (isTransitioning || index === currentIndex) return;
  isTransitioning = true;
  currentIndex = index;
  setTimeout(() => {
    isTransitioning = false;
  }, 500);
}

function openDetail() {
  if (currentItem) {
    window.dispatchEvent(new CustomEvent('openMediaDetail', { detail: currentItem }));
  }
}

function isInMyList(item) {
  const inList = myListItems.has(`${item.id}-${item.media_type}`);
  return inList;
}

function toggleMyList(event) {
  event.stopPropagation();
  if (currentItem) {
    console.log('🎬 Recommendations: Toggle for:', currentItem.title || currentItem.name);
    myListStore.toggleItem(currentItem);
  }
}

function formatRating(rating) {
  return rating ? rating.toFixed(1) : 'N/A';
}

function formatDate(dateStr) {
  if (!dateStr) return 'N/A';
  return new Date(dateStr).toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' });
}

function formatRuntime(minutes) {
  if (!minutes) return '';
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return `${hours}h ${mins}m`;
}

function getRatingColor(rating) {
  if (rating >= 9) return '#5fedd8';  // Aqua green (9-10)
  if (rating >= 8) return '#6bdb8f';  // Green (8-9)
  if (rating >= 7) return '#f5d95a';  // Yellow (7-8)
  if (rating >= 6) return '#ffa368';  // Orange (6-7)
  if (rating >= 5) return '#ff6b6b';  // Red (5-6)
  return '#d65db1';  // Purplish red (0-5)
}

function getGenres(item) {
  if (item.genre_ids && item.genre_ids.length > 0) {
    const genreMap = {
      28: 'Action', 12: 'Adventure', 16: 'Animation', 35: 'Comedy', 80: 'Crime',
      99: 'Documentary', 18: 'Drama', 10751: 'Family', 14: 'Fantasy', 36: 'History',
      27: 'Horror', 10402: 'Music', 9648: 'Mystery', 10749: 'Romance', 878: 'Sci-Fi',
      10770: 'TV Movie', 53: 'Thriller', 10752: 'War', 37: 'Western', 10759: 'Action & Adventure',
      10762: 'Kids', 10763: 'News', 10764: 'Reality', 10765: 'Sci-Fi & Fantasy', 10766: 'Soap',
      10767: 'Talk', 10768: 'War & Politics'
    };
    return item.genre_ids.slice(0, 3).map(id => genreMap[id] || '').filter(Boolean);
  }
  return [];
}
</script>

{#if myList.length === 0}
  <div class="recommendations-empty">
    <div class="empty-content">
      <i class="ri-heart-line"></i>
      <h2>Start Building Your Collection</h2>
      <p>Add movies and shows to My List to get personalized recommendations</p>
    </div>
  </div>
{:else if !loading && displayedRecommendations.length > 0 && currentItem}
  <div class="recommendations-featured" style="--backdrop-color: {backdropColor}; --prominent-color: {prominentColor}; --text-color: {textColor}">
    {#key currentItem.id}
      <div class="featured-backdrop">
        {#if currentItem.backdrop_path}
          <img src={getImageUrl(currentItem.backdrop_path, 'original')} alt={currentItem.title || currentItem.name} />
        {/if}
      </div>
    {/key}

    <button class="shuffle-btn-top" on:click={shuffleRecommendations} title="Shuffle">
      <i class="ri-refresh-line"></i>
    </button>
    
    <div class="featured-content">
      <div class="featured-header">
        <div class="detail-poster">
          {#if currentItem.poster_path}
            <img src={getImageUrl(currentItem.poster_path, 'w500')} alt={currentItem.title || currentItem.name} />
          {/if}
        </div>

        <div class="detail-info-wrapper">
          <div class="detail-info">
            {#key currentItem.id}
              <h1 class="detail-title">{currentItem.title || currentItem.name}</h1>
            {/key}

            <div class="detail-meta">
              <div class="rating-box" style="background-color: {getRatingColor(currentItem.vote_average)}">
                {formatRating(currentItem.vote_average)}
              </div>
              <span>{formatDate(currentItem.release_date || currentItem.first_air_date)}</span>
            </div>

            {#if getGenres(currentItem).length > 0}
              <div class="detail-genres">
                {#each getGenres(currentItem) as genre}
                  <span class="genre-tag">{genre}</span>
                {/each}
              </div>
            {/if}

            {#if currentItem.overview}
              {#key currentItem.id}
                <p class="detail-overview">{currentItem.overview}</p>
              {/key}
            {/if}

            <div class="detail-actions">
              <button class="btn-standard primary" on:click={openDetail}>
                <i class="ri-play-fill"></i>
                Play
              </button>
              <button class="btn-standard" on:click={toggleMyList}>
                <i class="{myListItems.has(`${currentItem.id}-${currentItem.media_type}`) ? 'ri-check-line' : 'ri-add-line'}"></i>
                {myListItems.has(`${currentItem.id}-${currentItem.media_type}`) ? 'In My List' : 'My List'}
              </button>
              <button class="btn-standard" on:click={openDetail} title="More Info">
                <i class="ri-information-line"></i>
              </button>
            </div>
          </div>
        </div>
      </div>

      <div class="recommendations-nav">
        <button class="nav-arrow" on:click={() => navigateRecommendation('prev')} disabled={isTransitioning}>
          <i class="ri-arrow-left-s-line"></i>
        </button>
        
        <div class="recommendations-indicators">
          {#each displayedRecommendations as item, index}
            <button 
              class="indicator" 
              class:active={index === currentIndex}
              on:click={() => goToIndex(index)}
              disabled={isTransitioning}
              aria-label="Go to recommendation {index + 1}"
            >
              <span class="indicator-bar"></span>
            </button>
          {/each}
        </div>

        <button class="nav-arrow" on:click={() => navigateRecommendation('next')} disabled={isTransitioning}>
          <i class="ri-arrow-right-s-line"></i>
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
.recommendations-empty {
  width: 100%;
  min-height: 400px;
  max-height: 400px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(255, 255, 255, 0.02);
  border-radius: var(--border-radius-lg);
  margin: var(--spacing-lg) 0;
}

.empty-content {
  text-align: center;
  padding: var(--spacing-2xl);
}

.empty-content i {
  font-size: 4rem;
  color: var(--color-text-secondary);
  margin-bottom: var(--spacing-lg);
  opacity: 0.5;
}

.empty-content h2 {
  font-size: 1.5rem;
  margin-bottom: var(--spacing-sm);
  color: var(--color-text-primary);
}

.empty-content p {
  font-size: 1rem;
  color: var(--color-text-secondary);
  max-width: 400px;
}

.recommendations-featured {
  position: relative;
  width: 100%;
  min-height: clamp(500px, 600px, 600px);
  max-height: clamp(500px, 600px, 600px);
  overflow: hidden;
  border-radius: var(--border-radius-lg);
  margin: var(--spacing-lg) 0;
  background: linear-gradient(to bottom, var(--backdrop-color), #000);
}

.shuffle-btn-top {
  position: absolute;
  top: var(--spacing-lg);
  right: var(--spacing-lg);
  z-index: 10;
  background: rgba(0, 0, 0, 0.5);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: white;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s ease;
  backdrop-filter: blur(10px);
}

.shuffle-btn-top:hover {
  background: rgba(0, 0, 0, 0.7);
  border-color: var(--prominent-color);
  transform: scale(1.1) rotate(180deg);
}

.shuffle-btn-top i {
  font-size: 1.25rem;
}

.featured-backdrop {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  animation: fadeIn 0.5s ease-out;
}

.featured-backdrop::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: linear-gradient(
    to bottom,
    rgba(0, 0, 0, 0.2) 0%,
    rgba(0, 0, 0, 0.6) 50%,
    rgba(0, 0, 0, 0.95) 100%
  );
}

.featured-backdrop img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  animation: scaleIn 0.5s ease-out;
}

.featured-content {
  position: relative;
  z-index: 1;
  padding: 0 var(--spacing-2xl) 0 var(--spacing-2xl);
  display: flex;
  flex-direction: column;
  min-height: clamp(500px, 600px, 600px);
  max-height: clamp(500px, 600px, 600px);
  height: 100%;
}

.featured-header {
  display: flex;
  gap: var(--spacing-xl);
  align-items: center;
  justify-content: center;
  text-align: left;
  max-width: 1080px;
  width: 100%;
  margin: auto;
  flex: 1;
}

.detail-poster {
  flex-shrink: 0;
  width: 240px;
  height: 360px;
  border-radius: var(--border-radius-md);
  overflow: hidden;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
  animation: fadeIn 0.5s ease-out;
}

.detail-poster img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.detail-info-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: flex-start;
  max-width: 720px;
  width: 100%;
}

.detail-info {
  animation: fadeIn 0.5s ease-out;
  width: 100%;
}

.detail-title {
  font-size: 2.5rem;
  font-weight: 700;
  margin-bottom: var(--spacing-sm);
  line-height: 1.1;
  text-shadow: 2px 2px 8px rgba(0, 0, 0, 0.8);
  color: var(--text-color);
  animation: fadeIn 0.5s ease-out;
}

.detail-meta {
  display: flex;
  gap: var(--spacing-md);
  align-items: center;
  justify-content: flex-start;
  font-size: 14px;
  color: var(--text-color);
  flex-wrap: wrap;
  margin-bottom: var(--spacing-sm);
  animation: fadeIn 0.5s ease-out;
}

.rating-box {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 45px;
  padding: 6px 12px;
  border-radius: var(--border-radius-md);
  font-size: 16px;
  font-weight: 700;
  color: #000;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.6);
  line-height: 1;
}

.detail-genres {
  display: flex;
  gap: var(--spacing-sm);
  flex-wrap: wrap;
  justify-content: flex-start;
  margin-bottom: var(--spacing-sm);
  animation: fadeIn 0.5s ease-out;
}

.genre-tag {
  padding: 6px 14px;
  background: rgba(0, 0, 0, 0.4);
  border: 1px solid color-mix(in srgb, var(--prominent-color) 40%, rgba(255, 255, 255, 0.2));
  border-radius: var(--border-radius-pill);
  font-size: 12px;
  color: var(--text-color);
  opacity: 0.9;
  transition: all 0.2s ease;
  backdrop-filter: blur(10px);
}

.genre-tag:hover {
  background: rgba(0, 0, 0, 0.6);
  opacity: 1;
  border-color: var(--prominent-color);
}

.detail-overview {
  font-size: 15px;
  line-height: 1.6;
  color: var(--text-color);
  opacity: 0.95;
  text-shadow: 1px 1px 4px rgba(0, 0, 0, 0.8);
  margin-bottom: var(--spacing-sm);
  animation: fadeIn 0.5s ease-out;
  display: -webkit-box;
  -webkit-line-clamp: 6;
  line-clamp: 6;
  -webkit-box-orient: vertical;
  overflow: hidden;
  max-width: 80ch;
}

.detail-actions {
  display: flex;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-sm);
  animation: fadeIn 0.5s ease-out;
}

.btn-standard {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  font-size: 14px;
  padding: 10px 20px;
  height: 44px;
  transition: all 0.2s ease;
}

.btn-standard i {
  font-size: 18px;
}

.btn-standard:last-child {
  padding: 10px;
  min-width: 44px;
}

.btn-standard.primary {
  background: rgba(255, 255, 255, 0.25);
  border-color: rgba(255, 255, 255, 0.4);
}

.btn-standard.primary:hover {
  background: rgba(255, 255, 255, 0.35);
  border-color: rgba(255, 255, 255, 0.6);
  transform: scale(1.05);
}

.btn-standard:not(.primary):hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: var(--prominent-color);
  transform: scale(1.05);
}

.recommendations-nav {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-lg);
  padding: var(--spacing-lg) var(--spacing-2xl);
  margin: 0 calc(-1 * var(--spacing-2xl));
  width: calc(100% + 2 * var(--spacing-2xl));
  background: linear-gradient(
    to top, 
    color-mix(in srgb, var(--prominent-color) 15%, rgba(0, 0, 0, 0.95)) 0%, 
    color-mix(in srgb, var(--prominent-color) 10%, rgba(0, 0, 0, 0.8)) 60%, 
    transparent 100%
  );
  animation: fadeInUp 0.5s ease-out;
  flex-shrink: 0;
}

.recommendations-nav > * {
  z-index: 1;
}

.recommendations-indicators {
  max-width: 800px;
  flex: 1;
}

.nav-arrow {
  background: rgba(255, 255, 255, 0.15);
  border: 1px solid rgba(255, 255, 255, 0.3);
  color: white;
  width: 48px;
  height: 48px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s ease;
  backdrop-filter: blur(10px);
  flex-shrink: 0;
}

.nav-arrow:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.25);
  border-color: var(--prominent-color);
  transform: scale(1.1);
}

.nav-arrow:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.nav-arrow i {
  font-size: 1.5rem;
}

.recommendations-indicators {
  display: flex;
  gap: var(--spacing-sm);
  margin: 0 var(--spacing-lg);
}

.indicator {
  flex: 1;
  height: 4px;
  background: none;
  border: none;
  padding: 0;
  cursor: pointer;
  position: relative;
  transition: all 0.3s ease;
}

.indicator:disabled {
  cursor: not-allowed;
}

.indicator-bar {
  display: block;
  width: 100%;
  height: 100%;
  background: rgba(255, 255, 255, 0.25);
  border-radius: 2px;
  transition: all 0.3s ease;
  transform-origin: left;
}

.indicator:hover:not(:disabled) .indicator-bar {
  background: rgba(255, 255, 255, 0.5);
  transform: scaleY(1.5);
}

.indicator.active .indicator-bar {
  background: var(--prominent-color);
  box-shadow: 0 0 8px color-mix(in srgb, var(--prominent-color) 60%, transparent);
  transform: scaleY(1.5);
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes scaleIn {
  from {
    transform: scale(1.1);
  }
  to {
    transform: scale(1);
  }
}

@keyframes slideInLeft {
  from {
    opacity: 0;
    transform: translateX(-50px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@keyframes slideInRight {
  from {
    opacity: 0;
    transform: translateX(50px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@keyframes fadeInUp {
  from {
    opacity: 0;
    transform: translateY(30px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (max-width: 1024px) {
  .detail-poster {
    width: 200px;
    height: 300px;
  }

  .detail-title {
    font-size: 2rem;
  }

  .detail-overview {
    font-size: 14px;
    max-width: 500px;
  }

  .recommendations-featured {
    min-height: clamp(350px, 45vh, 500px);
    max-height: clamp(350px, 45vh, 500px);
  }

  .featured-content {
    min-height: clamp(350px, 45vh, 500px);
    max-height: clamp(350px, 55vh, 500px);
  }
}

@media (max-height: 700px) {
  .recommendations-featured {
    min-height: 400px;
    max-height: 400px;
  }

  .featured-content {
    min-height: 400px;
    max-height: 400px;
    padding: var(--spacing-md) var(--spacing-xl) var(--spacing-md);
  }

  .featured-header {
    padding-bottom: var(--spacing-sm);
    margin-bottom: 0;
  }

  .detail-poster {
    width: 160px;
    height: 240px;
  }

  .detail-title {
    font-size: 1.75rem;
    margin-bottom: var(--spacing-xs);
  }

  .detail-meta,
  .detail-genres,
  .detail-overview,
  .detail-actions {
    margin-bottom: var(--spacing-xs);
  }

  .detail-overview {
    font-size: 13px;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .btn-standard {
    padding: 8px 16px;
    height: 36px;
    font-size: 13px;
  }

  .recommendations-nav {
    padding: var(--spacing-sm) var(--spacing-xl);
  }
}

</style>
