import { writable } from 'svelte/store';

function loadFromLocalStorage() {
  if (typeof window !== 'undefined') {
    try {
      const savedProgress = localStorage.getItem('watchProgress');
      return savedProgress ? JSON.parse(savedProgress) : {};
    } catch (error) {
      console.error('Error loading watch progress from localStorage:', error);
      return {};
    }
  }
  return {};
}

function createWatchProgressStore() {
  const { subscribe, set, update } = writable(loadFromLocalStorage());

  return {
    subscribe,

    updateProgress: (mediaId, mediaType, data) => {
      update(progress => {
        const key = `${mediaId}-${mediaType}`;
        progress[key] = {
          ...data,
          updatedAt: Date.now()
        };
        
        if (typeof window !== 'undefined') {
          localStorage.setItem('watchProgress', JSON.stringify(progress));
        }
        
        console.log('📊 Updated watch progress:', key, data);
        return progress;
      });
    },

    getProgress: (mediaId, mediaType) => {
      const progress = loadFromLocalStorage();
      const key = `${mediaId}-${mediaType}`;
      return progress[key] || null;
    },

    removeProgress: (mediaId, mediaType) => {
      update(progress => {
        const key = `${mediaId}-${mediaType}`;
        delete progress[key];
        
        if (typeof window !== 'undefined') {
          localStorage.setItem('watchProgress', JSON.stringify(progress));
        }
        
        console.log('🗑️ Removed watch progress:', key);
        return progress;
      });
    },

    clear: () => {
      set({});
      if (typeof window !== 'undefined') {
        localStorage.removeItem('watchProgress');
      }
      console.log('🗑️ All watch progress cleared');
    },
    
    reload: () => {
      set(loadFromLocalStorage());
    }
  };
}

export const watchProgressStore = createWatchProgressStore();

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (e.key === 'watchProgress') {
      watchProgressStore.reload();
    }
  });
}
