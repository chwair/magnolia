import SubtitlesOctopus from 'libass-wasm';

export class SubtitleRenderer {
  constructor(canvas, videoElement) {
    this.canvas = canvas;
    this.videoElement = videoElement;
    this.octopus = null;
    this.currentTrack = null;
    this.subtitleData = null;
  }

  async initialize() {
    if (!this.canvas || !this.videoElement) {
      throw new Error('Canvas and video element are required');
    }

    this.octopus = new SubtitlesOctopus({
      video: this.videoElement,
      canvas: this.canvas,
      workerUrl: new URL('libass-wasm/dist/js/subtitles-octopus-worker.js', import.meta.url).href,
      legacyWorkerUrl: new URL('libass-wasm/dist/js/subtitles-octopus-worker-legacy.js', import.meta.url).href,
      fonts: [],
      availableFonts: {},
      fallbackFont: 'Arial'
    });

    return this;
  }

  async loadSubtitleTrack(subtitleData, codec = 'ass') {
    if (!this.octopus) {
      throw new Error('Subtitle renderer not initialized');
    }

    this.currentTrack = {
      data: subtitleData,
      codec
    };

    if (codec === 'ass' || codec === 'ssa') {
      this.octopus.setTrack(subtitleData);
    } else if (codec === 'srt') {
      const assData = this.convertSrtToAss(subtitleData);
      this.octopus.setTrack(assData);
    } else {
      console.warn(`Unsupported subtitle codec: ${codec}`);
    }
  }

  convertSrtToAss(srtData) {
    const assHeader = `[Script Info]
Title: Converted from SRT
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000088EF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
`;

    const lines = srtData.split('\n');
    let assEvents = '';
    let i = 0;

    while (i < lines.length) {
      if (lines[i].match(/^\d+$/)) {
        const timeLine = lines[i + 1];
        if (timeLine && timeLine.includes('-->')) {
          const [start, end] = timeLine.split('-->').map(t => this.srtTimeToAss(t.trim()));
          
          let text = '';
          i += 2;
          while (i < lines.length && lines[i].trim() !== '') {
            text += lines[i].replace(/<[^>]*>/g, '') + '\\N';
            i++;
          }
          
          text = text.slice(0, -2);
          
          if (text) {
            assEvents += `Dialogue: 0,${start},${end},Default,,0,0,0,,${text}\n`;
          }
        }
      }
      i++;
    }

    return assHeader + assEvents;
  }

  srtTimeToAss(srtTime) {
    const match = srtTime.match(/(\d{2}):(\d{2}):(\d{2}),(\d{3})/);
    if (!match) return '0:00:00.00';
    
    const [, h, m, s, ms] = match;
    const centiseconds = Math.floor(parseInt(ms) / 10);
    return `${h}:${m}:${s}.${centiseconds.toString().padStart(2, '0')}`;
  }

  updateTime(currentTime) {
    if (this.octopus) {
      this.octopus.setCurrentTime(currentTime);
    }
  }

  resize(width, height) {
    if (this.octopus && this.canvas) {
      this.canvas.width = width;
      this.canvas.height = height;
      this.octopus.resize(width, height);
    }
  }

  show() {
    if (this.canvas) {
      this.canvas.style.display = 'block';
    }
  }

  hide() {
    if (this.canvas) {
      this.canvas.style.display = 'none';
    }
  }

  dispose() {
    if (this.octopus) {
      this.octopus.dispose();
      this.octopus = null;
    }
    this.currentTrack = null;
  }
}
