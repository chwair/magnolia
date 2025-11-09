import { writable } from 'svelte/store';

function loadFromLocalStorage() {
  if (typeof window !== 'undefined') {
    try {
      const savedList = localStorage.getItem('myList');
      return savedList ? JSON.parse(savedList) : [];
    } catch (error) {
      console.error('Error loading list from localStorage:', error);
      return [];
    }
  }
  return [];
}

function createListStore() {
  const { subscribe, set, update } = writable(loadFromLocalStorage());

  return {
    subscribe,

    toggleItem: (item) => {
      update(list => {
        const index = list.findIndex(media => 
          media.id === item.id && media.media_type === item.media_type
        );
        
        let newList;
        if (index >= 0) {
          newList = list.filter((_, i) => i !== index);
          console.log('🗑️ Removed from list:', item.title || item.name);
        } else {
          newList = [item, ...list];
          console.log('➕ Added to list:', item.title || item.name);
        }
        
        if (typeof window !== 'undefined') {
          localStorage.setItem('myList', JSON.stringify(newList));
        }
        
        console.log('📋 List now has', newList.length, 'items');
        return newList;
      });
    },

    isInList: (item, list) => {
      return list.some(media => 
        media.id === item.id && media.media_type === item.media_type
      );
    },
    
    reload: () => {
      set(loadFromLocalStorage());
    },
    
    setList: (newList) => {
      set(newList);
      if (typeof window !== 'undefined') {
        localStorage.setItem('myList', JSON.stringify(newList));
      }
    }
  };
}

export const myListStore = createListStore();

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (e.key === 'myList') {
      myListStore.reload();
    }
  });
}
