<script>
  export let activeTab = 'movies';
  
  const tabs = [
    { id: 'movies', label: 'Movies' },
    { id: 'tv', label: 'TV' }
  ];
</script>

<div class="tab-bar">
  {#each tabs as tab}
    <button 
      class="tab-button"
      class:active={activeTab === tab.id}
      on:click={() => activeTab = tab.id}
    >
      {tab.label}
    </button>
  {/each}
</div>
