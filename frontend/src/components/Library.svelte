<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Trash2, Search, Music, Disc, AlertCircle, RefreshCw } from '@lucide/svelte';

  let { token, currentPlayingId, isPlaying, onPlayTrack, addToast } = $props<{
    token: string;
    currentPlayingId: number | null;
    isPlaying: boolean;
    onPlayTrack: (track: { id: number; title: string; artist: string }) => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  interface Track {
    id: number;
    title: string | null;
    artist: string | null;
    album: string | null;
    duration: number | null;
    format: string | null;
  }

  let tracks = $state<Track[]>([]);
  let searchQuery = $state('');
  let isLoading = $state(true);

  async function fetchTracks() {
    isLoading = true;
    try {
      let url = '/api/tracks?limit=100';
      if (searchQuery.trim().length > 0) {
        url = `/api/search?q=${encodeURIComponent(searchQuery)}`;
      }

      const res = await fetch(url, {
        headers: {
          'Authorization': `Bearer ${token}`
        }
      });

      if (!res.ok) {
        throw new Error('Failed to fetch tracks');
      }

      const data = await res.json();
      if (searchQuery.trim().length > 0) {
        tracks = data.tracks;
      } else {
        tracks = data;
      }
    } catch (err: any) {
      addToast(err.message || 'Failed to load tracks', 'error');
    } finally {
      isLoading = false;
    }
  }

  async function handleDelete(trackId: number, title: string | null) {
    if (!confirm(`Are you sure you want to delete "${title || 'this track'}"?`)) {
      return;
    }

    try {
      const res = await fetch(`/api/tracks/${trackId}`, {
        method: 'DELETE',
        headers: {
          'Authorization': `Bearer ${token}`
        }
      });

      if (!res.ok) {
        throw new Error('Failed to delete track');
      }

      addToast('Track deleted successfully', 'success');
      fetchTracks();
    } catch (err: any) {
      addToast(err.message || 'Delete failed', 'error');
    }
  }

  function formatDuration(secs: number | null): string {
    if (secs === null) return '--:--';
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60).toString().padStart(2, '0');
    return `${m}:${s}`;
  }

  // Handle search input with debouncing
  let searchTimeout: number;
  function handleSearchInput() {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      fetchTracks();
    }, 300) as unknown as number;
  }

  onMount(() => {
    fetchTracks();
  });
</script>

<div class="page-header">
  <h1 class="page-title">Library Manager</h1>
  <p class="page-subtitle">Browse and manage audio files stored on the server.</p>
</div>

<div class="glass-card">
  <div style="display: flex; gap: 1rem; margin-bottom: 1.5rem; position: relative;">
    <div style="position: relative; flex: 1; display: flex; align-items: center;">
      <Search size={18} style="position: absolute; left: 1rem; color: var(--text-secondary);" />
      <input 
        type="text" 
        class="form-input" 
        placeholder="Search by title, artist, or album..." 
        style="width: 100%; padding-left: 2.75rem;" 
        bind:value={searchQuery}
        oninput={handleSearchInput}
      />
    </div>
    <button onclick={fetchTracks} class="btn btn-secondary" style="display: flex; gap: 0.5rem; align-items: center;">
      <RefreshCw size={16} /> Refresh
    </button>
  </div>

  {#if isLoading}
    <div style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
      <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
      <span>Loading tracks...</span>
    </div>
  {:else if tracks.length === 0}
    <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 200px; color: var(--text-secondary); gap: 0.75rem;">
      <AlertCircle size={32} style="color: var(--accent); opacity: 0.8;" />
      <span style="font-weight: 500;">No tracks found matching your search.</span>
      <p style="font-size: 0.85rem; color: var(--text-muted);">Try uploading files to add them to your library.</p>
    </div>
  {:else}
    <div style="overflow-x: auto;">
      <table class="library-table" style="font-size: 0.95rem;">
        <thead>
          <tr>
            <th style="width: 50px;"></th>
            <th>Title</th>
            <th>Artist</th>
            <th>Album</th>
            <th style="width: 80px; text-align: right;">Length</th>
            <th style="width: 60px; text-align: center;">Format</th>
            <th style="width: 60px; text-align: center;">Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each tracks as track (track.id)}
            <tr>
              <td>
                <button 
                  onclick={() => onPlayTrack({ id: track.id, title: track.title || 'Unknown Title', artist: track.artist || 'Unknown Artist' })} 
                  class="btn" 
                  style="background: rgba(168,85,247,0.1); border: 1px solid rgba(168,85,247,0.2); border-radius: 50%; width: 32px; height: 32px; padding: 0; display: flex; align-items: center; justify-content: center; color: var(--accent);"
                >
                  {#if currentPlayingId === track.id && isPlaying}
                    <Pause size={14} fill="currentColor" />
                  {:else}
                    <Play size={14} fill="currentColor" style="margin-left: 2px;" />
                  {/if}
                </button>
              </td>
              <td>
                <div style="display: flex; align-items: center; gap: 0.5rem; font-weight: 500; color: var(--text-primary);">
                  <Music size={14} style="color: var(--text-muted);" />
                  <span>{track.title || 'Unknown Title'}</span>
                </div>
              </td>
              <td style="color: var(--text-secondary);">{track.artist || 'Unknown Artist'}</td>
              <td style="color: var(--text-secondary); display: flex; align-items: center; gap: 0.4rem; border: none; padding-top: 1.25rem;">
                <Disc size={14} style="color: var(--text-muted);" />
                <span>{track.album || 'Unknown Album'}</span>
              </td>
              <td style="color: var(--text-secondary); text-align: right; font-family: monospace;">{formatDuration(track.duration)}</td>
              <td style="text-align: center;">
                <span style="font-size: 0.75rem; text-transform: uppercase; background: rgba(255,255,255,0.05); padding: 0.2rem 0.5rem; border-radius: 4px; border: 1px solid var(--border-color); color: var(--text-secondary);">
                  {track.format || 'mp3'}
                </span>
              </td>
              <td>
                <div style="display: flex; justify-content: center;">
                  <button 
                    onclick={() => handleDelete(track.id, track.title)} 
                    class="btn" 
                    style="background: transparent; border: none; color: var(--text-muted); padding: 0.25rem;"
                    title="Delete track"
                  >
                    <Trash2 size={16} class="hover:text-red-500" style="transition: color 0.2s;" />
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .hover\:text-red-500:hover {
    color: var(--danger) !important;
  }
</style>
