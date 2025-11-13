import { WebDemuxer } from 'web-demuxer';

export class MKVDemuxer {
  constructor() {
    this.demuxer = null;
    this.mediaInfo = null;
    this.videoTrack = null;
    this.audioTracks = [];
    this.subtitleTracks = [];
    this.chapters = [];
    this.onReady = null;
    this.onVideoSamples = null;
    this.onAudioSamples = null;
    this.onSubtitleData = null;
    this.videoStream = null;
    this.audioStreams = new Map();
  }

  async initialize(src) {
    try {
      // Use absolute URL for WASM file so it works in worker context
      const wasmUrl = new URL('/web-demuxer.wasm', window.location.origin).href;
      console.log('WASM URL:', wasmUrl);
      
      this.demuxer = new WebDemuxer({
        wasmFilePath: wasmUrl
      });

      await this.demuxer.load(src);
      
      this.mediaInfo = await this.demuxer.getMediaInfo();
      console.log('Web-Demuxer Media Info:', this.mediaInfo);
      
      await this.processMediaInfo();
      
      const info = {
        duration: this.mediaInfo.duration,
        videoTrack: this.videoTrack,
        audioTracks: this.audioTracks,
        subtitleTracks: this.subtitleTracks,
        chapters: this.chapters
      };
      
      if (this.onReady) {
        this.onReady(info);
      }
      
      return info;
    } catch (error) {
      console.error('Failed to initialize web-demuxer:', error);
      throw error;
    }
  }

  async processMediaInfo() {
    if (this.mediaInfo.videoStreams && this.mediaInfo.videoStreams.length > 0) {
      const videoStream = this.mediaInfo.videoStreams[0];
      this.videoTrack = {
        id: 0,
        codec: videoStream.codecName,
        width: videoStream.width,
        height: videoStream.height,
        duration: this.mediaInfo.duration
      };
    }

    if (this.mediaInfo.audioStreams && this.mediaInfo.audioStreams.length > 0) {
      this.audioTracks = this.mediaInfo.audioStreams.map((stream, index) => ({
        id: index,
        codec: stream.codecName,
        sampleRate: stream.sampleRate || 48000,
        channels: stream.channels || 2,
        language: stream.metadata?.language || 'und',
        name: stream.metadata?.title || `Audio Track ${index + 1}`
      }));
    }

    if (this.mediaInfo.subtitleStreams && this.mediaInfo.subtitleStreams.length > 0) {
      this.subtitleTracks = this.mediaInfo.subtitleStreams.map((stream, index) => ({
        id: index,
        codec: stream.codecName,
        language: stream.metadata?.language || 'und',
        name: stream.metadata?.title || `Subtitle Track ${index + 1}`
      }));
    }

    if (this.mediaInfo.chapters && this.mediaInfo.chapters.length > 0) {
      this.chapters = this.mediaInfo.chapters.map((chapter, index) => ({
        index,
        start_time: chapter.startTime,
        end_time: chapter.endTime,
        title: chapter.metadata?.title || `Chapter ${index + 1}`
      }));
    }
  }

  async startExtracting() {
    if (!this.demuxer) return;

    if (this.videoTrack) {
      this.videoStream = this.demuxer.read('video', 0);
      this.readVideoStream();
    }

    if (this.audioTracks.length > 0) {
      this.audioTracks.forEach((track, index) => {
        const stream = this.demuxer.read('audio', 0, undefined, undefined);
        this.audioStreams.set(track.id, stream);
        this.readAudioStream(track.id, stream);
      });
    }
  }

  async readVideoStream() {
    if (!this.videoStream) return;

    try {
      const reader = this.videoStream.getReader();
      
      while (true) {
        const { done, value } = await reader.read();
        
        if (done) break;
        
        if (this.onVideoSamples) {
          this.onVideoSamples([value]);
        }
      }
    } catch (error) {
      console.error('Error reading video stream:', error);
    }
  }

  async readAudioStream(trackId, stream) {
    if (!stream) return;

    try {
      const reader = stream.getReader();
      
      while (true) {
        const { done, value } = await reader.read();
        
        if (done) break;
        
        if (this.onAudioSamples) {
          this.onAudioSamples(trackId, [value]);
        }
      }
    } catch (error) {
      console.error('Error reading audio stream:', error);
    }
  }

  stop() {
    if (this.videoStream) {
      try {
        this.videoStream.cancel();
      } catch (e) {
        console.warn('Error canceling video stream:', e);
      }
      this.videoStream = null;
    }

    this.audioStreams.forEach(stream => {
      try {
        stream.cancel();
      } catch (e) {
        console.warn('Error canceling audio stream:', e);
      }
    });
    this.audioStreams.clear();
  }

  async seek(time) {
    if (!this.demuxer) return;
    
    this.stop();
    
    if (this.videoTrack) {
      this.videoStream = this.demuxer.read('video', time);
      this.readVideoStream();
    }

    if (this.audioTracks.length > 0) {
      this.audioTracks.forEach((track) => {
        const stream = this.demuxer.read('audio', time);
        this.audioStreams.set(track.id, stream);
        this.readAudioStream(track.id, stream);
      });
    }
  }

  destroy() {
    this.stop();
    
    if (this.demuxer) {
      this.demuxer.destroy();
      this.demuxer = null;
    }
  }
}
