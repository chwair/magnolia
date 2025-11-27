<script>
    import { createEventDispatcher, onDestroy } from "svelte";
    import { fade, scale } from "svelte/transition";
    import { getTrackerPreference, setTrackerPreference } from "./stores/watchHistoryStore.js";

    export let results = [];
    export let searchQuery = "";
    export let loading = false;

    const dispatch = createEventDispatcher();
    
    let trackerMode = 'auto'; // 'auto' or 'manual'
    let selectedTrackers = []; // For manual mode: ['nyaa', '1337x', etc]
    
    // Initialize from stored preference
    const storedPref = getTrackerPreference();
    if (Array.isArray(storedPref) && storedPref.length > 0) {
        trackerMode = 'manual';
        selectedTrackers = storedPref;
    } else {
        trackerMode = 'auto';
    }
    
    onDestroy(() => {
        if (researchTimeout) {
            clearTimeout(researchTimeout);
        }
    });

    // Filter state
    let selectedBatch = "all"; // all, batch, single
    let selectedQuality = "all";
    let selectedEncode = "all";
    let selectedAudioCodec = "all"; // all, no-ac3, specific codec
    let hideIncompatible = true; // Hide incompatible audio codecs by default
    let sortBy = "relevance"; // relevance (backend order), seeds, size, name, peers
    let sortDirection = "desc"; // asc, desc
    let searchFilter = "";

    // Derived values
    $: availableQualities = [...new Set(results.map(r => r.quality).filter(Boolean))].sort();
    $: availableEncodes = [...new Set(results.map(r => r.encode).filter(Boolean))].sort();
    $: availableAudioCodecs = [...new Set(results.map(r => r.audio_codec).filter(Boolean))].sort();

    // Check if audio codec is web-compatible
    // Supported: AAC, MP3, Opus, Vorbis, FLAC
    // Unsupported: AC3, E-AC3, DTS, DTS-HD, TrueHD
    function isWebCompatible(codec) {
        if (!codec) return true; // Unknown assumed compatible
        const compatible = ['AAC', 'MP3', 'Opus', 'Vorbis', 'FLAC'];
        return compatible.includes(codec);
    }

    // Filtered and sorted results
    $: filteredResults = results
        .filter(torrent => {
            // Search filter
            if (searchFilter && !torrent.title.toLowerCase().includes(searchFilter.toLowerCase())) {
                return false;
            }

            // Batch filter
            if (selectedBatch === "batch" && !torrent.is_batch) return false;
            if (selectedBatch === "single" && torrent.is_batch) return false;

            // Quality filter
            if (selectedQuality !== "all" && torrent.quality !== selectedQuality) return false;

            // Encode filter
            if (selectedEncode !== "all" && torrent.encode !== selectedEncode) return false;

            // Audio codec filter - hide incompatible formats by default
            if (hideIncompatible && !isWebCompatible(torrent.audio_codec)) return false;
            if (selectedAudioCodec !== "all" && torrent.audio_codec !== selectedAudioCodec) return false;

            return true;
        })
        .sort((a, b) => {
            let comparison = 0;
            
            if (sortBy === "relevance") {
                return 0; // Keep backend order (already sorted by relevance)
            } else if (sortBy === "seeds") {
                comparison = b.seeds - a.seeds;
            } else if (sortBy === "peers") {
                comparison = b.peers - a.peers;
            } else if (sortBy === "size") {
                comparison = parseSize(b.size) - parseSize(a.size);
            } else if (sortBy === "name") {
                comparison = a.title.localeCompare(b.title);
            }
            
            return sortDirection === "desc" ? comparison : -comparison;
        });

    function parseSize(sizeStr) {
        const units = { 'B': 1, 'KiB': 1024, 'MiB': 1024**2, 'GiB': 1024**3, 'TiB': 1024**4 };
        const match = sizeStr.match(/^([\d.]+)\s*(\w+)$/);
        if (!match) return 0;
        return parseFloat(match[1]) * (units[match[2]] || 1);
    }

    function selectTorrent(torrent) {
        dispatch("select", torrent);
    }

    function close() {
        dispatch("close");
    }

    function resetFilters() {
        selectedBatch = "all";
        selectedQuality = "all";
        selectedEncode = "all";
        selectedAudioCodec = "all";
        hideIncompatible = true; // Keep incompatible formats hidden by default
        sortBy = "relevance";
        sortDirection = "desc";
        searchFilter = "";
    }

    function toggleSort(column) {
        if (sortBy === column) {
            sortDirection = sortDirection === "desc" ? "asc" : "desc";
        } else {
            sortBy = column;
            sortDirection = column === "name" ? "asc" : "desc";
        }
    }

    let researchTimeout;
    
    function selectAuto() {
        trackerMode = 'auto';
        selectedTrackers = [];
        triggerResearch();
    }
    
    function toggleTracker(tracker) {
        // Switching to manual mode by selecting a tracker
        if (trackerMode === 'auto') {
            trackerMode = 'manual';
            selectedTrackers = [tracker];
        } else {
            const index = selectedTrackers.indexOf(tracker);
            if (index > -1) {
                selectedTrackers = selectedTrackers.filter(t => t !== tracker);
                // If all deselected, switch back to auto
                if (selectedTrackers.length === 0) {
                    trackerMode = 'auto';
                }
            } else {
                selectedTrackers = [...selectedTrackers, tracker];
            }
        }
        triggerResearch();
    }
    
    function triggerResearch() {
        clearTimeout(researchTimeout);
        researchTimeout = setTimeout(() => {
            const trackerData = trackerMode === 'auto' ? [] : selectedTrackers;
            console.log("Dispatching research event with trackers:", trackerData);
            setTrackerPreference(trackerData);
            dispatch("research", { trackers: trackerData });
        }, 300);
    }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="modal-overlay" transition:fade on:click={close}>
    <div class="modal-content" transition:scale on:click|stopPropagation>
        <div class="modal-header">
            <h3>Select a Torrent</h3>
            <div class="tracker-selector">
                <span class="tracker-label">Tracker:</span>
                <div class="tracker-buttons">
                    <button 
                        class="tracker-btn" 
                        class:active={trackerMode === 'auto'}
                        on:click={selectAuto}
                    >
                        Auto
                    </button>
                    <button 
                        class="tracker-btn" 
                        class:active={selectedTrackers.includes('nyaa')}
                        on:click={() => toggleTracker('nyaa')}
                    >
                        Nyaa
                    </button>
                    <button 
                        class="tracker-btn" 
                        class:active={selectedTrackers.includes('1337x')}
                        on:click={() => toggleTracker('1337x')}
                    >
                        1337x
                    </button>
                    <button 
                        class="tracker-btn" 
                        class:active={selectedTrackers.includes('thepiratebay')}
                        on:click={() => toggleTracker('thepiratebay')}
                    >
                        TPB
                    </button>
                    <button 
                        class="tracker-btn" 
                        class:active={selectedTrackers.includes('eztv')}
                        on:click={() => toggleTracker('eztv')}
                    >
                        EZTV
                    </button>
                </div>
            </div>
        </div>

        <div class="search-info">
            <div class="search-input-wrapper">
                <i class="ri-search-line"></i>
                <input 
                    type="text" 
                    placeholder="Filter torrents..." 
                    bind:value={searchFilter}
                    class="search-input"
                />
                {#if searchFilter}
                    <button class="clear-search" on:click={() => searchFilter = ""}>
                        <i class="ri-close-line"></i>
                    </button>
                {/if}
            </div>
            <div class="search-meta">
                <p>Results for: <strong>{searchQuery}</strong></p>
                <span class="result-count">{filteredResults.length} of {results.length} results</span>
            </div>
        </div>

        {#if !loading && results.length > 0}
            <div class="filters-bar">
                <div class="filter-group">
                    <span class="filter-label">Type:</span>
                    <div class="filter-options">
                        <button 
                            class="filter-chip" 
                            class:active={selectedBatch === 'all'}
                            on:click={() => selectedBatch = 'all'}
                        >
                            All
                        </button>
                        <button 
                            class="filter-chip" 
                            class:active={selectedBatch === 'single'}
                            on:click={() => selectedBatch = 'single'}
                        >
                            Single
                        </button>
                        <button 
                            class="filter-chip" 
                            class:active={selectedBatch === 'batch'}
                            on:click={() => selectedBatch = 'batch'}
                        >
                            Batch
                        </button>
                    </div>
                </div>

                {#if availableQualities.length > 0}
                    <div class="filter-group">
                        <span class="filter-label">Quality:</span>
                        <div class="filter-options">
                            <button 
                                class="filter-chip" 
                                class:active={selectedQuality === 'all'}
                                on:click={() => selectedQuality = 'all'}
                            >
                                All
                            </button>
                            {#each availableQualities as quality}
                                <button 
                                    class="filter-chip" 
                                    class:active={selectedQuality === quality}
                                    on:click={() => selectedQuality = quality}
                                >
                                    {quality}
                                </button>
                            {/each}
                        </div>
                    </div>
                {/if}

                {#if availableEncodes.length > 0}
                    <div class="filter-group">
                        <span class="filter-label">Encode:</span>
                        <div class="filter-options">
                            <button 
                                class="filter-chip" 
                                class:active={selectedEncode === 'all'}
                                on:click={() => selectedEncode = 'all'}
                            >
                                All
                            </button>
                            {#each availableEncodes as encode}
                                <button 
                                    class="filter-chip" 
                                    class:active={selectedEncode === encode}
                                    on:click={() => selectedEncode = encode}
                                >
                                    {encode}
                                </button>
                            {/each}
                        </div>
                    </div>
                {/if}

                <!-- Audio Codec Filter - Hide Incompatible Toggle -->
                <div class="filter-group">
                    <span class="filter-label">Compatibility:</span>
                    <div class="filter-options">
                        <button 
                            class="filter-chip" 
                            class:active={hideIncompatible}
                            on:click={() => hideIncompatible = !hideIncompatible}
                            title="Hide torrents with incompatible audio codecs (AC3, DTS, TrueHD, etc.)"
                        >
                            <i class="ri-{hideIncompatible ? 'eye-off' : 'eye'}-line"></i>
                            Hide Incompatible
                        </button>
                    </div>
                </div>

                <button class="reset-btn" on:click={resetFilters}>
                    <i class="ri-refresh-line"></i>
                </button>
            </div>
        {/if}

        <div class="results-list">
            {#if loading}
                <div class="loading-state">
                    <div class="spinner"></div>
                    <p>Searching {trackerMode === 'auto' ? 'multiple trackers' : `${selectedTrackers.length} tracker(s)`} for "{searchQuery}"...</p>
                    <span class="loading-subtext">This may take a few seconds</span>
                </div>
            {:else if results.length === 0}
                <div class="empty-state">
                    <i class="ri-file-search-line"></i>
                    <p>No results found</p>
                </div>
            {:else if filteredResults.length === 0}
                <div class="empty-state">
                    <i class="ri-filter-line"></i>
                    <p>No results match the current filters</p>
                    <button class="reset-btn-alt" on:click={resetFilters}>
                        Reset Filters
                    </button>
                </div>
            {:else}
                <div class="table-header">
                    <!-- svelte-ignore a11y-click-events-have-key-events -->
                    <!-- svelte-ignore a11y-no-static-element-interactions -->
                    <div class="header-col col-name" on:click={() => toggleSort('name')}>
                        <span>NAME</span>
                        {#if sortBy === 'name'}
                            <i class="ri-arrow-{sortDirection === 'asc' ? 'up' : 'down'}-s-line"></i>
                        {/if}
                    </div>
                    <!-- svelte-ignore a11y-click-events-have-key-events -->
                    <!-- svelte-ignore a11y-no-static-element-interactions -->
                    <div class="header-col col-size" on:click={() => toggleSort('size')}>
                        <span>SIZE</span>
                        {#if sortBy === 'size'}
                            <i class="ri-arrow-{sortDirection === 'asc' ? 'up' : 'down'}-s-line"></i>
                        {/if}
                    </div>
                    <!-- svelte-ignore a11y-click-events-have-key-events -->
                    <!-- svelte-ignore a11y-no-static-element-interactions -->
                    <div class="header-col col-seeds" on:click={() => toggleSort('seeds')}>
                        <span>SEEDS</span>
                        {#if sortBy === 'seeds'}
                            <i class="ri-arrow-{sortDirection === 'asc' ? 'up' : 'down'}-s-line"></i>
                        {/if}
                    </div>
                    <!-- svelte-ignore a11y-click-events-have-key-events -->
                    <!-- svelte-ignore a11y-no-static-element-interactions -->
                    <div class="header-col col-peers" on:click={() => toggleSort('peers')}>
                        <span>PEERS</span>
                        {#if sortBy === 'peers'}
                            <i class="ri-arrow-{sortDirection === 'asc' ? 'up' : 'down'}-s-line"></i>
                        {/if}
                    </div>
                </div>
                <div class="table-body">
                    {#each filteredResults as torrent}
                        <div
                            class="torrent-row"
                            on:click={() => selectTorrent(torrent)}
                        >
                            <div class="col-name">
                                <div class="torrent-title">{torrent.title}</div>
                                {#if torrent.quality || torrent.encode || torrent.is_batch || torrent.season || torrent.episode}
                                    <div class="metadata-tags">
                                        {#if torrent.season && torrent.episode}
                                            <span class="tag tag-episode">S{torrent.season.toString().padStart(2, '0')}E{torrent.episode.toString().padStart(2, '0')}</span>
                                        {:else if torrent.season}
                                            <span class="tag tag-episode">Season {torrent.season}</span>
                                        {/if}
                                        {#if torrent.quality}
                                            <span class="tag tag-quality">{torrent.quality}</span>
                                        {/if}
                                        {#if torrent.encode}
                                            <span class="tag tag-encode">{torrent.encode}</span>
                                        {/if}
                                        {#if torrent.is_batch}
                                            <span class="tag tag-batch">BATCH</span>
                                        {/if}
                                    </div>
                                {/if}
                            </div>
                            <div class="col-size">{torrent.size}</div>
                            <div
                                class="col-seeds {torrent.seeds > 0
                                    ? 'has-seeds'
                                    : ''}"
                            >
                                {torrent.seeds}
                            </div>
                            <div class="col-peers">{torrent.peers}</div>
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    </div>
</div>

<style>
    .modal-overlay {
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background: rgba(0, 0, 0, 0.85);
        display: flex;
        justify-content: center;
        align-items: center;
        z-index: 9750;
        backdrop-filter: blur(8px);
    }

    .modal-content {
        background: var(--bg-primary);
        width: 92%;
        max-width: 1100px;
        max-height: 85vh;
        border-radius: var(--border-radius-lg);
        border: 1px solid rgba(255, 255, 255, 0.08);
        display: flex;
        flex-direction: column;
        box-shadow: var(--shadow-depth), 0 20px 60px rgba(0, 0, 0, 0.6);
        overflow: hidden;
    }

    .modal-header {
        padding: var(--spacing-xl) var(--spacing-2xl);
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        display: flex;
        justify-content: space-between;
        align-items: center;
        background: var(--bg-secondary);
    }

    .modal-header h3 {
        margin: 0;
        font-size: 18px;
        font-weight: 600;
        color: var(--text-primary);
        letter-spacing: -0.01em;
    }

    .tracker-selector {
        display: flex;
        align-items: center;
        gap: var(--spacing-md);
    }

    .tracker-label {
        font-size: 13px;
        color: var(--text-secondary);
        font-weight: 500;
    }

    .tracker-buttons {
        display: flex;
        gap: var(--spacing-xs);
    }

    .tracker-btn {
        padding: 6px 14px;
        background: var(--bg-tertiary);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: var(--text-secondary);
        border-radius: var(--border-radius-sm);
        font-size: 12px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s ease;
        font-family: inherit;
    }

    .tracker-btn:hover {
        background: rgba(255, 255, 255, 0.08);
        border-color: rgba(255, 255, 255, 0.15);
        color: var(--text-primary);
    }

    .tracker-btn.active {
        background: var(--accent-color);
        border-color: var(--accent-color);
        color: white;
    }

    .search-info {
        padding: var(--spacing-lg) var(--spacing-2xl);
        background: rgba(0, 0, 0, 0.3);
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        display: flex;
        flex-direction: column;
        gap: var(--spacing-md);
    }

    .search-input-wrapper {
        position: relative;
        display: flex;
        align-items: center;
    }

    .search-input-wrapper i.ri-search-line {
        position: absolute;
        left: var(--spacing-md);
        color: var(--text-tertiary);
        font-size: 16px;
        pointer-events: none;
    }

    .search-input {
        width: 100%;
        background: var(--bg-tertiary);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: var(--text-primary);
        padding: 10px 40px;
        border-radius: var(--border-radius-sm);
        font-size: 13px;
        font-family: inherit;
        transition: all 0.2s ease;
    }

    .search-input:focus {
        outline: none;
        background: rgba(255, 255, 255, 0.08);
        border-color: var(--accent-color);
        box-shadow: 0 0 0 3px rgba(211, 118, 195, 0.1);
    }

    .search-input::placeholder {
        color: var(--text-tertiary);
    }

    .clear-search {
        position: absolute;
        right: var(--spacing-sm);
        background: rgba(255, 255, 255, 0.08);
        border: none;
        color: var(--text-secondary);
        width: 24px;
        height: 24px;
        border-radius: 4px;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        transition: all 0.2s ease;
        padding: 0;
    }

    .clear-search:hover {
        background: rgba(255, 255, 255, 0.12);
        color: var(--text-primary);
    }

    .search-meta {
        display: flex;
        justify-content: space-between;
        align-items: center;
        color: var(--text-secondary);
        font-size: 13px;
    }

    .search-meta p {
        margin: 0;
    }

    .search-meta strong {
        color: var(--text-primary);
        font-weight: 500;
    }

    .result-count {
        font-size: 12px;
        color: var(--text-tertiary);
        font-family: "Geist Mono Variable", monospace;
    }

    .filters-bar {
        display: flex;
        gap: var(--spacing-xl);
        padding: var(--spacing-lg) var(--spacing-2xl);
        background: var(--bg-secondary);
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        align-items: flex-start;
        flex-wrap: wrap;
    }

    .filter-group {
        display: flex;
        flex-direction: column;
        gap: var(--spacing-sm);
    }

    .filter-label {
        font-size: 11px;
        color: var(--text-tertiary);
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    .filter-options {
        display: flex;
        gap: var(--spacing-sm);
        flex-wrap: wrap;
    }

    .filter-chip {
        background: var(--bg-tertiary);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: var(--text-secondary);
        padding: 6px 12px;
        border-radius: var(--border-radius-sm);
        font-size: 12px;
        cursor: pointer;
        transition: all 0.2s ease;
        font-weight: 500;
        white-space: nowrap;
    }

    .filter-chip:hover {
        background: rgba(255, 255, 255, 0.08);
        color: var(--text-primary);
    }

    .filter-chip.active {
        background: var(--accent-color);
        color: #000;
        border-color: var(--accent-color);
    }

    .reset-btn {
        background: rgba(239, 68, 68, 0.1);
        border: 1px solid rgba(239, 68, 68, 0.25);
        color: #ef4444;
        padding: 6px;
        border-radius: var(--border-radius-sm);
        font-size: 16px;
        cursor: pointer;
        transition: all 0.2s ease;
        display: flex;
        align-items: center;
        justify-content: center;
        width: 32px;
        height: 32px;
        margin-top: 18px;
    }

    .reset-btn:hover {
        background: rgba(239, 68, 68, 0.2);
        border-color: rgba(239, 68, 68, 0.4);
        transform: scale(1.05);
    }

    .reset-btn-alt {
        background: rgba(239, 68, 68, 0.1);
        border: 1px solid rgba(239, 68, 68, 0.25);
        color: #ef4444;
        padding: 8px 16px;
        border-radius: var(--border-radius-sm);
        font-size: 13px;
        cursor: pointer;
        transition: all 0.2s ease;
        display: flex;
        align-items: center;
        gap: var(--spacing-sm);
        font-weight: 500;
        white-space: nowrap;
        margin-top: var(--spacing-md);
    }

    .reset-btn-alt:hover {
        background: rgba(239, 68, 68, 0.2);
        border-color: rgba(239, 68, 68, 0.4);
        transform: scale(1.02);
    }

    .results-list {
        flex: 1;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
    }

    .table-header {
        display: flex;
        padding: var(--spacing-md) var(--spacing-2xl);
        background: rgba(0, 0, 0, 0.4);
        backdrop-filter: blur(10px);
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        position: sticky;
        top: 0;
        z-index: 10;
        font-family: "Geist Mono Variable", monospace;
        gap: var(--spacing-lg);
    }

    .header-col {
        display: flex;
        align-items: center;
        gap: 6px;
        color: var(--text-tertiary);
        font-size: 11px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        cursor: pointer;
        transition: color 0.2s ease;
        user-select: none;
        pointer-events: auto;

        &.col-name {
            letter-spacing: 0.05em;
            flex: 1;
            min-width: 0;
            width: 70px;
            text-align: left !important;
            font-family: "Geist Mono Variable", monospace;
            font-size: 13px;
            font-weight: 600;
            color: var(--text-tertiary);
            flex-shrink: 0;
            flex-direction: row;
        }
    }

    .header-col:hover {
        color: var(--text-primary);
    }

    .header-col span {
        white-space: nowrap;
    }

    .header-col i {
        font-size: 14px;
        color: var(--accent-color);
        flex-shrink: 0;
    }

    .table-header .col-name {
        flex: 1;
        min-width: 0;
    }

    .table-header .col-size {
        width: 90px;
        flex-shrink: 0;
    }

    .table-header .col-seeds {
        width: 70px;
        flex-shrink: 0;
    }

    .table-header .col-peers {
        width: 70px;
        flex-shrink: 0;
    }

    .table-body {
        display: flex;
        flex-direction: column;
    }

    .torrent-row {
        display: flex;
        padding: var(--spacing-md) var(--spacing-2xl);
        cursor: pointer;
        transition: all 0.15s ease;
        color: var(--text-secondary);
        font-size: 13px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.02);
        align-items: center;
        background: transparent;
        gap: var(--spacing-lg);
    }

    .torrent-row:hover {
        background: var(--bg-tertiary);
        color: var(--text-primary);
    }

    .torrent-row:active {
        background: rgba(255, 255, 255, 0.08);
    }

    .col-name {
        flex: 1;
        font-weight: 500;
        display: flex;
        flex-direction: column;
        gap: var(--spacing-sm);
        min-width: 0;
    }

    .torrent-title {
        position: relative;
        white-space: nowrap;
        overflow: hidden;
        padding-right: 60px;
    }

    .torrent-title::after {
        content: '';
        position: absolute;
        right: 0;
        top: 0;
        bottom: 0;
        width: 60px;
        background: linear-gradient(to right, transparent, var(--bg-primary));
        pointer-events: none;
    }

    .torrent-row:hover .torrent-title::after {
        display: none;
    }

    .metadata-tags {
        display: flex;
        gap: var(--spacing-xs);
        flex-wrap: wrap;
    }

    .tag {
        font-size: 10px;
        padding: 3px var(--spacing-sm);
        border-radius: 4px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.04em;
        white-space: nowrap;
    }

    .tag-episode {
        background: rgba(59, 130, 246, 0.12);
        color: #60a5fa;
        border: 1px solid rgba(59, 130, 246, 0.3);
    }

    .tag-quality {
        background: rgba(16, 185, 129, 0.12);
        color: #34d399;
        border: 1px solid rgba(16, 185, 129, 0.3);
    }

    .tag-encode {
        background: rgba(168, 85, 247, 0.12);
        color: #c084fc;
        border: 1px solid rgba(168, 85, 247, 0.3);
    }

    .tag-batch {
        background: rgba(251, 191, 36, 0.12);
        color: #fbbf24;
        border: 1px solid rgba(251, 191, 36, 0.3);
    }

    .col-size {
        width: 90px;
        text-align: left;
        font-family: "Geist Mono Variable", monospace;
        font-size: 12px;
        color: var(--text-secondary);
        flex-shrink: 0;
    }

    .col-seeds {
        width: 70px;
        text-align: left;
        font-family: "Geist Mono Variable", monospace;
        font-size: 13px;
        font-weight: 600;
        color: var(--text-tertiary);
        flex-shrink: 0;
    }

    .col-seeds.has-seeds {
        color: #10b981;
    }

    .col-peers {
        width: 70px;
        text-align: left;
        font-family: "Geist Mono Variable", monospace;
        font-size: 12px;
        color: var(--text-tertiary);
        flex-shrink: 0;
    }

    .loading-state,
    .empty-state {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: var(--spacing-3xl) var(--spacing-2xl);
        color: var(--text-secondary);
        gap: var(--spacing-lg);
        min-height: 300px;
    }

    .loading-state p {
        margin: 0;
        font-size: 14px;
        font-weight: 500;
    }

    .loading-subtext {
        font-size: 12px;
        color: var(--text-tertiary);
        margin-top: -8px;
    }

    .spinner {
        width: 40px;
        height: 40px;
        border: 3px solid rgba(255, 255, 255, 0.08);
        border-top-color: var(--accent-color);
        border-radius: 50%;
        animation: spin 1s linear infinite;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    .empty-state i {
        font-size: 48px;
        opacity: 0.3;
        margin-bottom: var(--spacing-sm);
    }

    .empty-state p {
        font-size: 14px;
        opacity: 0.7;
    }
</style>
