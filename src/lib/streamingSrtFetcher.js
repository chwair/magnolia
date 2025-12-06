/**
 * StreamingSrtFetcher - Progressively fetches and caches SRT subtitles
 * Fetches only the subtitle segments relevant to the current playhead position
 * and caches them to disk periodically
 */
export class StreamingSrtFetcher {
  constructor(url, cacheId, fileIndex, trackIndex, cacheCallbacks = {}) {
    this.url = url;
    this.cacheId = cacheId;
    this.fileIndex = fileIndex;
    this.trackIndex = trackIndex;
    
    // Cache callbacks: { load, save }
    this.loadCache = cacheCallbacks.load || (() => null);
    this.saveCache = cacheCallbacks.save || (() => {});
    
    this.fullContent = null;
    this.subtitles = [];
    this.isFetching = false;
    this.fetchProgress = 0;
    this.lastCachedTime = 0;
    this.cacheInterval = 30000; // Cache every 30 seconds
    this.cacheableSegments = []; // Segments that should be cached
    
    this.lastPlayheadTime = 0;
  }

  /**
   * Parse SRT subtitle from text
   */
  parseSRT(srtText) {
    const lines = srtText.trim().split('\n');
    const subtitles = [];
    let current = {};
    
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      
      if (/^\d+$/.test(line)) {
        current = {};
        continue;
      }
      
      if (line.includes('-->')) {
        const [start, end] = line.split('-->').map(t => this.parseTimestamp(t.trim()));
        current.start = start;
        current.end = end;
        current.text = '';
        continue;
      }
      
      if (line && current.start !== undefined) {
        current.text += (current.text ? '\n' : '') + line;
      }
      
      if (!line && current.start !== undefined) {
        subtitles.push(current);
        current = {};
      }
    }
    
    if (current.start !== undefined) {
      subtitles.push(current);
    }
    
    this.subtitles = subtitles;
    return subtitles;
  }

  parseTimestamp(timeStr) {
    const parts = timeStr.replace(',', '.').split(':');
    const hours = parseInt(parts[0]);
    const minutes = parseInt(parts[1]);
    const secondsParts = parts[2].split('.');
    const seconds = parseInt(secondsParts[0]);
    const milliseconds = parseInt(secondsParts[1] || 0);
    
    return hours * 3600 + minutes * 60 + seconds + milliseconds / 1000;
  }

  /**
   * Initialize: check cache first, then fetch if needed
   */
  async initialize() {
    console.log(`[StreamingSRT] Initializing for ${this.url}`);
    
    try {
      // Try to load from cache first
      const cached = await this.loadCache(this.cacheId, this.fileIndex, this.trackIndex);
      if (cached) {
        console.log(`[StreamingSRT] Loaded full content from cache (${cached.length} bytes)`);
        this.fullContent = cached;
        this.parseSRT(cached);
        return this.subtitles;
      }
    } catch (error) {
      console.warn(`[StreamingSRT] Failed to load from cache:`, error);
    }

    // Not in cache, fetch it
    console.log(`[StreamingSRT] Fetching from URL...`);
    return this.fetch();
  }

  /**
   * Fetch the full subtitle file
   */
  async fetch() {
    if (this.isFetching || this.fullContent) return this.subtitles;
    
    this.isFetching = true;
    this.fetchProgress = 0;

    try {
      const response = await fetch(this.url);
      if (!response.ok) {
        throw new Error(`Failed to fetch: ${response.statusText}`);
      }

      const text = await response.text();
      this.fullContent = text;
      this.parseSRT(text);
      
      console.log(`[StreamingSRT] Fetched ${this.fullContent.length} bytes, ${this.subtitles.length} subtitles`);
      
      // Cache to disk asynchronously
      this.cacheAsync(text);
      
      this.fetchProgress = 100;
      return this.subtitles;
    } catch (error) {
      console.error(`[StreamingSRT] Fetch failed:`, error);
      throw error;
    } finally {
      this.isFetching = false;
    }
  }

  /**
   * Cache content to disk asynchronously
   */
  async cacheAsync(content) {
    try {
      await this.saveCache(this.cacheId, this.fileIndex, this.trackIndex, content);
      console.log(`[StreamingSRT] Cached to disk (${content.length} bytes)`);
      this.lastCachedTime = Date.now();
    } catch (error) {
      console.warn(`[StreamingSRT] Failed to cache:`, error);
    }
  }

  /**
   * Get subtitle at current playhead time
   * Returns the subtitle text that should be displayed at this time
   */
  getSubtitleAtTime(currentTime) {
    if (!this.subtitles.length) return null;
    
    const activeSub = this.subtitles.find(
      sub => currentTime >= sub.start && currentTime <= sub.end
    );
    
    return activeSub?.text || null;
  }

  /**
   * Get all subtitles for a time range
   * Used for prefetching subtitles ahead of playhead
   */
  getSubtitlesInRange(startTime, endTime) {
    return this.subtitles.filter(
      sub => sub.end >= startTime && sub.start <= endTime
    );
  }

  /**
   * Get cached content as string
   */
  getContent() {
    return this.fullContent;
  }

  /**
   * Get all parsed subtitles
   */
  getSubtitles() {
    return this.subtitles;
  }

  /**
   * Check if fetch is in progress
   */
  isFetching() {
    return this.isFetching;
  }

  /**
   * Get fetch progress (0-100)
   */
  getFetchProgress() {
    return this.fetchProgress;
  }
}
