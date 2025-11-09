<script>
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import SearchBar from './SearchBar.svelte';

  export let searchActive = false;
  export let accentColor = null;

  const isMacOS = navigator.userAgent.includes('Mac');
  const appWindow = getCurrentWindow();

  async function minimizeWindow() {
    await appWindow.minimize();
  }

  async function maximizeWindow() {
    await appWindow.toggleMaximize();
  }

  async function closeWindow() {
    await appWindow.close();
  }

  function hexToRgb(hex) {
    if (!hex) return '255, 255, 255';
    hex = hex.replace('#', '');
    const r = parseInt(hex.substring(0, 2), 16);
    const g = parseInt(hex.substring(2, 4), 16);
    const b = parseInt(hex.substring(4, 6), 16);
    return `${r}, ${g}, ${b}`;
  }
</script>

<div class="titlebar" data-tauri-drag-region style={accentColor ? `--dynamic-accent: ${accentColor}; background: linear-gradient(135deg, rgba(${hexToRgb(accentColor)}, 0.35), rgba(10, 10, 10, 0.95));` : ''}>
  {#if isMacOS}
    <div class="titlebar-controls macos">
      <button class="titlebar-button close" on:click={closeWindow} aria-label="Close">
        <div class="dot"></div>
      </button>
      <button class="titlebar-button minimize" on:click={minimizeWindow} aria-label="Minimize">
        <div class="dot"></div>
      </button>
      <button class="titlebar-button maximize" on:click={maximizeWindow} aria-label="Maximize">
        <div class="dot"></div>
      </button>
    </div>
    <div class="titlebar-center">
      <div class="logo"></div>
      <SearchBar bind:searchActive />
    </div>
  {:else}
    <div class="titlebar-left">
      <div class="logo"></div>
      <SearchBar bind:searchActive />
    </div>
    <div class="titlebar-controls windows">
      <button class="titlebar-button minimize" on:click={minimizeWindow} aria-label="Minimize">
        <i class="ri-subtract-line"></i>
      </button>
      <button class="titlebar-button maximize" on:click={maximizeWindow} aria-label="Maximize">
        <i class="ri-stop-line"></i>
      </button>
      <button class="titlebar-button close" on:click={closeWindow} aria-label="Close">
        <i class="ri-close-line"></i>
      </button>
    </div>
  {/if}
</div>

<style>
  .titlebar {
    height: var(--titlebar-height);
    background: rgba(10, 10, 10, 0.95);
    display: flex;
    align-items: center;
    justify-content: space-between;
    user-select: none;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px 8px 0 0;
    padding: 0 12px;
    gap: 12px;
    box-shadow: 
      inset -2px -2px 4px rgba(255, 255, 255, 0.02),
      inset 2px 2px 4px rgba(0, 0, 0, 0.2);
    z-index: 10000;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    transition: background 0.5s ease, border-color 0.5s ease;
  }

  .titlebar-left,
  .titlebar-center {
    display: flex;
    align-items: center;
    gap: 32px;
    flex: 1;
  }

  .titlebar-controls {
    display: flex;
    align-items: center;
    height: 100%;
  }

  .titlebar-controls.macos {
    gap: 8px;
  }

  .titlebar-controls.windows {
    margin-left: auto;
  }

  .titlebar-button {
    border: none;
    background: transparent;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.7);
    transition: background-color 0.2s, color 0.2s;
    padding: 0;
  }

  .titlebar-controls.macos .titlebar-button {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    padding: 0;
  }

  .titlebar-controls.macos .titlebar-button .dot {
    width: 100%;
    height: 100%;
    border-radius: 50%;
  }

  .titlebar-controls.macos .titlebar-button.close .dot {
    background-color: #ff5f56;
  }

  .titlebar-controls.macos .titlebar-button.minimize .dot {
    background-color: #ffbd2e;
  }

  .titlebar-controls.macos .titlebar-button.maximize .dot {
    background-color: #27c93f;
  }

  .titlebar-controls.macos .titlebar-button:hover {
    opacity: 0.8;
  }

  .titlebar-controls.windows .titlebar-button {
    width: var(--titlebar-height);
    height: var(--titlebar-height);
    font-size: 18px;
  }

  .titlebar-controls.windows .titlebar-button:hover {
    background-color: rgba(255, 255, 255, 0.1);
  }

  .titlebar-controls.windows .titlebar-button.close:hover {
    background-color: #e81123;
    color: white;
  }
</style>
