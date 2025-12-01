<script>
  import { onMount, onDestroy } from 'svelte';
  import SubtitlesOctopus from '@jellyfin/libass-wasm';

  let videoElement;
  let octopus = null;
  let status = 'Load video and ASS file';

  async function handleVideoFile(event) {
    const file = event.target.files[0];
    if (file) {
      const url = URL.createObjectURL(file);
      videoElement.src = url;
      status = `Video loaded: ${file.name}`;
    }
  }

  async function handleAssFile(event) {
    const file = event.target.files[0];
    if (file) {
      const text = await file.text();
      
      if (octopus) {
        console.log('Setting track with', text.length, 'bytes');
        console.log('First 200 chars:', text.substring(0, 200));
        
        octopus.setTrack(text);
        status = `Subtitle loaded: ${file.name} (${text.length} bytes)`;
        
        setTimeout(() => {
          const canvas = octopus.canvas;
          console.log('Canvas check:', {
            exists: !!canvas,
            width: canvas?.width,
            height: canvas?.height,
            display: canvas?.style.display,
            visibility: canvas?.style.visibility,
            opacity: canvas?.style.opacity,
            zIndex: canvas?.style.zIndex,
            position: canvas?.style.position,
            top: canvas?.style.top,
            left: canvas?.style.left,
            pointerEvents: canvas?.style.pointerEvents
          });
          
          // Try to get pixel data to see if anything is being drawn
          if (canvas) {
            const ctx = canvas.getContext('2d');
            const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
            let hasPixels = false;
            for (let i = 3; i < imageData.data.length; i += 4) {
              if (imageData.data[i] > 0) {
                hasPixels = true;
                break;
              }
            }
            console.log('Canvas has non-transparent pixels:', hasPixels);
          }
        }, 1000);
      } else {
        status = 'Load video first';
      }
    }
  }

  onMount(() => {
    if (videoElement) {
      videoElement.addEventListener('loadedmetadata', () => {
        octopus = new SubtitlesOctopus({
          video: videoElement,
          subContent: '[Script Info]\nTitle: Default\n\n[V4+ Styles]\nFormat: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,Alignment,MarginL,MarginR,MarginV,Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1\n\n[Events]\nFormat: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text\n',
          fonts: ['/fonts/geist-sans.woff2', '/fonts/.fallback-default.woff2'],
          workerUrl: '/subtitles-octopus-worker.js',
          legacyWorkerUrl: '/subtitles-octopus-worker-legacy.js',
        });
        
        status = 'Ready - load ASS file';
        console.log('SubtitlesOctopus initialized');
      });
    }
  });

  onDestroy(() => {
    if (octopus) {
      octopus.dispose();
    }
  });
</script>

<div>
  <h2>SubtitlesOctopus Debug</h2>
  <p>{status}</p>

  <div>
    <label>Video: <input type="file" accept="video/*" on:change={handleVideoFile} /></label>
  </div>

  <div>
    <label>ASS: <input type="file" accept=".ass,.ssa" on:change={handleAssFile} /></label>
  </div>

  <video
    bind:this={videoElement}
    controls
    preload="metadata"
    style="width: 100%; max-width: 800px; background: black;"
  >
  </video>
</div>
