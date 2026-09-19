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

<div class="page-header" style="display: flex; align-items: flex-start; justify-content: space-between; flex-wrap: wrap; gap: 1rem;">
  <div>
    <h1 class="page-title">Dashboard</h1>
    <p class="page-subtitle">Welcome to your Audion Server administration center.</p>
  </div>
  <button onclick={fetchStats} class="btn btn-secondary" style="padding: 0.4rem 0.8rem; font-size: 0.8rem;" disabled={isLoading}>
    <RefreshCw size={12} style="margin-right: 0.25rem;" /> Refresh Stats
  </button>
</div>

{#if isLoading}
  <div class="dashboard-grid" aria-busy="true">
    {#each Array(4) as _}
      <div class="glass-card skeleton-card skeleton">
        <div style="display: flex; align-items: center; justify-content: space-between;">
          <div class="skeleton-line" style="width: 90px;"></div>
          <div class="skeleton-icon"></div>
        </div>
        <div class="skeleton-line" style="width: 140px; height: 28px;"></div>
      </div>
    {/each}
  </div>

  <div class="glass-card skeleton" style="margin-top: 1rem;">
    <div class="skeleton-line" style="width: 160px; margin-bottom: 1.25rem;"></div>
    <div class="quick-actions-grid">
      {#each Array(3) as _}
        <div class="glass-card skeleton" style="padding: 1.25rem;">
          <div class="skeleton-line" style="width: 160px; margin-bottom: 0.5rem;"></div>
          <div class="skeleton-line" style="width: 220px;"></div>
        </div>
      {/each}
    </div>
  </div>
{:else if stats}
  {#if stats.total_tracks === 0}
    <div class="glass-card" style="margin-bottom: 1.5rem; padding: 2rem; text-align: center; border: 1px dashed var(--border-color);">
      <div style="font-size: 2.5rem; margin-bottom: 0.75rem;">🎵</div>
      <h2 style="font-family: var(--font-heading); font-size: 1.25rem; font-weight: 600; margin-bottom: 0.5rem;">Your library is empty</h2>
      <p style="color: var(--text-secondary); margin-bottom: 1.25rem;">Upload your first tracks to get started with Audion Server.</p>
      <button onclick={() => setActiveTab('upload')} class="btn btn-primary">
        Upload Music →
      </button>
    </div>
  {/if}
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
    <div class="quick-actions-grid">
      <button 
        onclick={() => setActiveTab('upload')} 
        class="btn btn-secondary quick-action-btn"
      >
        <span class="quick-action-text">
          <strong class="quick-action-title">Upload New Music</strong>
          <span class="quick-action-desc">Batch drag-and-drop audio files</span>
        </span>
        <ArrowRight size={18} style="color: var(--accent);" />
      </button>

      <button 
        onclick={() => setActiveTab('started')} 
        class="btn btn-secondary quick-action-btn"
      >
        <span class="quick-action-text">
          <strong class="quick-action-title">Getting Started Guide</strong>
          <span class="quick-action-desc">Connect your Audion desktop client</span>
        </span>
        <ArrowRight size={18} style="color: var(--accent);" />
      </button>

      <button 
        onclick={() => setActiveTab('library')} 
        class="btn btn-secondary quick-action-btn"
      >
        <span class="quick-action-text">
          <strong class="quick-action-title">Library Browser</strong>
          <span class="quick-action-desc">Browse, preview and manage tracks</span>
        </span>
        <ArrowRight size={18} style="color: var(--accent);" />
      </button>
    </div>
  </div>

{:else}
  <div class="glass-card" style="text-align: center; padding: 3rem; color: var(--text-secondary);">
    <p>Failed to load server statistics. Please try refreshing.</p>
    <button onclick={fetchStats} class="btn btn-primary" style="margin-top: 1rem;">
      <RefreshCw size={16} style="margin-right: 0.25rem;" /> Retry
    </button>
  </div>
{/if}

