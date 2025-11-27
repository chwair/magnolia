import SubtitlesOctopus from '@jellyfin/libass-wasm';

export class SubtitleRenderer {
  constructor(canvas, videoElement) {
    this.videoElement = videoElement;
    this.octopus = null;
  }

  async initialize() {
    if (!this.videoElement) {
      throw new Error('Video element required');
    }

    if (!this.videoElement.parentElement) {
      throw new Error('Video element must be in DOM');
    }

    this.octopus = new SubtitlesOctopus({
      video: this.videoElement,
      subContent: '[Script Info]\nTitle: Default\n\n[V4+ Styles]\nFormat: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,Alignment,MarginL,MarginR,MarginV,Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1\n\n[Events]\nFormat: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text\n',
      fonts: ['/fonts/geist-sans.woff2', '/fonts/.fallback-default.woff2'],
      workerUrl: '/subtitles-octopus-worker.js',
      legacyWorkerUrl: '/subtitles-octopus-worker-legacy.js',
    });

    console.log('SubtitleRenderer initialized');
    return this;
  }

  async loadSubtitleTrack(subtitleData, codec = 'ass') {
    if (!this.octopus) {
      throw new Error('Renderer not initialized');
    }

    console.log(`Loading ${codec} subtitles (${subtitleData.length} bytes)`);
    this.octopus.setTrack(subtitleData);
  }

  show() {
    this.setVisible(true);
  }

  hide() {
    this.setVisible(false);
  }

  setVisible(visible) {
    if (this.octopus?.canvas) {
      this.octopus.canvas.style.display = visible ? 'block' : 'none';
    }
  }

  dispose() {
    if (this.octopus) {
      this.octopus.dispose();
      this.octopus = null;
    }
  }

  isReady() {
    return this.octopus !== null;
  }

  getCanvas() {
    return this.octopus?.canvas || null;
  }
}
