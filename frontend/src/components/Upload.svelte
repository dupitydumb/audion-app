<script lang="ts">
  import { Upload, FileMusic, CheckCircle2, AlertCircle, RefreshCw } from '@lucide/svelte';
  import { addFilesToQueue, clearCompleted, summarizeQueue, uploadQueue } from '../stores/uploadQueue';

  let isDragging = $state(false);
  let fileInputRef = $state<HTMLInputElement | null>(null);

  let queueSummary = $state({
    total: 0,
    pending: 0,
    uploading: 0,
    success: 0,
    error: 0,
    duplicate: 0
  });

  $: queueSummary = summarizeQueue($uploadQueue);

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    isDragging = true;
  }

  function handleDragLeave() {
    isDragging = false;
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragging = false;
    if (e.dataTransfer?.files) {
      addFilesToQueue(e.dataTransfer.files);
    }
  }

  function handleFileSelect(e: Event) {
    const target = e.target as HTMLInputElement;
    if (target.files) {
      addFilesToQueue(target.files);
    }
  }

  function triggerFileInput() {
    fileInputRef?.click();
  }
</script>

<div class="page-header">
  <h1 class="page-title">Upload Music</h1>
  <p class="page-subtitle">Drag-and-drop or browse audio files to import them directly into the server library.</p>
</div>

<div class="glass-card">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div 
    class="dropzone {isDragging ? 'dragover' : ''}" 
    onclick={triggerFileInput}
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
  >
    <div class="dropzone-icon">
      <Upload size={28} />
    </div>
    <div>
      <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600; margin-bottom: 0.25rem;">
        Drag & Drop your music here
      </h3>
      <p style="color: var(--text-secondary); font-size: 0.9rem;">
        or click to browse from your device
      </p>
    </div>
    <span style="font-size: 0.8rem; color: var(--text-muted);">
      Supported formats: MP3, FLAC, OGG, WAV, M4A
    </span>
    <input 
      type="file" 
      bind:this={fileInputRef} 
      onchange={handleFileSelect} 
      multiple 
      accept=".mp3,.flac,.ogg,.wav,.m4a" 
      style="display: none;" 
    />
  </div>

  {#if $uploadQueue.length > 0}
    <div style="margin-top: 2rem;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; gap: 1rem; flex-wrap: wrap;">
        <div style="display: flex; flex-direction: column; gap: 0.25rem;">
          <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600;">Upload Queue ({queueSummary.total})</h3>
          <div style="font-size: 0.85rem; color: var(--text-secondary);">
            In progress: {queueSummary.uploading} · Pending: {queueSummary.pending} · Completed: {queueSummary.success + queueSummary.duplicate} · Failed: {queueSummary.error}
          </div>
        </div>
        <button onclick={clearCompleted} class="btn btn-secondary" style="padding: 0.4rem 0.8rem; font-size: 0.8rem;">
          Clear Completed
        </button>
      </div>

      <div class="progress-list">
        {#each $uploadQueue as item (item.id)}
          <div class="progress-item">
            <div class="progress-item-header">
              <div style="display: flex; align-items: center; gap: 0.5rem; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; max-width: 70%;">
                <FileMusic size={16} style="color: var(--accent); flex-shrink: 0;" />
                <span style="font-weight: 500; font-size: 0.9rem; overflow: hidden; text-overflow: ellipsis;">{item.file.name}</span>
              </div>
              
              <div style="display: flex; align-items: center; gap: 0.5rem; font-size: 0.85rem; font-weight: 600;">
                {#if item.status === 'pending'}
                  <span style="color: var(--text-muted);">Pending</span>
                {:else if item.status === 'uploading'}
                  <span style="color: var(--accent); display: flex; align-items: center; gap: 0.25rem;">
                    <RefreshCw size={12} class="animate-spin" style="animation: spin 1s linear infinite;" />
                    {item.progress}%
                  </span>
                {:else if item.status === 'success'}
                  <span style="color: var(--success); display: flex; align-items: center; gap: 0.25rem;">
                    <CheckCircle2 size={14} /> Completed
                  </span>
                {:else if item.status === 'duplicate'}
                  <span style="color: var(--text-secondary); display: flex; align-items: center; gap: 0.25rem;">
                    <AlertCircle size={14} /> Duplicate Skipped
                  </span>
                {:else if item.status === 'error'}
                  <span style="color: var(--danger); display: flex; align-items: center; gap: 0.25rem;" title={item.errorMsg}>
                    <AlertCircle size={14} /> Failed
                  </span>
                {/if}
              </div>
            </div>
            
            <div class="progress-bar-container">
              <div 
                class="progress-bar" 
                style="width: {item.progress}%; background: {item.status === 'error' ? 'var(--danger)' : item.status === 'duplicate' ? 'var(--text-muted)' : 'var(--accent-gradient)'}"
              ></div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
