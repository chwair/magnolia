import { writable } from 'svelte/store';

function loadFromLocalStorage() {
  if (typeof window !== 'undefined') {
    try {
      const savedHistory = localStorage.getItem('watchHistory');
      return savedHistory ? JSON.parse(savedHistory) : [];
    } catch (error) {
      console.error('Error loading watch history from localStorage:', error);
      return [];
    }
  }
  return [];
}

function createWatchHistoryStore() {
  const { subscribe, set, update } = writable(loadFromLocalStorage());

  return {
    subscribe,

    addItem: (item, episodeData = null) => {
      update(history => {
        // Remove if already exists
        const filtered = history.filter(media => 
          !(media.id === item.id && media.media_type === item.media_type)
        );
        
        // Add to front with timestamp and episode data
        const newHistory = [
          { 
            ...item, 
            watchedAt: Date.now(),
            ...(episodeData && {
              currentSeason: episodeData.season,
              currentEpisode: episodeData.episode,
              currentTimestamp: episodeData.timestamp || 0
            })
          },
          ...filtered
        ].slice(0, 20); // Keep only last 20 items
        
        if (typeof window !== 'undefined') {
          localStorage.setItem('watchHistory', JSON.stringify(newHistory));
        }
        
        console.log('📺 Added to watch history:', item.title || item.name, episodeData);
        return newHistory;
      });
    },

    removeItem: (mediaId, mediaType) => {
      update(history => {
        const newHistory = history.filter(media => 
          !(media.id === mediaId && media.media_type === mediaType)
        );
        
        if (typeof window !== 'undefined') {
          localStorage.setItem('watchHistory', JSON.stringify(newHistory));
        }
        
        console.log('🗑️ Removed from watch history:', mediaId);
        return newHistory;
      });
    },

    clear: () => {
      set([]);
      if (typeof window !== 'undefined') {
        localStorage.removeItem('watchHistory');
      }
      console.log('🗑️ Watch history cleared');
    },
    
    reload: () => {
      set(loadFromLocalStorage());
    }
  };
}

export const watchHistoryStore = createWatchHistoryStore();

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (e.key === 'watchHistory') {
      watchHistoryStore.reload();
    }
  });
}

// Tracker preference utilities
export function getTrackerPreference() {
  if (typeof window !== 'undefined') {
    try {
      const stored = localStorage.getItem('trackerPreference');
      if (stored) {
        const parsed = JSON.parse(stored);
        return Array.isArray(parsed) ? parsed : [];
      }
    } catch (e) {
      // Migration: if it's an old string value, clear it
      localStorage.removeItem('trackerPreference');
    }
  }
  return [];
}

export function setTrackerPreference(trackers) {
  if (typeof window !== 'undefined') {
    const trackersArray = Array.isArray(trackers) ? trackers : [];
    localStorage.setItem('trackerPreference', JSON.stringify(trackersArray));
    console.log('🔧 Tracker preference set to:', trackersArray);
  }
}
