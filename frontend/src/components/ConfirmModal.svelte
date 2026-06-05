<script lang="ts">
  import { onMount } from 'svelte';
  import { AlertTriangle, Trash2, X } from '@lucide/svelte';

  let {
    show = false,
    title = 'Confirm Action',
    message = 'Are you sure you want to proceed?',
    confirmText = 'Confirm',
    cancelText = 'Cancel',
    isDanger = false,
    onConfirm,
    onCancel
  } = $props<{
    show: boolean;
    title?: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    isDanger?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  }>();

  // Handle Escape key to close modal
  function handleKeyDown(event: KeyboardEvent) {
    if (show && event.key === 'Escape') {
      onCancel();
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  });
</script>

{#if show}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={onCancel}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal-content glass-card" onclick={(e) => e.stopPropagation()}>
      <button class="close-btn" onclick={onCancel} aria-label="Close modal">
        <X size={18} />
      </button>

      <div class="modal-header">
        <div class="icon-container" class:danger={isDanger}>
          {#if isDanger}
            <Trash2 size={24} />
          {:else}
            <AlertTriangle size={24} />
          {/if}
        </div>
        <h2 class="modal-title">{title}</h2>
      </div>

      <div class="modal-body">
        <p class="modal-message">{message}</p>
      </div>

      <div class="modal-actions">
        <button type="button" class="btn btn-secondary" onclick={onCancel}>
          {cancelText}
        </button>
        <button 
          type="button" 
          class="btn" 
          class:btn-danger={isDanger} 
          class:btn-primary={!isDanger}
          onclick={onConfirm}
        >
          {confirmText}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .modal-content {
    position: relative;
    width: 90%;
    max-width: 440px;
    padding: 2.25rem 2rem 2rem;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(18, 18, 22, 0.85);
    border-radius: 16px;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.6),
                0 0 25px 0 rgba(255, 255, 255, 0.02);
    animation: scaleIn 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @keyframes scaleIn {
    from {
      transform: scale(0.95);
      opacity: 0;
    }
    to {
      transform: scale(1);
      opacity: 1;
    }
  }

  .close-btn {
    position: absolute;
    top: 1.25rem;
    right: 1.25rem;
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.25rem;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.05);
  }

  .modal-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 1rem;
    margin-bottom: 1.25rem;
  }

  .icon-container {
    width: 52px;
    height: 52px;
    border-radius: 50%;
    background: rgba(245, 158, 11, 0.1);
    color: #f59e0b;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 0 15px rgba(245, 158, 11, 0.1);
  }

  .icon-container.danger {
    background: rgba(239, 68, 68, 0.1);
    color: #f87171;
    box-shadow: 0 0 15px rgba(239, 68, 68, 0.1);
  }

  .modal-title {
    font-family: var(--font-heading);
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
  }

  .modal-body {
    text-align: center;
    margin-bottom: 2rem;
  }

  .modal-message {
    font-size: 0.95rem;
    line-height: 1.5;
    color: var(--text-secondary);
    margin: 0;
  }

  .modal-actions {
    display: flex;
    gap: 0.75rem;
    justify-content: center;
  }

  .modal-actions button {
    flex: 1;
    max-width: 160px;
    padding: 0.65rem 1rem;
    font-size: 0.9rem;
    font-weight: 600;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn-danger {
    background: #ef4444;
    color: white;
    border: none;
  }

  .btn-danger:hover {
    background: #dc2626;
    box-shadow: 0 0 12px rgba(239, 68, 68, 0.4);
  }
</style>
