<script>
  import { onMount, onDestroy } from "svelte";
  import {
    getMovieDetails,
    getTVDetails,
    getSeasonDetails,
    getMovieCredits,
    getTVCredits,
    getMovieRecommendations,
    getTVRecommendations,
    getImageUrl,
  } from "./tmdb.js";
  import { myListStore } from "./stores/listStore.js";
  import { watchProgressStore } from "./stores/watchProgressStore.js";
  import { getTrackerPreference, setTrackerPreference } from "./stores/watchHistoryStore.js";
  import { invoke } from "@tauri-apps/api/core";
  import TorrentSelector from "./TorrentSelector.svelte";
  import ErrorModal from "./ErrorModal.svelte";

  import { createEventDispatcher } from "svelte";

  export let media = null;

  const dispatch = createEventDispatcher();

  let details = null;
  let loading = true;
  let backdropColor = "#1a1a1a";
  let prominentColor = "#1a1a1a";
  let textColor = "#ffffff";
  let selectedSeason = null;
  let seasonDetails = null;
  let allSeasonsData = {};
  let selectedEpisode = null;
  let credits = null;
  let recommendations = [];
  let activeTab = "";
  let availableTabs = [];
  let viewMode = "list"; // 'list' or 'grid'

  // Torrent Search State
  let showTorrentSelector = false;
  let searchResults = [];
  let isSearching = false;
  let currentSearchQuery = "";
  let pendingPlayRequest = null; // { season, episode }
  
  // Manual file selection state
  let showFileSelector = false;
  let availableFiles = [];
  let selectedTorrentForManual = null;
  let manualHandleId = null;
  let autoPlayTriggered = false;

  // Error modal state
  let showErrorModal = false;
  let errorMessage = "";
  let errorTitle = "Error";

  $: {
    if (media) {
      // Ensure media_type is set
      if (!media.media_type) {
        media.media_type = media.title ? "movie" : "tv";
      }
    }
  }

  $: isInMyList =
    media &&
    $myListStore.some(
      (item) => item.id === media.id && item.media_type === media.media_type,
    );

  // Helper function to detect anime (Animation genre ID is 16 in TMDB)
  const isAnime = () => {
    if (!details || !details.genres) return false;
    return details.genres.some(genre => genre.id === 16);
  };

  $: {
    if (media && isInMyList !== undefined) {
      console.log(
        "🎥 MediaDetail: isInMyList updated for",
        media.title || media.name,
        ":",
        isInMyList,
      );
    }
  }

  $: if (media) {
    loadDetails();
    selectedSeason = null;
    seasonDetails = null;
    allSeasonsData = {};
    selectedEpisode = null;
    recommendations = [];
    autoPlayTriggered = false; // Reset autoplay flag for new media
  }

  $: if (details) {
    availableTabs = [];
    if (details.seasons && details.seasons.length > 0) {
      availableTabs.push("seasons");
    }
    availableTabs.push("cast");
    availableTabs.push("details");

    if (details.seasons && details.seasons.length > 0) {
      activeTab = "seasons";
      // Skip specials (season 0) when selecting default
      const firstSeason = details.seasons.find((s) => s.season_number > 0);
      if (firstSeason) {
        selectedSeason = firstSeason.season_number;
      }
      loadAllSeasons();
    } else {
      activeTab = "cast";
    }

    loadRecommendations();
  }

  $: if (selectedSeason && details) {
    loadSeasonDetails();
  }

  onMount(() => {
    // Check for autoplay after details load
    if (media && media.autoPlay && !autoPlayTriggered) {
      autoPlayTriggered = true;
      setTimeout(() => {
        if (details) {
          handleAutoPlay();
        }
      }, 500);
    }
    
    const handleKeyDown = (e) => {
      if (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA")
        return;
      if (e.key === "Tab" && availableTabs.length > 0) {
        e.preventDefault();
        const currentIndex = availableTabs.indexOf(activeTab);
        if (e.shiftKey) {
          activeTab =
            availableTabs[
              (currentIndex - 1 + availableTabs.length) % availableTabs.length
            ];
        } else {
          activeTab = availableTabs[(currentIndex + 1) % availableTabs.length];
        }
      }
      if (e.key >= "1" && e.key <= "9") {
        const index = parseInt(e.key) - 1;
        if (index < availableTabs.length) {
          e.preventDefault();
          activeTab = availableTabs[index];
        }
      }

      if (e.key === "Escape" && selectedEpisode) {
        e.preventDefault();
        e.stopPropagation();
        selectedEpisode = null;
      }
    };

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  });

  async function loadDetails() {
    loading = true;
    details = null; // Clear previous details to prevent stale data
    try {
      if (media.media_type === "movie" || media.title) {
        details = await getMovieDetails(media.id);
        credits = await getMovieCredits(media.id);
      } else {
        details = await getTVDetails(media.id);
        credits = await getTVCredits(media.id);
      }

      if (details && details.backdrop_path) {
        await extractColors(getImageUrl(details.backdrop_path, "w300"));
      }
    } catch (err) {
      console.error("Error loading details:", err);
      details = null; // Ensure details is null on error
    }
    loading = false;
  }

  async function loadSeasonDetails() {
    try {
      seasonDetails = await getSeasonDetails(details.id, selectedSeason);
    } catch (err) {
      console.error("Error loading season:", err);
    }
  }

  async function loadAllSeasons() {
    if (!details || !details.seasons) return;

    try {
      const seasonPromises = details.seasons
        .filter((s) => s.season_number > 0)
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
      console.error("Error loading all seasons:", err);
    }
  }

  async function loadRecommendations() {
    try {
      let response;
      if (media.media_type === "movie" || media.title) {
        response = await getMovieRecommendations(media.id);
      } else {
        response = await getTVRecommendations(media.id);
      }
      recommendations = (response?.results?.slice(0, 10) || []).map((rec) => {
        // Ensure media_type is set for recommendations
        if (!rec.media_type) {
          rec.media_type =
            media.media_type === "movie" || media.title ? "movie" : "tv";
        }
        return rec;
      });
    } catch (err) {
      console.error("Error loading recommendations:", err);
    }
  }

  async function extractColors(imageUrl) {
    try {
      const img = new Image();
      img.crossOrigin = "Anonymous";
      img.src = imageUrl;
      await new Promise((resolve, reject) => {
        img.onload = resolve;
        img.onerror = reject;
      });

      const canvas = document.createElement("canvas");
      const ctx = canvas.getContext("2d");
      canvas.width = img.width;
      canvas.height = img.height;
      ctx.drawImage(img, 0, 0);

      const imageData = ctx.getImageData(
        0,
        0,
        canvas.width,
        canvas.height,
      ).data;

      // Build a histogram of mid-range brightness pixels to find a dominant accent color
      const colorMap = {};
      for (let i = 0; i < imageData.length; i += 4 * 10) {
        const r = imageData[i];
        const g = imageData[i + 1];
        const b = imageData[i + 2];
        const brightness = (r + g + b) / 3;

        if (brightness > 40 && brightness < 200) {
          const colorKey = `${Math.floor(r / 20) * 20},${Math.floor(g / 20) * 20},${Math.floor(b / 20) * 20}`;
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
        const [r, g, b] = dominantColor.split(",").map(Number);
        prominentColor = `rgb(${r}, ${g}, ${b})`;

        const hexColor = rgbToHex(r, g, b);
        window.dispatchEvent(
          new CustomEvent("updateTitleBarColor", {
            detail: { color: hexColor },
          }),
        );
        textColor = "#ffffff";
      }

      // Build a muted backdrop by averaging mid-range brightness values
      let br = 0,
        bg = 0,
        bb = 0,
        count = 0;
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
      console.error("Color extraction failed:", err);
    }
  }

  function rgbToHex(r, g, b) {
    return (
      "#" +
      [r, g, b]
        .map((x) => {
          const hex = x.toString(16);
          return hex.length === 1 ? "0" + hex : hex;
        })
        .join("")
    );
  }

  function toggleMyList() {
    console.log(
      "🎥 MediaDetail: Toggle for:",
      media.title || media.name,
      "Current state:",
      isInMyList,
    );
    myListStore.toggleItem(media);
  }

  function formatRuntime(minutes) {
    if (!minutes) return "N/A";
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return `${hours}h ${mins}m`;
  }

  function formatDate(dateStr) {
    if (!dateStr) return "N/A";
    return new Date(dateStr).toLocaleDateString("en-US", {
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  }

  function formatMoney(amount) {
    if (!amount) return "N/A";
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "USD",
      minimumFractionDigits: 0,
    }).format(amount);
  }

  import { getRatingClass } from "./utils/colorUtils.js";

  function showError(message, title = "Error") {
    errorMessage = message;
    errorTitle = title;
    showErrorModal = true;
  }

  function toggleSeason(seasonNumber) {
    if (selectedSeason === seasonNumber) {
      selectedSeason = null;
    } else {
      selectedSeason = seasonNumber;
    }
  }

  async function handleAutoPlay() {
    console.log('Auto-play triggered');
    
    // Clear autoPlay flag immediately to prevent re-triggering
    if (media && media.autoPlay) {
      media.autoPlay = false;
    }
    
    // Use resumeProgress if passed from quick play, otherwise load from store
    const progress = media.resumeProgress || watchProgressStore.getProgress(media.id, media.media_type);
    
    if (media.media_type === 'movie' || media.title) {
      // Movie: play from beginning (timestamp resume handled by VideoPlayer)
      await handlePlay(0, 0);
    } else if (progress && progress.currentSeason && progress.currentEpisode) {
      // TV Show: resume from last watched episode
      console.log(`Resuming from S${progress.currentSeason}E${progress.currentEpisode} at ${progress.currentTimestamp}s`);
      await handlePlay(progress.currentSeason, progress.currentEpisode);
    } else {
      // TV Show: start from S01E01
      await handlePlay(1, 1);
    }
  }

  async function handlePlay(seasonNum, episodeNum, forceReselect = false) {
    console.log(
      "Play requested:",
      seasonNum,
      episodeNum,
      "Force reselect:",
      forceReselect,
    );
    pendingPlayRequest = { season: seasonNum, episode: episodeNum };

    // 1. Check persistence (skip if forcing reselect)
    if (!forceReselect) {
      try {
        const saved = await invoke("get_saved_selection", {
          showId: details.id,
          season: seasonNum,
          episode: episodeNum,
        });

        if (saved) {
          console.log("Found saved torrent:", saved);
          startStream(saved.magnet_link, saved.file_index);
          return;
        }
      } catch (err) {
        console.error("Error checking saved selection:", err);
      }
    }

    // 2. Start Search
    isSearching = true;
    showTorrentSelector = true;
    searchResults = [];

    const showName = details.title || details.name;
    const isMovie = media.media_type === "movie" || !!details.title;

    // Build search query - just the name, no season/episode
    let searchQuery = showName;
    if (isMovie) {
      const year = (details.release_date || "").split("-")[0];
      if (year) {
        searchQuery = `${showName} ${year}`;
      }
      currentSearchQuery = `${showName} (Movie)`;
    } else {
      const s = seasonNum.toString().padStart(2, "0");
      const e = episodeNum.toString().padStart(2, "0");
      currentSearchQuery = `${showName} S${s}E${e}`;
    }

    console.log("Starting search:", searchQuery);

    // Detect if this is anime based on genre
    const mediaType = isAnime() ? "anime" : (isMovie ? "movie" : "tv");
    console.log("Media type for search:", mediaType, "Genres:", details.genres);

    // Get tracker preference
    const storedTrackers = getTrackerPreference();
    const trackerArray = Array.isArray(storedTrackers) && storedTrackers.length > 0 ? storedTrackers : null;
    console.log("Tracker preference:", trackerArray);

    // Execute search with filtering on backend
    try {
      searchResults = await invoke("search_nyaa_filtered", {
        query: searchQuery,
        season: isMovie ? null : seasonNum,
        episode: isMovie ? null : episodeNum,
        isMovie: isMovie,
        mediaType: mediaType,
        trackerPreference: trackerArray,
      });

      if (searchResults.length === 0) {
        console.log("No results found.");
      }
    } catch (err) {
      console.error("Search error:", err);
      searchResults = [];
    } finally {
      isSearching = false;
    }
  }

  function reselectTorrent() {
    if (details.seasons && details.seasons.length > 0) {
      // For TV shows, we need to know which episode.
      // If we have a selected episode, use that. Otherwise default to S1E1.
      if (selectedEpisode) {
        handlePlay(selectedSeason, selectedEpisode.episode_number, true);
      } else {
        handlePlay(1, 1, true);
      }
    } else {
      // Movie
      handlePlay(0, 0, true);
    }
  }

  async function onTorrentSelect(event) {
    const torrent = event.detail;
    console.log("Selected torrent:", torrent);

    if (!pendingPlayRequest) return;

    // We need to find the right file in the torrent.
    // For now, we'll add the torrent in "list_only" mode (via add_torrent) to get file list,
    // then try to match the episode.

    try {
      // 1. Add torrent to get metadata
      // Note: add_torrent returns a handle_id
      const handleId = await invoke("add_torrent", {
        magnetOrUrl: torrent.magnet_link,
      });
      const info = await invoke("get_torrent_info", { handleId });

      console.log("Torrent info:", info);

      // 2. Find the matching file
      // Simple heuristic: look for SXXEXX or just EXX in filename
      const s = pendingPlayRequest.season.toString().padStart(2, "0");
      const e = pendingPlayRequest.episode.toString().padStart(2, "0");

      let fileIndex = -1;

      // Try specific match first
      fileIndex = info.files.findIndex(
        (f) =>
          f.name.toUpperCase().includes(`S${s}E${e}`) ||
          f.name.toUpperCase().includes(`${pendingPlayRequest.season}X${e}`),
      );

      // If not found, and it's a single file torrent, use it
      if (fileIndex === -1 && info.files.length === 1) {
        fileIndex = 0;
      }

      // If still not found, maybe try just episode number if it's a season pack?
      if (fileIndex === -1) {
        fileIndex = info.files.findIndex(
          (f) =>
            f.name.toUpperCase().includes(`E${e}`) ||
            f.name.toUpperCase().includes(` ${e} `),
        );
      }

      if (fileIndex !== -1) {
        console.log("Found matching file at index:", fileIndex);

        // 3. Save selection
        await invoke("save_torrent_selection", {
          showId: details.id,
          season: pendingPlayRequest.season,
          episode: pendingPlayRequest.episode,
          magnetLink: torrent.magnet_link,
          fileIndex: info.files[fileIndex].index,
        });

        // If this is a batch torrent, save it for all episodes in the batch
        if (torrent.is_batch && info.files.length > 1) {
          console.log("Batch torrent detected, saving for all episodes");
          for (const file of info.files) {
            // Extract episode number from filename
            const epMatch = file.name.match(/[Ee](?:pisode)?\s*(\d+)|[-\s](\d{1,3})\s*(?:v\d)?/);
            if (epMatch) {
              const episodeNum = parseInt(epMatch[1] || epMatch[2]);
              if (episodeNum && episodeNum !== pendingPlayRequest.episode) {
                console.log(`Saving batch entry for episode ${episodeNum}`);
                await invoke("save_torrent_selection", {
                  showId: details.id,
                  season: pendingPlayRequest.season,
                  episode: episodeNum,
                  magnetLink: torrent.magnet_link,
                  fileIndex: file.index,
                });
              }
            }
          }
        }

        // 4. Start Stream
        startStream(torrent.magnet_link, info.files[fileIndex].index, handleId);
        showTorrentSelector = false;
      } else {
        // Show manual file selection
        showManualFileSelector(torrent, info, handleId);
      }
    } catch (err) {
      console.error("Error processing selection:", err);
      showError("Failed to load torrent metadata. Please try again.");
    }
  }

  async function startStream(magnetLink, fileIndex, existingHandleId = null) {
    try {
      let handleId = existingHandleId;
      if (handleId === null) {
        handleId = await invoke("add_torrent", { magnetOrUrl: magnetLink });
      }

      // Don't update progress here - let VideoPlayer handle it to preserve saved timestamps
      // The VideoPlayer will track progress during playback

      // Don't start stream here. Let VideoPlayer handle it so it can show loading screen.
      console.log("Opening player for handle:", handleId, "file:", fileIndex);

      // Build title - hide SXXEXX for movies
      const isMovie = media.media_type === "movie" || !!details.title;
      const playerTitle = isMovie 
        ? (details.title || details.name)
        : `${details.title || details.name} - S${pendingPlayRequest.season}E${pendingPlayRequest.episode}`;

      // Get saved progress for timestamp
      const progress = watchProgressStore.getProgress(details.id, media.media_type);
      let initialTimestamp = 0;
      
      // For TV shows, only use saved timestamp if we're playing the same episode
      if (!isMovie && progress && 
          progress.currentSeason === pendingPlayRequest.season && 
          progress.currentEpisode === pendingPlayRequest.episode) {
        initialTimestamp = progress.currentTimestamp || 0;
      } else if (isMovie && progress) {
        // For movies, always use saved timestamp
        initialTimestamp = progress.currentTimestamp || 0;
      }

      // Dispatch event to open video player
      window.dispatchEvent(
        new CustomEvent("openVideoPlayer", {
          detail: {
            src: null, // VideoPlayer will fetch this
            title: playerTitle,
            metadata: details, // Pass full details for watch history
            handleId: handleId,
            fileIndex: fileIndex,
            magnetLink: magnetLink,
            initialTimestamp: initialTimestamp,
            mediaId: details.id,
            mediaType: media.media_type,
            seasonNum: isMovie ? null : pendingPlayRequest.season,
            episodeNum: isMovie ? null : pendingPlayRequest.episode,
          },
        }),
      );
    } catch (err) {
      console.error("Error preparing stream:", err);
      showError("Failed to prepare stream. Please try again.");
    }
  }

  function closeTorrentSelector() {
    showTorrentSelector = false;
    pendingPlayRequest = null;
  }

  const handleResearch = async (event) => {
    console.log("=== RESEARCH EVENT RECEIVED ===");
    console.log("Event detail:", event.detail);
    console.log("isSearching:", isSearching);
    console.log("pendingPlayRequest:", pendingPlayRequest);
    
    const { trackers } = event.detail;
    
    // Prevent concurrent searches
    if (isSearching) {
      console.log("Search already in progress, ignoring research request");
      return;
    }
    
    if (!pendingPlayRequest) {
      console.log("No pending play request, ignoring research");
      return;
    }
    
    console.log("=== STARTING RESEARCH ===");
    console.log("New trackers:", trackers);
    
    // Re-run the search with new tracker preference
    isSearching = true;
    searchResults = [];

    const showName = details.title || details.name;
    const isMovieCheck = media.media_type === "movie" || !!details.title;
    const { season: seasonNum, episode: episodeNum } = pendingPlayRequest;

    let searchQuery = showName;
    if (isMovieCheck) {
      const year = (details.release_date || "").split("-")[0];
      if (year) {
        searchQuery = `${showName} ${year}`;
      }
    }

    const mediaType = isAnime() ? "anime" : (isMovieCheck ? "movie" : "tv");
    
    console.log("Invoking search_nyaa_filtered with:");
    console.log("- query:", searchQuery);
    console.log("- trackers:", trackers);
    
    try {
      searchResults = await invoke("search_nyaa_filtered", {
        query: searchQuery,
        season: isMovieCheck ? null : seasonNum,
        episode: isMovieCheck ? null : episodeNum,
        isMovie: isMovieCheck,
        mediaType: mediaType,
        trackerPreference: trackers && trackers.length > 0 ? trackers : null,
      });

      console.log(`=== RESEARCH COMPLETE ===`);
      console.log(`Found ${searchResults.length} results`);
    } catch (err) {
      console.error("Error during research:", err);
      searchResults = [];
    } finally {
      isSearching = false;
    }
  };

  function showManualFileSelector(torrent, info, handleId) {
    // Filter to video files only
    availableFiles = info.files.filter(f => {
      const ext = f.name.toLowerCase();
      return ext.endsWith('.mkv') || ext.endsWith('.mp4') || 
             ext.endsWith('.avi') || ext.endsWith('.mov') || 
             ext.endsWith('.webm') || ext.endsWith('.m4v');
    });
    
    if (availableFiles.length === 0) {
      showError("No video files found in this torrent.");
      return;
    }
    
    selectedTorrentForManual = torrent;
    manualHandleId = handleId;
    showFileSelector = true;
    showTorrentSelector = false;
  }

  async function selectManualFile(file) {
    try {
      // Save the selection
      await invoke("save_torrent_selection", {
        showId: details.id,
        season: pendingPlayRequest.season,
        episode: pendingPlayRequest.episode,
        magnetLink: selectedTorrentForManual.magnet_link,
        fileIndex: file.index,
      });

      // Try to infer numbering from selected file for batch torrents
      if (selectedTorrentForManual.is_batch && availableFiles.length > 1) {
        console.log("Inferring episode numbering from manual selection");
        
        // Extract episode number from selected filename
        const selectedEpMatch = file.name.match(/[Ee](?:pisode)?\s*(\d+)|[-\s](\d{1,3})\s*(?:v\d)?/);
        if (selectedEpMatch) {
          const selectedEpNum = parseInt(selectedEpMatch[1] || selectedEpMatch[2]);
          const offset = pendingPlayRequest.episode - selectedEpNum;
          
          console.log(`Selected file is E${selectedEpNum}, current episode is E${pendingPlayRequest.episode}, offset: ${offset}`);
          
          // Apply offset to other files
          for (const otherFile of availableFiles) {
            if (otherFile.index === file.index) continue;
            
            const epMatch = otherFile.name.match(/[Ee](?:pisode)?\s*(\d+)|[-\s](\d{1,3})\s*(?:v\d)?/);
            if (epMatch) {
              const fileEpNum = parseInt(epMatch[1] || epMatch[2]);
              const inferredEpisode = fileEpNum + offset;
              
              console.log(`Saving E${inferredEpisode} for file: ${otherFile.name}`);
              await invoke("save_torrent_selection", {
                showId: details.id,
                season: pendingPlayRequest.season,
                episode: inferredEpisode,
                magnetLink: selectedTorrentForManual.magnet_link,
                fileIndex: otherFile.index,
              });
            }
          }
        }
      }

      // Start stream
      startStream(selectedTorrentForManual.magnet_link, file.index, manualHandleId);
      closeFileSelector();
    } catch (err) {
      console.error("Error saving manual selection:", err);
      showError("Failed to save selection.");
    }
  }

  function closeFileSelector() {
    showFileSelector = false;
    availableFiles = [];
    selectedTorrentForManual = null;
    manualHandleId = null;
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="detail-overlay" style="animation: fadeIn 0.3s ease;">
  <div
    class="detail-container"
    style="--backdrop-color: {backdropColor}; --prominent-color: {prominentColor}; --text-color: {textColor}"
  >
    <button
      class="close-btn btn-standard"
      on:click={() => dispatch("close")}
      title="Back"
    >
      <i class="ri-arrow-left-line"></i>
    </button>

    {#if loading}
      <div class="loading">Loading...</div>
    {:else if details}
      <div class="detail-backdrop">
        {#if details.backdrop_path}
          <img
            src={getImageUrl(details.backdrop_path, "w1280")}
            alt="Backdrop"
          />
        {/if}
      </div>

      <div class="detail-content">
        <div class="detail-header">
          <div class="detail-poster poster-large">
            {#if details.poster_path}
              <img
                src={getImageUrl(details.poster_path, "w500")}
                alt={details.title || details.name}
              />
            {/if}
          </div>

          <div class="detail-info-wrapper">
            <div class="detail-info">
              <h1 class="detail-title detail-title-large">
                {details.title || details.name}
              </h1>
              {#if details.tagline}
                <p class="detail-tagline detail-tagline-large">
                  "{details.tagline}"
                </p>
              {/if}

              <div class="detail-meta">
                {#if details.vote_average !== undefined && details.vote_average !== null}
                <div class="rating-box {getRatingClass(details.vote_average)}">
                  {details.vote_average.toFixed(1)}
                </div>
                {/if}
                {#if details.content_ratings?.results?.length || details.release_dates?.results?.length}
                  <span class="age-rating">
                    {#if details.content_ratings}
                      {details.content_ratings.results.find(
                        (r) => r.iso_3166_1 === "US",
                      )?.rating || "NR"}
                    {:else if details.release_dates}
                      {details.release_dates.results.find(
                        (r) => r.iso_3166_1 === "US",
                      )?.release_dates?.[0]?.certification || "NR"}
                    {/if}
                  </span>
                {/if}
                <span
                  >{formatDate(
                    details.release_date || details.first_air_date,
                  )}</span
                >
                {#if details.runtime}
                  <span>•</span>
                  <span>{formatRuntime(details.runtime)}</span>
                {/if}
                {#if details.number_of_seasons}
                  <span>•</span>
                  <span
                    >{details.number_of_seasons} Season{details.number_of_seasons >
                    1
                      ? "s"
                      : ""}</span
                  >
                {/if}
              </div>

              {#if details.genres && details.genres.length > 0}
                <div class="detail-genres">
                  {#each details.genres as genre}
                    <span class="genre-tag genre-tag-large">{genre.name}</span>
                  {/each}
                </div>
              {/if}

              <p class="detail-overview detail-overview-large">
                {details.overview}
              </p>

              <div class="detail-actions">
                <button
                  class="btn-standard primary btn-large"
                  on:click={() => {
                    if (details.seasons && details.seasons.length > 0) {
                      // Default to S1E1
                      handlePlay(1, 1);
                    } else {
                      // Movie
                      handlePlay(0, 0); // Use 0,0 for movie? Or handle differently.
                      // Actually for movie we should pass something else or handle in handlePlay
                    }
                  }}
                >
                  <i class="ri-play-fill"></i>
                  Play
                </button>
                <button class="btn-standard btn-large" on:click={toggleMyList}>
                  <i class={isInMyList ? "ri-check-line" : "ri-add-line"}></i>
                  {isInMyList ? "In My List" : "My List"}
                </button>
                <button
                  class="btn-standard btn-icon-only"
                  on:click={reselectTorrent}
                  title="Reselect Torrent"
                >
                  <i class="ri-more-fill"></i>
                </button>
              </div>
            </div>
          </div>
        </div>

        <div class="detail-tabs">
          {#if details.seasons && details.seasons.length > 0}
            <button
              class="tab-btn"
              class:active={activeTab === "seasons"}
              on:click={() => (activeTab = "seasons")}
            >
              Seasons
            </button>
          {/if}
          <button
            class="tab-btn"
            class:active={activeTab === "cast"}
            on:click={() => (activeTab = "cast")}
          >
            Cast & Crew
          </button>
          <button
            class="tab-btn"
            class:active={activeTab === "details"}
            on:click={() => (activeTab = "details")}
          >
            Details
          </button>
        </div>

        <div class="tab-content">
          {#if activeTab === "seasons" && details.seasons && details.seasons.length > 0}
            <div class="seasons-header">
              <div class="view-toggle">
                <button
                  class="toggle-btn"
                  class:active={viewMode === "list"}
                  on:click={() => (viewMode = "list")}
                  title="List View"
                >
                  <i class="ri-list-check"></i>
                </button>
                <button
                  class="toggle-btn"
                  class:active={viewMode === "heatmap"}
                  on:click={() => (viewMode = "heatmap")}
                  title="Heatmap View"
                >
                  <i class="ri-grid-fill"></i>
                </button>
              </div>
            </div>

            {#if viewMode === "list"}
              <div class="seasons-accordion">
                {#each details.seasons.filter((s) => s.season_number > 0) as season}
                  <div
                    class="accordion-item"
                    class:expanded={selectedSeason === season.season_number}
                  >
                    <button
                      class="accordion-header"
                      on:click={() => toggleSeason(season.season_number)}
                    >
                      <div class="accordion-title">
                        <span class="season-name"
                          >Season {season.season_number}</span
                        >
                        <span class="episode-count"
                          >{season.episode_count} Episodes</span
                        >
                      </div>
                      <i class="ri-arrow-down-s-line accordion-icon"></i>
                    </button>

                    {#if selectedSeason === season.season_number}
                      <div class="accordion-content">
                        {#if allSeasonsData[season.season_number]}
                          <div class="episodes-list">
                            {#each allSeasonsData[season.season_number].episodes as episode}
                              {@const progressKey = `${details.id}-${media.media_type}`}
                              {@const progress = $watchProgressStore[progressKey]}
                              {@const hasProgress = progress && progress.currentSeason === season.season_number && progress.currentEpisode === episode.episode_number}
                              {@const progressText = hasProgress ? (() => {
                                const timestamp = progress.currentTimestamp || 0;
                                const hours = Math.floor(timestamp / 3600);
                                const minutes = Math.floor((timestamp % 3600) / 60);
                                const seconds = Math.floor(timestamp % 60);
                                if (hours > 0) return `${hours}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
                                return `${minutes}:${seconds.toString().padStart(2, '0')}`;
                              })() : ''}
                              <!-- svelte-ignore a11y-click-events-have-key-events -->
                              <!-- svelte-ignore a11y-no-static-element-interactions -->
                              <div
                                class="episode-list-item"
                                class:selected={selectedEpisode?.episode_number ===
                                  episode.episode_number}
                                on:click={() => (selectedEpisode = episode)}
                              >
                                <div class="episode-still">
                                  {#if episode.still_path}
                                    <img
                                      src={getImageUrl(
                                        episode.still_path,
                                        "w300",
                                      )}
                                      alt={episode.name}
                                    />
                                  {:else}
                                    <div class="episode-placeholder">
                                      <i class="ri-film-line"></i>
                                    </div>
                                  {/if}
                                  {#if hasProgress}
                                    <span class="progress-badge">{progressText}</span>
                                  {/if}
                                </div>
                                <div class="episode-details">
                                  <div class="episode-top">
                                    <span class="episode-num"
                                      >E{episode.episode_number}</span
                                    >
                                    <span class="episode-name"
                                      >{episode.name}</span
                                    >
                                  </div>
                                  <div class="episode-meta">
                                    {#if episode.vote_average}
                                      <span
                                        class="episode-rating {getRatingClass(
                                          episode.vote_average,
                                        )}"
                                      >
                                        {episode.vote_average.toFixed(1)}
                                      </span>
                                    {/if}
                                    <span class="episode-date"
                                      >{formatDate(episode.air_date)}</span
                                    >
                                    {#if episode.runtime}
                                      <span>{episode.runtime}m</span>
                                    {/if}
                                  </div>
                                  <p class="episode-overview">
                                    {episode.overview}
                                  </p>
                                </div>
                              </div>
                            {/each}
                          </div>
                        {:else}
                          <div class="loading-season">Loading episodes...</div>
                        {/if}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {:else if viewMode === "heatmap"}
              <div class="episodes-heatmap">
                <div class="heatmap-grid">
                  {#each details.seasons.filter((s) => s.season_number > 0) as season}
                    <div class="heatmap-row">
                      <div class="season-label">S{season.season_number}</div>
                      <div class="episodes-row">
                        {#if allSeasonsData[season.season_number]?.episodes}
                          {#each allSeasonsData[season.season_number].episodes as episode}
                            <!-- svelte-ignore a11y-click-events-have-key-events -->
                            <!-- svelte-ignore a11y-no-static-element-interactions -->
                            <div
                              class="heatmap-cell loaded {episode.vote_average
                                ? getRatingClass(episode.vote_average)
                                : 'rating-none'}"
                              on:click={() => {
                                selectedSeason = season.season_number;
                                selectedEpisode = episode;
                              }}
                              data-tooltip="{episode.name} - {episode.vote_average
                                ? episode.vote_average.toFixed(1)
                                : 'N/A'}"
                            >
                              {episode.vote_average
                                ? episode.vote_average.toFixed(1)
                                : "—"}
                            </div>
                          {/each}
                        {:else if season.episode_count}
                          {#each Array(season.episode_count) as _, episodeIndex}
                            <div class="heatmap-cell loading-cell">
                              <div class="loading-spinner"></div>
                            </div>
                          {/each}
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          {/if}

          {#if activeTab === "cast" && credits}
            <div class="cast-grid">
              {#each credits.cast.slice(0, 20) as person}
                <div class="cast-card">
                  {#if person.profile_path}
                    <img
                      src={getImageUrl(person.profile_path, "w185")}
                      alt={person.name}
                    />
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

          {#if activeTab === "details"}
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
                  <span class="value"
                    >{details.original_language.toUpperCase()}</span
                  >
                </div>
              {/if}
              {#if details.vote_count}
                <div class="detail-item">
                  <span class="label">Votes</span>
                  <span class="value"
                    >{details.vote_count.toLocaleString()}</span
                  >
                </div>
              {/if}
              {#if details.production_companies && details.production_companies.length > 0}
                <div class="detail-item full-width">
                  <span class="label">Production</span>
                  <span class="value"
                    >{details.production_companies
                      .map((c) => c.name)
                      .join(", ")}</span
                  >
                </div>
              {/if}
              {#if details.networks && details.networks.length > 0}
                <div class="detail-item full-width">
                  <span class="label">Network</span>
                  <span class="value"
                    >{details.networks.map((n) => n.name).join(", ")}</span
                  >
                </div>
              {/if}
              {#if details.created_by && details.created_by.length > 0}
                <div class="detail-item full-width">
                  <span class="label">Created By</span>
                  <span class="value"
                    >{details.created_by.map((c) => c.name).join(", ")}</span
                  >
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
                <div
                  class="recommendation-card"
                  on:click={() =>
                    window.dispatchEvent(
                      new CustomEvent("openMediaDetail", { detail: rec }),
                    )}
                >
                  {#if rec.poster_path}
                    <img
                      src={getImageUrl(rec.poster_path, "w342")}
                      alt={rec.title || rec.name}
                    />
                  {:else}
                    <div class="poster-placeholder">
                      <i class="ri-film-line"></i>
                    </div>
                  {/if}
                  <div class="recommendation-info">
                    <h4>{rec.title || rec.name}</h4>
                    {#if rec.vote_average}
                      <span
                        class="rec-rating {getRatingClass(rec.vote_average)}"
                      >
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
  <div class="episode-modal-overlay" on:click={() => (selectedEpisode = null)}>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="episode-modal" on:click={(e) => e.stopPropagation()}>
      <div class="episode-modal-header">
        <h3>
          Episode {selectedEpisode.episode_number}: {selectedEpisode.name}
        </h3>
        <div class="episode-modal-actions">
          <button
            class="btn-standard secondary"
            on:click={() => {
              handlePlay(selectedSeason, selectedEpisode.episode_number, true);
              selectedEpisode = null;
            }}
            title="Select a different torrent for this episode"
          >
            <i class="ri-refresh-line"></i> Change Torrent
          </button>
          <button
            class="btn-standard primary"
            on:click={() =>
              handlePlay(selectedSeason, selectedEpisode.episode_number)}
          >
            <i class="ri-play-fill"></i> Play
          </button>
          <button
            class="btn-standard close-modal-btn"
            on:click={() => (selectedEpisode = null)}
          >
            <i class="ri-close-line"></i>
          </button>
        </div>
      </div>

      <div class="episode-modal-content">
        {#if selectedEpisode.still_path}
          <div class="episode-modal-still">
            <img
              src={getImageUrl(selectedEpisode.still_path, "original")}
              alt={selectedEpisode.name}
            />
          </div>
        {/if}

        <div class="episode-modal-meta">
          {#if selectedEpisode.vote_average && selectedEpisode.vote_average > 0}
            <div class="episode-modal-stat">
              <span class="stat-label">Rating</span>
              <div class="stat-value">
                <span
                  class="rating-badge {getRatingClass(
                    selectedEpisode.vote_average,
                  )}"
                >
                  {selectedEpisode.vote_average.toFixed(1)}
                </span>
                {#if selectedEpisode.vote_count}
                  <span class="vote-count">({selectedEpisode.vote_count})</span>
                {/if}
              </div>
            </div>
          {/if}

          <div class="episode-modal-stat">
            <span class="stat-label">Air Date</span>
            <span class="stat-value"
              >{formatDate(selectedEpisode.air_date)}</span
            >
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
          <p>{selectedEpisode.overview || "No overview available."}</p>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showErrorModal}
  <ErrorModal
    {errorMessage}
    title={errorTitle}
    on:close={() => (showErrorModal = false)}
  />
{/if}
{#if showTorrentSelector}
  <TorrentSelector
    searchQuery={currentSearchQuery}
    results={searchResults}
    loading={isSearching}
    on:select={onTorrentSelect}
    on:close={closeTorrentSelector}
    on:research={handleResearch}
  />
{/if}

{#if showFileSelector}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="modal-overlay" on:click={closeFileSelector}>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="file-selector-modal" on:click={(e) => e.stopPropagation()}>
      <div class="file-selector-header">
        <h3>Select Episode File</h3>
        <button class="btn-standard" on:click={closeFileSelector}>
          <i class="ri-close-line"></i>
        </button>
      </div>
      <div class="file-selector-content">
        <p class="file-selector-hint">
          Could not automatically detect the episode file. Please select it manually:
        </p>
        <div class="file-list">
          {#each availableFiles as file}
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div class="file-item" on:click={() => selectManualFile(file)}>
              <i class="ri-file-text-line"></i>
              <span class="file-name">{file.name}</span>
              <span class="file-size">{((file.length || file.size || 0) / 1024 / 1024).toFixed(1)} MB</span>
            </div>
          {/each}
        </div>
      </div>
    </div>
  </div>
{/if}
