export class AudioPlayer {
  constructor() {
    this.audioContext = null;
    this.sourceNode = null;
    this.gainNode = null;
    this.analyserNode = null;
    this.currentTrackId = null;
    this.audioQueue = [];
    this.isPlaying = false;
    this.volume = 1.0;
    this.muted = false;
    this.sampleRate = 48000;
    this.channels = 2;
    this.scheduledTime = 0;
    this.startTime = 0;
  }

  async initialize() {
    this.audioContext = new (window.AudioContext || window.webkitAudioContext)();
    
    this.gainNode = this.audioContext.createGain();
    this.analyserNode = this.audioContext.createAnalyser();
    
    this.gainNode.connect(this.analyserNode);
    this.analyserNode.connect(this.audioContext.destination);
    
    this.gainNode.gain.value = this.volume;
    
    return this;
  }

  setTrack(trackId, trackInfo) {
    this.currentTrackId = trackId;
    this.sampleRate = trackInfo.sampleRate || 48000;
    this.channels = trackInfo.channels || 2;
    
    this.audioQueue = [];
    this.scheduledTime = this.audioContext.currentTime;
  }

  async decodeAndScheduleAudio(samples) {
    if (!samples || samples.length === 0) return;

    for (const sample of samples) {
      try {
        const audioData = this.extractAudioData(sample);
        const audioBuffer = await this.audioContext.decodeAudioData(audioData.buffer.slice(0));
        
        this.scheduleAudioBuffer(audioBuffer);
      } catch (error) {
        console.error('Error decoding audio:', error);
      }
    }
  }

  extractAudioData(sample) {
    return new Uint8Array(sample.data);
  }

  scheduleAudioBuffer(audioBuffer) {
    if (!this.isPlaying) return;

    const source = this.audioContext.createBufferSource();
    source.buffer = audioBuffer;
    source.connect(this.gainNode);

    if (this.scheduledTime < this.audioContext.currentTime) {
      this.scheduledTime = this.audioContext.currentTime;
    }

    source.start(this.scheduledTime);
    this.scheduledTime += audioBuffer.duration;

    source.onended = () => {
      source.disconnect();
    };
  }

  play() {
    if (this.audioContext.state === 'suspended') {
      this.audioContext.resume();
    }
    this.isPlaying = true;
    this.startTime = this.audioContext.currentTime;
  }

  pause() {
    this.isPlaying = false;
    if (this.audioContext.state === 'running') {
      this.audioContext.suspend();
    }
  }

  stop() {
    this.isPlaying = false;
    this.audioQueue = [];
    
    if (this.sourceNode) {
      try {
        this.sourceNode.stop();
        this.sourceNode.disconnect();
      } catch (e) {
        console.warn('Error stopping audio source:', e);
      }
      this.sourceNode = null;
    }
    
    this.scheduledTime = this.audioContext.currentTime;
  }

  setVolume(volume) {
    this.volume = Math.max(0, Math.min(1, volume));
    if (this.gainNode) {
      this.gainNode.gain.setValueAtTime(this.muted ? 0 : this.volume, this.audioContext.currentTime);
    }
  }

  setMuted(muted) {
    this.muted = muted;
    if (this.gainNode) {
      this.gainNode.gain.setValueAtTime(muted ? 0 : this.volume, this.audioContext.currentTime);
    }
  }

  getCurrentTime() {
    if (!this.isPlaying) return 0;
    return this.audioContext.currentTime - this.startTime;
  }

  seek(time) {
    this.stop();
    this.scheduledTime = this.audioContext.currentTime;
    this.startTime = this.audioContext.currentTime - time;
    
    if (this.isPlaying) {
      this.play();
    }
  }

  getAnalyserData() {
    if (!this.analyserNode) return null;
    
    const dataArray = new Uint8Array(this.analyserNode.frequencyBinCount);
    this.analyserNode.getByteFrequencyData(dataArray);
    return dataArray;
  }

  dispose() {
    this.stop();
    
    if (this.gainNode) {
      this.gainNode.disconnect();
      this.gainNode = null;
    }
    
    if (this.analyserNode) {
      this.analyserNode.disconnect();
      this.analyserNode = null;
    }
    
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }
  }
}
