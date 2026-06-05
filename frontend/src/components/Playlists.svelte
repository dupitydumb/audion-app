<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Trash2, ListMusic, Plus, ArrowLeft, ArrowUp, ArrowDown, Music, AlertCircle, RefreshCw } from '@lucide/svelte';

  let { token, currentPlayingId, isPlaying, onPlayTrack, addToast } = $props<{
    token: string;
    currentPlayingId: number | null;
    isPlaying: boolean;
    onPlayTrack: (track: { id: number; title: string; artist: string; format?: string | null; bitrate?: number | null }, queue?: any[]) => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  interface Playlist {
    id: number;
    name: string;
    cover_url: string | null;
    created_at: string | null;
  }

  interface Track {
    id: number;
    title: string | null;
    artist: string | null;
    album: string | null;
    duration: number | null;
    format: string | null;
    bitrate: number | null;
    genre?: string | null;
  }

  let playlists = $state<Playlist[]>([]);
  let isLoading = $state(true);
  let newPlaylistName = $state('');
  let isCreating = $state(false);

  // Detail View State
  let selectedPlaylist = $state<Playlist | null>(null);
  let playlistTracks = $state<Track[]>([]);
  let isLoadingTracks = $state(false);

  async function fetchPlaylists() {
    isLoading = true;
    try {
      const res = await fetch('/api/playlists', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!res.ok) throw new Error('Failed to load playlists');
      playlists = await res.json();
    } catch (err: any) {
      addToast(err.message || 'Failed to load playlists', 'error');
    } finally {
      isLoading = false;
    }
  }

  async function createPlaylist(e: SubmitEvent) {
    e.preventDefault();
    if (!newPlaylistName.trim()) return;

    isCreating = true;
    try {
      const res = await fetch('/api/playlists', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({ name: newPlaylistName, cover_url: null })
      });
      if (!res.ok) throw new Error('Failed to create playlist');
      newPlaylistName = '';
      addToast('Playlist created', 'success');
      fetchPlaylists();
    } catch (err: any) {
      addToast(err.message || 'Failed to create playlist', 'error');
    } finally {
      isCreating = false;
    }
  }

  async function deletePlaylist(id: number, name: string) {
    if (!confirm(`Are you sure you want to delete the playlist "${name}"?`)) return;

    try {
      const res = await fetch(`/api/playlists/${id}`, {
        method: 'DELETE',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!res.ok) throw new Error('Failed to delete playlist');
      addToast('Playlist deleted', 'success');
      if (selectedPlaylist?.id === id) {
        selectedPlaylist = null;
      }
      fetchPlaylists();
    } catch (err: any) {
      addToast(err.message || 'Failed to delete playlist', 'error');
    }
  }

  async function fetchPlaylistTracks(playlistId: number) {
    isLoadingTracks = true;
    try {
      const res = await fetch(`/api/playlists/${playlistId}/tracks`, {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!res.ok) throw new Error('Failed to load playlist tracks');
      playlistTracks = await res.json();
    } catch (err: any) {
      addToast(err.message || 'Failed to load tracks', 'error');
    } finally {
      isLoadingTracks = false;
    }
  }

  function selectPlaylist(playlist: Playlist) {
    selectedPlaylist = playlist;
    fetchPlaylistTracks(playlist.id);
  }

  async function removeTrackFromPlaylist(trackId: number, title: string | null) {
    if (!selectedPlaylist) return;
    if (!confirm(`Remove "${title || 'this track'}" from playlist?`)) return;

    try {
      const res = await fetch(`/api/playlists/${selectedPlaylist.id}/tracks/${trackId}`, {
        method: 'DELETE',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!res.ok) throw new Error('Failed to remove track');
      addToast('Track removed', 'success');
      fetchPlaylistTracks(selectedPlaylist.id);
    } catch (err: any) {
      addToast(err.message || 'Failed to remove track', 'error');
    }
  }

  async function moveTrack(index: number, direction: 'up' | 'down') {
    if (!selectedPlaylist) return;
    const toIndex = direction === 'up' ? index - 1 : index + 1;
    if (toIndex < 0 || toIndex >= playlistTracks.length) return;

    try {
      const res = await fetch(`/api/playlists/${selectedPlaylist.id}/tracks/reorder`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({ from_index: index, to_index: toIndex })
      });
      if (!res.ok) throw new Error('Failed to reorder tracks');
      // Instant updates local state to avoid flickers
      const temp = [...playlistTracks];
      const movedItem = temp.splice(index, 1)[0];
      temp.splice(toIndex, 0, movedItem);
      playlistTracks = temp;
    } catch (err: any) {
      addToast(err.message || 'Failed to reorder', 'error');
      // Fallback
      fetchPlaylistTracks(selectedPlaylist.id);
    }
  }

  function formatDuration(secs: number | null): string {
    if (secs === null) return '--:--';
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60).toString().padStart(2, '0');
    return `${m}:${s}`;
  }

  onMount(() => {
    fetchPlaylists();
  });
</script>

{#if !selectedPlaylist}
  <div class="page-header">
    <h1 class="page-title">Playlists</h1>
    <p class="page-subtitle">Create and organize custom track collections.</p>
  </div>

  <div style="display: flex; flex-direction: column; gap: 1.5rem;">
    <!-- Create Playlist Form -->
    <div class="glass-card">
      <form onsubmit={createPlaylist} style="display: flex; gap: 0.75rem; align-items: flex-end;">
        <div class="form-group" style="flex: 1; margin: 0;">
          <label class="form-label" for="playlist-name">New Playlist</label>
          <input 
            type="text" 
            id="playlist-name" 
            class="form-input" 
            placeholder="Enter playlist name..." 
            bind:value={newPlaylistName}
            disabled={isCreating}
          />
        </div>
        <button type="submit" class="btn btn-primary" style="height: 40px;" disabled={isCreating || !newPlaylistName.trim()}>
          <Plus size={16} /> Create
        </button>
      </form>
    </div>

    <!-- Playlists Grid -->
    <div class="glass-card">
      {#if isLoading}
        <div style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
          <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
          <span>Loading playlists...</span>
        </div>
      {:else if playlists.length === 0}
        <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 200px; color: var(--text-secondary); gap: 0.75rem;">
          <ListMusic size={32} style="color: var(--text-muted); opacity: 0.8;" />
          <span style="font-weight: 500;">No playlists created yet.</span>
          <p style="font-size: 0.85rem; color: var(--text-muted);">Use the form above to create your first playlist.</p>
        </div>
      {:else}
        <div class="playlist-grid">
          {#each playlists as pl (pl.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="playlist-card" onclick={() => selectPlaylist(pl)}>
              <div class="playlist-artwork">
                <ListMusic size={32} style="color: var(--text-muted);" />
              </div>
              <div class="playlist-info">
                <h4 class="playlist-title">{pl.name}</h4>
                <button 
                  onclick={(e) => { e.stopPropagation(); deletePlaylist(pl.id, pl.name); }} 
                  class="btn delete-btn"
                  title="Delete playlist"
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{:else}
  <!-- Playlist Detail View -->
  <div class="page-header" style="display: flex; align-items: center; gap: 1rem;">
    <button onclick={() => selectedPlaylist = null} class="btn btn-secondary" style="padding: 0.5rem; border-radius: 50%; display: flex; align-items: center; justify-content: center;">
      <ArrowLeft size={16} />
    </button>
    <div>
      <h1 class="page-title">{selectedPlaylist.name}</h1>
      <p class="page-subtitle">Playlist · {playlistTracks.length} tracks</p>
    </div>
  </div>

  <div class="glass-card">
    {#if isLoadingTracks}
      <div style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
        <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
        <span>Loading tracks...</span>
      </div>
    {:else if playlistTracks.length === 0}
      <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 200px; color: var(--text-secondary); gap: 0.75rem;">
        <AlertCircle size={32} style="color: var(--text-muted); opacity: 0.8;" />
        <span style="font-weight: 500;">No tracks in this playlist.</span>
        <p style="font-size: 0.85rem; color: var(--text-muted);">Go to the Library tab and click "+" on any track to add it here.</p>
      </div>
    {:else}
      <div style="overflow-x: auto;">
        <table class="library-table" style="font-size: 0.95rem;">
          <thead>
            <tr>
              <th style="width: 50px;"></th>
              <th style="width: 80px; text-align: center;">Order</th>
              <th>Title</th>
              <th>Artist</th>
              <th>Album</th>
              <th style="width: 80px; text-align: right;">Length</th>
              <th style="width: 100px; text-align: center;">Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each playlistTracks as track, index (track.id)}
              <tr>
                <td>
                  <button 
                    onclick={() => onPlayTrack({ 
                      id: track.id, 
                      title: track.title || 'Unknown Title', 
                      artist: track.artist || 'Unknown Artist',
                      format: track.format,
                      bitrate: track.bitrate
                    }, playlistTracks.map(t => ({
                      id: t.id,
                      title: t.title || 'Unknown Title',
                      artist: t.artist || 'Unknown Artist',
                      format: t.format,
                      bitrate: t.bitrate
                    })))} 
                    class="btn" 
                    style="background: rgba(255,255,255,0.04); border: 1px solid var(--border-color); border-radius: 50%; width: 32px; height: 32px; padding: 0; display: flex; align-items: center; justify-content: center; color: var(--text-primary);"
                  >
                    {#if currentPlayingId === track.id && isPlaying}
                      <Pause size={14} fill="currentColor" />
                    {:else}
                      <Play size={14} fill="currentColor" style="margin-left: 2px;" />
                    {/if}
                  </button>
                </td>
                <td>
                  <div style="display: flex; justify-content: center; align-items: center; gap: 0.25rem;">
                    <button 
                      onclick={() => moveTrack(index, 'up')} 
                      class="btn move-btn" 
                      disabled={index === 0}
                      title="Move Up"
                    >
                      <ArrowUp size={12} />
                    </button>
                    <button 
                      onclick={() => moveTrack(index, 'down')} 
                      class="btn move-btn" 
                      disabled={index === playlistTracks.length - 1}
                      title="Move Down"
                    >
                      <ArrowDown size={12} />
                    </button>
                  </div>
                </td>
                <td>
                  <div style="display: flex; align-items: center; gap: 0.75rem; font-weight: 500; color: var(--text-primary);">
                    <div class="track-thumbnail">
                      <img 
                        src="/api/tracks/{track.id}/cover?token={token}" 
                        alt="" 
                        onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
                      />
                      <Music size={14} style="color: var(--text-muted);" />
                    </div>
                    <div style="display: flex; flex-direction: column; gap: 0.2rem; min-width: 0;">
                      <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 240px;">
                        {track.title || 'Unknown Title'}
                      </span>
                      {#if track.genre}
                        <span class="genre-tag">{track.genre}</span>
                      {/if}
                    </div>
                  </div>
                </td>
                <td style="color: var(--text-secondary); max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                  {track.artist || 'Unknown Artist'}
                </td>
                <td style="color: var(--text-secondary); max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                  {track.album || 'Unknown Album'}
                </td>
                <td style="color: var(--text-secondary); text-align: right; font-family: monospace;">{formatDuration(track.duration)}</td>
                <td>
                  <div style="display: flex; justify-content: center;">
                    <button 
                      onclick={() => removeTrackFromPlaylist(track.id, track.title)} 
                      class="btn" 
                      style="background: transparent; border: none; color: var(--text-muted); padding: 0.25rem;"
                      title="Remove from playlist"
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
{/if}

<style>
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .hover\:text-red-500:hover {
    color: var(--danger) !important;
  }

  .playlist-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 1.25rem;
  }

  .playlist-card {
    background: rgba(255, 255, 255, 0.01);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.85rem;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    transition: all 0.2s ease;
  }

  .playlist-card:hover {
    border-color: var(--border-color-hover);
    background: rgba(255, 255, 255, 0.03);
  }

  .playlist-artwork {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .playlist-info {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }

  .playlist-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 0;
  }

  .delete-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    padding: 0.25rem;
  }

  .delete-btn:hover {
    color: var(--danger);
  }

  .track-thumbnail {
    width: 32px;
    height: 32px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-color);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    position: relative;
    flex-shrink: 0;
  }

  .track-thumbnail img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    position: absolute;
    top: 0;
    left: 0;
  }

  .move-btn {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    padding: 0.2rem;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .move-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-primary);
  }

  .move-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
</style>
