<script>
import { onMount, afterUpdate } from 'svelte';
import { getTrending, getPopularMovies, getPopularTV, getTopRatedMovies, getTopRatedTV, getNowPlaying, discoverTV, getImageUrl } from './tmdb.js';
import { myListStore } from './stores/listStore.js';

export let title = 'Section Title';
export let type = 'movie';
export let category = 'popular';
export let genre = null;
export let accentColor = '#6366f1';
export let customItems = null;

let items = [];
let loading = true;
let error = null;
let carouselElement;
let showLeftArrow = false;
let showRightArrow = false;
let cardColors = {};

$: myListItems = new Set($myListStore.map(item => `${item.id}-${item.media_type}`));
$: {
  if (title === "My List") console.log('📺 My List carousel updated:', myListItems.size, 'items in store');
}
$: {
  if (myListItems && items.length > 0) {
    console.log(`🔄 ${title}: myListItems updated, size:`, myListItems.size);
  }
}

$: if (customItems) {
  items = customItems;
  loading = false;
  console.log('🔄 Custom items updated:', customItems.length, 'items');
  customItems.forEach(item => {
    if (item.poster_path) {
      extractDominantColor(item.id, getImageUrl(item.poster_path, 'w92'));
    }
  });
}

onMount(async () => {
  if (customItems) {
    return;
  }

  try {
    let response;
    if (category === 'trending') {
      response = await getTrending(type === 'all' ? 'all' : type, 'day');
    } else if (category === 'popular') {
      if (genre) {
        response = await discoverTV({ with_genres: genre, sort_by: 'popularity.desc' });
      } else if (type === 'movie') {
        response = await getPopularMovies();
      } else if (type === 'tv') {
        response = await getPopularTV();
      }
    } else if (category === 'top_rated') {
      if (genre) {
        response = await discoverTV({ with_genres: genre, sort_by: 'vote_average.desc', 'vote_count.gte': 100 });
      } else if (type === 'movie') {
        response = await getTopRatedMovies();
      } else if (type === 'tv') {
        response = await getTopRatedTV();
      }
    } else if (category === 'now_playing') {
      response = await getNowPlaying();
    }
    items = response?.results || [];
    loading = false;

    items.forEach(item => {
      if (item.poster_path) {
        extractDominantColor(item.id, getImageUrl(item.poster_path, 'w92'));
      }
    });
  } catch (err) {
    console.error('Error fetching TMDB data:', err);
    error = err.message;
    loading = false;
  }
});

afterUpdate(() => {
updateArrows();
});

function formatRating(rating) {
return rating ? rating.toFixed(1) : 'N/A';
}

function formatDate(dateStr) {
if (!dateStr) return 'N/A';
const date = new Date(dateStr);
return date.getFullYear();
}

function getRatingColor(rating) {
if (rating >= 9) return '#5fedd8';  // Aqua green (9-10)
if (rating >= 8) return '#6bdb8f';  // Green (8-9)
if (rating >= 7) return '#f5d95a';  // Yellow (7-8)
if (rating >= 6) return '#ffa368';  // Orange (6-7)
if (rating >= 5) return '#ff6b6b';  // Red (5-6)
return '#d65db1';  // Purplish red (0-5)
}

function updateArrows() {
if (!carouselElement) return;
const scrollLeft = carouselElement.scrollLeft;
const scrollWidth = carouselElement.scrollWidth;
const clientWidth = carouselElement.clientWidth;
showLeftArrow = scrollLeft > 10;
showRightArrow = scrollLeft < scrollWidth - clientWidth - 10;
}

function scroll(direction) {
if (!carouselElement) return;
const scrollAmount = 400;
carouselElement.scrollBy({
left: direction === 'left' ? -scrollAmount : scrollAmount,
behavior: 'smooth'
});
setTimeout(updateArrows, 300);
}

async function extractDominantColor(itemId, imageUrl) {
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
let r = 0, g = 0, b = 0, count = 0;

// Sample pixels and calculate average to avoid very dark or light swatches
for (let i = 0; i < imageData.length; i += 4 * 4) {
const red = imageData[i];
const green = imageData[i + 1];
const blue = imageData[i + 2];
const brightness = (red + green + blue) / 3;

if (brightness > 40 && brightness < 180) {
r += red;
g += green;
b += blue;
count++;
}
}

if (count > 0) {
r = Math.floor(r / count);
g = Math.floor(g / count);
b = Math.floor(b / count);
const max = Math.max(r, g, b);
const min = Math.min(r, g, b);
const saturation = max === 0 ? 0 : (max - min) / max;
const boost = 1.5; // Increase saturation
r = Math.min(255, Math.floor(r + (r - min) * boost * saturation));
g = Math.min(255, Math.floor(g + (g - min) * boost * saturation));
b = Math.min(255, Math.floor(b + (b - min) * boost * saturation));
cardColors[itemId] = `rgb(${r}, ${g}, ${b})`;
} else {
cardColors[itemId] = accentColor;
}
cardColors = cardColors;
} catch (err) {
cardColors[itemId] = accentColor;
cardColors = cardColors;
}
}

function openDetail(item) {
  window.dispatchEvent(new CustomEvent('openMediaDetail', { detail: item }));
}

function isInMyList(item) {
  const inList = myListItems.has(`${item.id}-${item.media_type}`);
  return inList;
}

function toggleMyList(event, item) {
  event.stopPropagation();
  console.log('🔘 Toggle button clicked for:', item.title || item.name);
  myListStore.toggleItem(item);
}function handleViewAll() {
  const detail = {
    title,
    type,
    category,
    genre,
    customItems
  };
  window.dispatchEvent(new CustomEvent('viewAll', { detail }));
}
</script>

<div class="carousel-section">
<div class="section-header">
<h2 class="section-title">{title}</h2>
<button class="btn-standard view-all" on:click={handleViewAll}>View All →</button>
</div>
{#if loading}
<div class="loading">Loading...</div>
{:else if error}
<div class="error">Error: {error}</div>
{:else}
    <div class="carousel-container" class:show-left-gradient={showLeftArrow} class:show-right-gradient={showRightArrow}>
      <button class="carousel-arrow left" class:visible={showLeftArrow} on:click={() => scroll('left')}>
        <i class="ri-arrow-left-s-line"></i>
      </button>
      <div class="carousel" bind:this={carouselElement} on:scroll={updateArrows}>
{#each items as item (item.id)}
<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="media-card" style="--card-accent: {cardColors[item.id] || accentColor}" on:click={() => openDetail(item)}>
{#if item.poster_path}
<img class="media-poster" src={getImageUrl(item.poster_path, 'w500')} alt={item.title || item.name} loading="lazy" />
{/if}
<div class="media-content">
<div class="media-info">
<h3 class="media-title">{item.title || item.name}</h3>
<div class="media-meta">
<span>{formatDate(item.release_date || item.first_air_date)}</span>
<span class="rating-badge" style="background-color: {getRatingColor(item.vote_average)}">
{formatRating(item.vote_average)}
</span>
</div>
</div>
<div class="media-actions">
<button class="action-btn" title="Play" on:click={(e) => e.stopPropagation()}>
<i class="ri-play-fill"></i>
</button>
<button 
  class="action-btn" 
  title={myListItems.has(`${item.id}-${item.media_type}`) ? "Remove from List" : "Add to List"}
  on:click={(e) => toggleMyList(e, item)}
>
<i class="{myListItems.has(`${item.id}-${item.media_type}`) ? 'ri-check-line' : 'ri-add-line'}"></i>
</button>
</div>
</div>
</div>
{/each}
</div>
<button class="carousel-arrow right" class:visible={showRightArrow} on:click={() => scroll('right')}>
<i class="ri-arrow-right-s-line"></i>
</button>
</div>
{/if}
</div>
