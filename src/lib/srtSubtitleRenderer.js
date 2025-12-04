export class SRTSubtitleRenderer {
  constructor(videoElement) {
    this.videoElement = videoElement;
    this.subtitles = [];
    this.currentCue = null;
    this.container = null;
    this.isVisible = true;
  }

  initialize() {
    // Create subtitle container overlay
    this.container = document.createElement('div');
    this.container.className = 'srt-subtitle-overlay';
    this.container.style.cssText = `
      position: absolute;
      bottom: 60px;
      left: 0;
      right: 0;
      text-align: center;
      pointer-events: none;
      z-index: 10;
      padding: 0 20px;
    `;
    
    const playerContainer = this.videoElement.closest('.video-player');
    if (playerContainer) {
      playerContainer.appendChild(this.container);
    }

    // Update subtitles on timeupdate
    this.videoElement.addEventListener('timeupdate', () => this.updateSubtitles());
  }

  parseSRT(srtText) {
    const lines = srtText.trim().split('\n');
    const subtitles = [];
    let current = {};
    
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      
      // Skip subtitle index numbers
      if (/^\d+$/.test(line)) {
        current = {};
        continue;
      }
      
      // Parse timestamp line
      if (line.includes('-->')) {
        const [start, end] = line.split('-->').map(t => this.parseTimestamp(t.trim()));
        current.start = start;
        current.end = end;
        current.text = '';
        continue;
      }
      
      // Parse subtitle text
      if (line && current.start !== undefined) {
        current.text += (current.text ? '\n' : '') + line;
      }
      
      // Empty line marks end of subtitle block
      if (!line && current.start !== undefined) {
        subtitles.push(current);
        current = {};
      }
    }
    
    // Add last subtitle if exists
    if (current.start !== undefined) {
      subtitles.push(current);
    }
    
    this.subtitles = subtitles;
    console.log('Parsed SRT subtitles:', this.subtitles.length);
  }

  parseTimestamp(timeStr) {
    // Parse timestamp like "00:00:20,000" or "00:00:20.000"
    const parts = timeStr.replace(',', '.').split(':');
    const hours = parseInt(parts[0]);
    const minutes = parseInt(parts[1]);
    const secondsParts = parts[2].split('.');
    const seconds = parseInt(secondsParts[0]);
    const milliseconds = parseInt(secondsParts[1] || 0);
    
    return hours * 3600 + minutes * 60 + seconds + milliseconds / 1000;
  }

  updateSubtitles() {
    if (!this.isVisible || !this.container) return;
    
    const currentTime = this.videoElement.currentTime;
    const activeSub = this.subtitles.find(
      sub => currentTime >= sub.start && currentTime <= sub.end
    );
    
    if (activeSub && activeSub !== this.currentCue) {
      this.currentCue = activeSub;
      this.displaySubtitle(activeSub.text);
    } else if (!activeSub && this.currentCue) {
      this.currentCue = null;
      this.hideSubtitle();
    }
  }

  displaySubtitle(text) {
    if (!this.container) return;
    
    // Parse position tags
    let positionStyle = '';
    const posMatch = text.match(/X1:(\d+)\s*X2:(\d+)\s*Y1:(\d+)\s*Y2:(\d+)/);
    if (posMatch) {
      const [, x1, x2, y1, y2] = posMatch.map(Number);
      // Remove position tags from text
      text = text.replace(/X1:\d+\s*X2:\d+\s*Y1:\d+\s*Y2:\d+/g, '').trim();
      
      // Calculate position (assuming video is 100% width/height)
      const leftPercent = (x1 / 1920) * 100; // Assuming 1920x1080 reference
      const topPercent = (y1 / 1080) * 100;
      positionStyle = `left: ${leftPercent}%; top: ${topPercent}%; bottom: auto; transform: none;`;
    }
    
    // Convert newlines to <br> and format SRT tags
    const htmlText = text
      .split('\n')
      .map(line => this.formatSRT(line))
      .join('<br>');
    
    this.container.innerHTML = `
      <div style="
        display: inline-block;
        background: rgba(0, 0, 0, 0.8);
        padding: 8px 16px;
        border-radius: 4px;
        font-size: 18px;
        font-weight: 500;
        color: white;
        text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.9);
        line-height: 1.4;
        max-width: 80%;
        ${positionStyle}
      ">
        ${htmlText}
      </div>
    `;
  }

  hideSubtitle() {
    if (this.container) {
      this.container.innerHTML = '';
    }
  }

  formatSRT(text) {
    // Position tags (X1, Y1, X2, Y2) are handled in displaySubtitle
    
    // Convert {i} to <i> (alternate italic format)
    text = text.replace(/\{i\}/g, '<i>').replace(/\{\/i\}/g, '</i>');
    text = text.replace(/\{b\}/g, '<b>').replace(/\{\/b\}/g, '</b>');
    text = text.replace(/\{u\}/g, '<u>').replace(/\{\/u\}/g, '</u>');
    
    // Support <font color=#xxxxxx> tags
    text = text.replace(/<font color="?(#[0-9a-fA-F]{6})"?>/gi, (match, color) => {
      return `<span style="color: ${color}">`;
    });
    text = text.replace(/<\/font>/gi, '</span>');
    
    // Ensure standard tags are preserved: <i>, <b>, <u>
    // These are already HTML tags, so they'll work as-is
    
    // Escape other potentially dangerous characters while preserving our formatting tags
    const allowedTags = /<\/?(?:i|b|u|span[^>]*)>/gi;
    const parts = text.split(allowedTags);
    const tags = text.match(allowedTags) || [];
    
    // Escape text parts but not tags
    let result = '';
    for (let i = 0; i < parts.length; i++) {
      if (parts[i]) {
        // Escape the text
        result += parts[i]
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;');
      }
      if (tags[i]) {
        // Add the tag as-is
        result += tags[i];
      }
    }
    
    return result;
  }

  show() {
    this.isVisible = true;
    if (!this.container) {
      this.initialize();
    }
    if (this.container) {
      this.container.style.display = 'block';
      // Force update to show current subtitle if any
      this.updateSubtitles();
    }
  }

  hide() {
    this.isVisible = false;
    this.hideSubtitle();
    if (this.container) {
      this.container.style.display = 'none';
    }
  }

  dispose() {
    if (this.container && this.container.parentNode) {
      this.container.parentNode.removeChild(this.container);
    }
    this.subtitles = [];
    this.currentCue = null;
    this.container = null;
  }
}
