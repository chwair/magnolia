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

    addItem: (item) => {
      update(history => {
        // Remove if already exists
        const filtered = history.filter(media => 
          !(media.id === item.id && media.media_type === item.media_type)
        );
        
        // Add to front with timestamp
        const newHistory = [
          { ...item, watchedAt: Date.now() },
          ...filtered
        ].slice(0, 20); // Keep only last 20 items
        
        if (typeof window !== 'undefined') {
          localStorage.setItem('watchHistory', JSON.stringify(newHistory));
        }
        
        console.log('📺 Added to watch history:', item.title || item.name);
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
