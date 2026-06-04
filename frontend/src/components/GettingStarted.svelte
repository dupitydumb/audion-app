<script lang="ts">
  import { Laptop, Settings, Link, ShieldCheck, Check, Copy } from '@lucide/svelte';

  let { addToast } = $props<{
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  let copiedUrl = $state(false);
  const serverUrl = typeof window !== 'undefined' ? window.location.origin : 'http://localhost:8080';

  function copyUrl() {
    navigator.clipboard.writeText(serverUrl);
    addToast('Server URL copied to clipboard', 'success');
    copiedUrl = true;
    setTimeout(() => copiedUrl = false, 2000);
  }
</script>

<div class="page-header">
  <h1 class="page-title">Getting Started</h1>
  <p class="page-subtitle">Learn how to connect your Audion desktop application to this server.</p>
</div>

<div style="display: flex; flex-direction: column; gap: 1.5rem;">
  <div class="glass-card" style="display: flex; flex-direction: column; gap: 1rem; align-items: flex-start;">
    <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600; margin-bottom: 0.25rem;">Connection URL</h3>
    <p style="font-size: 0.9rem; color: var(--text-secondary); line-height: 1.4;">
      You will need this URL inside your Audion desktop application to establish a connection.
    </p>
    <div style="display: flex; gap: 0.75rem; width: 100%;">
      <input type="text" class="form-input" readonly value={serverUrl} style="flex: 1; font-family: monospace;" />
      <button onclick={copyUrl} class="btn btn-primary" style="padding: 0 1.25rem; display: flex; gap: 0.5rem; align-items: center;">
        {#if copiedUrl}
          <Check size={16} /> Copied!
        {:else}
          <Copy size={16} /> Copy URL
        {/if}
      </button>
    </div>
  </div>

  <div class="glass-card">
    <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600; margin-bottom: 1.5rem;">Step-by-Step Connection Guide</h3>
    
    <div class="steps-container">
      <div class="step-card">
        <div class="step-number">1</div>
        <div class="step-content">
          <h4 class="step-title">Open Audion Desktop</h4>
          <p class="step-desc">Launch the Audion desktop media player application on your system.</p>
        </div>
        <Laptop size={24} style="color: var(--accent); opacity: 0.7;" />
      </div>

      <div class="step-card">
        <div class="step-number">2</div>
        <div class="step-content">
          <h4 class="step-title">Go to Settings</h4>
          <p class="step-desc">Click the gear icon in the lower left corner of the application window to open settings.</p>
        </div>
        <Settings size={24} style="color: var(--accent); opacity: 0.7;" />
      </div>

      <div class="step-card">
        <div class="step-number">3</div>
        <div class="step-content">
          <h4 class="step-title">Enter Server URL</h4>
          <p class="step-desc">In the Sync settings section, paste the Connection URL copied from the top of this page.</p>
        </div>
        <Link size={24} style="color: var(--accent); opacity: 0.7;" />
      </div>

      <div class="step-card">
        <div class="step-number">4</div>
        <div class="step-content">
          <h4 class="step-title">Authenticate & Connect</h4>
          <p class="step-desc">Enter your admin credentials (username and password) and click "Connect". Your library will begin syncing automatically in the background!</p>
        </div>
        <ShieldCheck size={24} style="color: var(--accent); opacity: 0.7;" />
      </div>
    </div>
  </div>

  <div class="glass-card">
    <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600; margin-bottom: 1rem;">Frequently Asked Questions</h3>
    <div style="display: flex; flex-direction: column; gap: 1rem; font-size: 0.9rem; line-height: 1.5;">
      <div>
        <h4 style="font-weight: 600; margin-bottom: 0.25rem; color: var(--text-primary);">Can I sync multiple devices?</h4>
        <p style="color: var(--text-secondary);">Yes! You can connect as many instances of the Audion app as you like. They will all synchronize to this central database.</p>
      </div>
      <div style="border-top: 1px solid var(--border-color); padding-top: 1rem;">
        <h4 style="font-weight: 600; margin-bottom: 0.25rem; color: var(--text-primary);">How is data stored?</h4>
        <p style="color: var(--text-secondary);">All database records and audio files are securely saved inside the persistent Docker volume volume on the server host machine.</p>
      </div>
    </div>
  </div>
</div>
