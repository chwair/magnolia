<script>
  import { createEventDispatcher } from "svelte";
  import { fade, scale } from "svelte/transition";

  export let message = "";
  export let title = "Error";

  const dispatch = createEventDispatcher();

  function close() {
    dispatch("close");
  }

  function handleKeydown(event) {
    if (event.key === "Escape") {
      close();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="modal-overlay" transition:fade on:click={close}>
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="modal-content" transition:scale on:click|stopPropagation>
    <div class="modal-header">
      <i class="ri-error-warning-line error-icon"></i>
      <h3>{title}</h3>
    </div>
    <div class="modal-body">
      <p>{message}</p>
    </div>
    <div class="modal-footer">
      <button class="btn-primary" on:click={close}>
        OK
      </button>
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
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10000;
    backdrop-filter: blur(4px);
  }

  .modal-content {
    background: var(--bg-secondary);
    border-radius: var(--border-radius-lg);
    max-width: 500px;
    width: 90%;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.1);
    overflow: hidden;
  }

  .modal-header {
    padding: var(--spacing-xl);
    background: rgba(239, 68, 68, 0.1);
    border-bottom: 1px solid rgba(239, 68, 68, 0.2);
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
  }

  .error-icon {
    font-size: 28px;
    color: #ef4444;
  }

  .modal-header h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .modal-body {
    padding: var(--spacing-xl);
  }

  .modal-body p {
    margin: 0;
    color: var(--text-secondary);
    line-height: 1.6;
    font-size: 14px;
  }

  .modal-footer {
    padding: var(--spacing-lg) var(--spacing-xl);
    background: rgba(0, 0, 0, 0.2);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: flex-end;
  }

  .btn-primary {
    padding: 10px 24px;
    background: var(--accent-color);
    color: white;
    border: none;
    border-radius: var(--border-radius-sm);
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    font-family: inherit;
  }

  .btn-primary:hover {
    background: var(--accent-hover);
    transform: translateY(-1px);
  }

  .btn-primary:active {
    transform: translateY(0);
  }
</style>
