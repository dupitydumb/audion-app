<script lang="ts">
  import { onMount } from 'svelte';
  import { Music, Disc, Users, HardDrive, ArrowRight, RefreshCw } from '@lucide/svelte';

  let { token, setActiveTab, addToast } = $props<{
    token: string;
    setActiveTab: (tab: string) => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  let stats = $state<{
    total_tracks: number;
    total_albums: number;
    total_artists: number;
    total_size_bytes: number;
    data_dir: string;
  } | null>(null);

  let isLoading = $state(true);

  async function fetchStats() {
    isLoading = true;
    try {
      const res = await fetch('/api/stats', {
        headers: {
          'Authorization': `Bearer ${token}`
        }
      });
      if (!res.ok) {
        throw new Error('Failed to fetch stats');
      }
      stats = await res.json();
    } catch (err: any) {
      addToast(err.message || 'Failed to load stats', 'error');
    } finally {
      isLoading = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  onMount(() => {
    fetchStats();
  });
</script>

<div class="page-header">
  <h1 class="page-title">Dashboard</h1>
  <p class="page-subtitle">Welcome to your Audion Server administration center.</p>
</div>

{#if isLoading}
  <div style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
    <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
    <span>Loading stats...</span>
  </div>
{:else if stats}
  <div class="dashboard-grid">
    <div class="glass-card stat-card">
      <div class="stat-header">
        <span>Total Tracks</span>
        <div class="stat-icon-wrapper">
          <Music size={18} />
        </div>
      </div>
      <div class="stat-value">{stats.total_tracks}</div>
    </div>

    <div class="glass-card stat-card">
      <div class="stat-header">
        <span>Total Albums</span>
        <div class="stat-icon-wrapper">
          <Disc size={18} />
        </div>
      </div>
      <div class="stat-value">{stats.total_albums}</div>
    </div>

    <div class="glass-card stat-card">
      <div class="stat-header">
        <span>Total Artists</span>
        <div class="stat-icon-wrapper">
          <Users size={18} />
        </div>
      </div>
      <div class="stat-value">{stats.total_artists}</div>
    </div>

    <div class="glass-card stat-card">
      <div class="stat-header">
        <span>Storage Used</span>
        <div class="stat-icon-wrapper">
          <HardDrive size={18} />
        </div>
      </div>
      <div class="stat-value">{formatBytes(stats.total_size_bytes)}</div>
    </div>
  </div>

  <div class="glass-card" style="margin-top: 1rem;">
    <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600; margin-bottom: 1.25rem;">Quick Actions</h3>
    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 1rem;">
      <button 
        onclick={() => setActiveTab('upload')} 
        class="btn btn-secondary" 
        style="justify-content: space-between; text-align: left; padding: 1.25rem;"
      >
        <span style="display: flex; flex-direction: column; gap: 0.25rem;">
          <strong style="font-size: 1rem; color: var(--text-primary);">Upload New Music</strong>
          <span style="font-size: 0.8rem; font-weight: normal; color: var(--text-secondary);">Batch drag-and-drop audio files</span>
        </span>
        <ArrowRight size={18} style="color: var(--accent);" />
      </button>

      <button 
        onclick={() => setActiveTab('started')} 
        class="btn btn-secondary" 
        style="justify-content: space-between; text-align: left; padding: 1.25rem;"
      >
        <span style="display: flex; flex-direction: column; gap: 0.25rem;">
          <strong style="font-size: 1rem; color: var(--text-primary);">Getting Started Guide</strong>
          <span style="font-size: 0.8rem; font-weight: normal; color: var(--text-secondary);">Connect your Audion desktop client</span>
        </span>
        <ArrowRight size={18} style="color: var(--accent);" />
      </button>

      <button 
        onclick={() => setActiveTab('library')} 
        class="btn btn-secondary" 
        style="justify-content: space-between; text-align: left; padding: 1.25rem;"
      >
        <span style="display: flex; flex-direction: column; gap: 0.25rem;">
          <strong style="font-size: 1rem; color: var(--text-primary);">Library Browser</strong>
          <span style="font-size: 0.8rem; font-weight: normal; color: var(--text-secondary);">Browse, preview and manage tracks</span>
        </span>
        <ArrowRight size={18} style="color: var(--accent);" />
      </button>
    </div>
  </div>

  <div class="glass-card" style="margin-top: 1.5rem; display: flex; align-items: center; justify-content: space-between; font-size: 0.85rem; color: var(--text-secondary);">
    <div>
      <span style="font-weight: 600;">Data Directory:</span> 
      <code style="margin-left: 0.5rem; background: rgba(0,0,0,0.3); border: 1px solid var(--border-color);">{stats.data_dir}</code>
    </div>
    <button onclick={fetchStats} class="btn btn-secondary" style="padding: 0.4rem 0.8rem; font-size: 0.8rem;">
      <RefreshCw size={12} style="margin-right: 0.25rem;" /> Refresh Stats
    </button>
  </div>
{:else}
  <div class="glass-card" style="text-align: center; padding: 3rem; color: var(--text-secondary);">
    <p>Failed to load server statistics. Please try refreshing.</p>
    <button onclick={fetchStats} class="btn btn-primary" style="margin-top: 1rem;">
      <RefreshCw size={16} style="margin-right: 0.25rem;" /> Retry
    </button>
  </div>
{/if}

<style>
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
