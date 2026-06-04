<script lang="ts">
  import { Upload, FileMusic, CheckCircle2, AlertCircle, RefreshCw } from '@lucide/svelte';

  let { token, addToast } = $props<{
    token: string;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  interface UploadItem {
    id: number;
    file: File;
    progress: number;
    status: 'pending' | 'uploading' | 'success' | 'error' | 'duplicate';
    errorMsg?: string;
  }

  let queue = $state<UploadItem[]>([]);
  let isDragging = $state(false);
  let isUploading = $state(false);
  let fileInputRef = $state<HTMLInputElement | null>(null);

  let nextId = 0;

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

  function addFilesToQueue(fileList: FileList) {
    const validExtensions = ['.mp3', '.flac', '.ogg', '.wav', '.m4a'];
    const newItems: UploadItem[] = [];

    for (let i = 0; i < fileList.length; i++) {
      const file = fileList[i];
      const ext = '.' + file.name.split('.').pop()?.toLowerCase();
      
      if (!validExtensions.includes(ext)) {
        addToast(`Unsupported file type: ${file.name}`, 'error');
        continue;
      }

      newItems.push({
        id: nextId++,
        file,
        progress: 0,
        status: 'pending'
      });
    }

    if (newItems.length > 0) {
      queue = [...queue, ...newItems];
      processQueue();
    }
  }

  async function processQueue() {
    if (isUploading) return;
    
    const pendingItem = queue.find(item => item.status === 'pending');
    if (!pendingItem) {
      isUploading = false;
      return;
    }

    isUploading = true;
    pendingItem.status = 'uploading';

    try {
      await uploadFile(pendingItem);
    } catch (err) {
      // Handled inside uploadFile
    }

    isUploading = false;
    processQueue(); // Process next in line
  }

  function uploadFile(item: UploadItem): Promise<void> {
    return new Promise((resolve) => {
      const formData = new FormData();
      formData.append('file', item.file);

      const xhr = new XMLHttpRequest();
      xhr.open('POST', '/api/tracks');
      xhr.setRequestHeader('Authorization', `Bearer ${token}`);

      xhr.upload.addEventListener('progress', (e) => {
        if (e.lengthComputable) {
          const percent = Math.round((e.loaded / e.total) * 100);
          item.progress = percent;
        }
      });

      xhr.onload = () => {
        if (xhr.status === 201) {
          item.status = 'success';
          item.progress = 100;
          addToast(`Uploaded: ${item.file.name}`, 'success');
        } else if (xhr.status === 409) {
          item.status = 'duplicate';
          item.progress = 100;
          item.errorMsg = 'Duplicate track';
          addToast(`Duplicate skipped: ${item.file.name}`, 'info');
        } else {
          item.status = 'error';
          item.errorMsg = xhr.responseText || `Error ${xhr.status}`;
          addToast(`Failed: ${item.file.name}`, 'error');
        }
        resolve();
      };

      xhr.onerror = () => {
        item.status = 'error';
        item.errorMsg = 'Network error';
        addToast(`Network error: ${item.file.name}`, 'error');
        resolve();
      };

      xhr.send(formData);
    });
  }

  function clearCompleted() {
    queue = queue.filter(item => item.status === 'pending' || item.status === 'uploading');
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

  {#if queue.length > 0}
    <div style="margin-top: 2rem;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
        <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600;">Upload Queue ({queue.length})</h3>
        <button onclick={clearCompleted} class="btn btn-secondary" style="padding: 0.4rem 0.8rem; font-size: 0.8rem;">
          Clear Completed
        </button>
      </div>

      <div class="progress-list">
        {#each queue as item (item.id)}
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
