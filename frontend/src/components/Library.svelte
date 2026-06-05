<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Trash2, Search, Music, Disc, AlertCircle, RefreshCw, Heart, Plus, Pencil } from '@lucide/svelte';

  let { token, currentPlayingId, isPlaying, likedTrackIds, onPlayTrack, onToggleLike, addToast } = $props<{
    token: string;
    currentPlayingId: number | null;
    isPlaying: boolean;
    likedTrackIds: number[];
    onPlayTrack: (track: { id: number; title: string; artist: string; format?: string | null; bitrate?: number | null }, queue?: any[]) => void;
    onToggleLike: (trackId: number) => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  interface Track {
    id: number;
    title: string | null;
    artist: string | null;
    album: string | null;
    duration: number | null;
    format: string | null;
    bitrate: number | null;
    genre?: string | null;
    track_number?: number | null;
    disc_number?: number | null;
  }

  let showEditModal = $state(false);
  let editingTrack = $state<Track | null>(null);
  let editTitle = $state('');
  let editArtist = $state('');
  let editAlbum = $state('');
  let editGenre = $state('');
  let editTrackNumber = $state<number | null>(null);
  let editDiscNumber = $state<number | null>(null);
  let isSavingMetadata = $state(false);

  function openEditModal(track: Track) {
    editingTrack = track;
    editTitle = track.title || '';
    editArtist = track.artist || '';
    editAlbum = track.album || '';
    editGenre = track.genre || '';
    editTrackNumber = track.track_number || null;
    editDiscNumber = track.disc_number || null;
    showEditModal = true;
  }

  async function handleSaveMetadata(e: SubmitEvent) {
    e.preventDefault();
    if (!editingTrack) return;
    isSavingMetadata = true;
    try {
      const res = await fetch(`/api/tracks/${editingTrack.id}/metadata`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({
          title: editTitle,
          artist: editArtist,
          album: editAlbum,
          genre: editGenre,
          track_number: editTrackNumber,
          disc_number: editDiscNumber
        })
      });

      if (!res.ok) {
        throw new Error('Failed to update track metadata');
      }

      const updatedTrack = await res.json();
      tracks = tracks.map(t => t.id === updatedTrack.id ? updatedTrack : t);
      addToast('Metadata updated successfully', 'success');
      showEditModal = false;
    } catch (err: any) {
      addToast(err.message || 'Failed to save metadata', 'error');
    } finally {
      isSavingMetadata = false;
    }
  }

  interface Playlist {
    id: number;
    name: string;
  }

  let tracks = $state<Track[]>([]);
  let playlists = $state<Playlist[]>([]);
  let searchQuery = $state('');
  let isLoading = $state(true);
  let openPlaylistDropdownId = $state<number | null>(null);

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

  async function fetchPlaylists() {
    if (!token) return;
    try {
      const res = await fetch('/api/playlists', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        playlists = await res.json();
      }
    } catch (e) {
      // Ignore
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

  function togglePlaylistDropdown(trackId: number, event: MouseEvent) {
    event.stopPropagation();
    if (openPlaylistDropdownId === trackId) {
      openPlaylistDropdownId = null;
    } else {
      openPlaylistDropdownId = trackId;
    }
  }

  async function addTrackToPlaylist(trackId: number, playlistId: number) {
    try {
      const res = await fetch(`/api/playlists/${playlistId}/tracks`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({ track_id: trackId })
      });
      if (res.ok || res.status === 201) {
        addToast('Track added to playlist', 'success');
      } else {
        addToast('Failed to add track to playlist', 'error');
      }
    } catch (e) {
      addToast('Failed to add track to playlist', 'error');
    }
    openPlaylistDropdownId = null;
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
    fetchPlaylists();

    const handleGlobalClick = () => {
      openPlaylistDropdownId = null;
    };
    window.addEventListener('click', handleGlobalClick);
    return () => {
      window.removeEventListener('click', handleGlobalClick);
    };
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
      <AlertCircle size={32} style="color: var(--text-muted); opacity: 0.8;" />
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
            <th style="width: 120px; text-align: center;">Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each tracks as track (track.id)}
            <tr>
              <td>
                <button 
                  onclick={() => onPlayTrack({ 
                    id: track.id, 
                    title: track.title || 'Unknown Title', 
                    artist: track.artist || 'Unknown Artist',
                    format: track.format,
                    bitrate: track.bitrate
                  }, tracks.map(t => ({
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
                <div style="display: flex; align-items: center; gap: 0.75rem; font-weight: 500; color: var(--text-primary);">
                  <div class="track-thumbnail">
                    <img 
                      src="/api/tracks/{track.id}/cover?token={token}" 
                      alt="" 
                      onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
                    />
                    <Music size={14} style="color: var(--text-muted);" />
                  </div>
                  <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 240px;">
                    {track.title || 'Unknown Title'}
                  </span>
                </div>
              </td>
              <td style="color: var(--text-secondary); max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                {track.artist || 'Unknown Artist'}
              </td>
              <td style="color: var(--text-secondary); max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                <div style="display: flex; align-items: center; gap: 0.4rem;">
                  <Disc size={14} style="color: var(--text-muted);" />
                  <span>{track.album || 'Unknown Album'}</span>
                </div>
              </td>
              <td style="color: var(--text-secondary); text-align: right; font-family: monospace;">{formatDuration(track.duration)}</td>
              <td style="text-align: center;">
                <span style="font-size: 0.75rem; text-transform: uppercase; background: rgba(255,255,255,0.03); padding: 0.2rem 0.5rem; border-radius: 4px; border: 1px solid var(--border-color); color: var(--text-secondary);">
                  {track.format || 'mp3'}
                </span>
              </td>
              <td>
                <div style="display: flex; justify-content: center; align-items: center; gap: 0.5rem;">
                  <button 
                    onclick={() => onToggleLike(track.id)} 
                    class="btn" 
                    style="background: transparent; border: none; color: {likedTrackIds.includes(track.id) ? 'var(--danger)' : 'var(--text-muted)'}; padding: 0.25rem;"
                    title={likedTrackIds.includes(track.id) ? 'Unlike track' : 'Like track'}
                  >
                    <Heart size={16} fill={likedTrackIds.includes(track.id) ? 'currentColor' : 'none'} />
                  </button>

                  <div style="position: relative;">
                    <button 
                      onclick={(e) => togglePlaylistDropdown(track.id, e)} 
                      class="btn" 
                      style="background: transparent; border: none; color: var(--text-muted); padding: 0.25rem;"
                      title="Add to playlist"
                    >
                      <Plus size={16} />
                    </button>
                    {#if openPlaylistDropdownId === track.id}
                      <div class="playlist-dropdown glass-card">
                        {#if playlists.length === 0}
                          <div style="padding: 0.5rem; font-size: 0.75rem; color: var(--text-muted); white-space: nowrap;">No playlists.</div>
                        {:else}
                          {#each playlists as pl}
                            <button 
                              onclick={() => addTrackToPlaylist(track.id, pl.id)} 
                              class="dropdown-item"
                            >
                              {pl.name}
                            </button>
                          {/each}
                        {/if}
                      </div>
                    {/if}
                  </div>

                  <button 
                    onclick={() => openEditModal(track)} 
                    class="btn" 
                    style="background: transparent; border: none; color: var(--text-muted); padding: 0.25rem;"
                    title="Edit metadata"
                  >
                    <Pencil size={16} class="hover:text-accent" style="transition: color 0.2s;" />
                  </button>

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

{#if showEditModal}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => showEditModal = false}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal-content glass-card" onclick={(e) => e.stopPropagation()}>
      <h2 style="font-family: var(--font-heading); font-size: 1.3rem; font-weight: 600; margin-bottom: 1.5rem; color: var(--text-primary);">Edit Metadata</h2>
      
      <form onsubmit={handleSaveMetadata}>
        <div style="display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1rem; margin-bottom: 1.5rem;">
          <div class="form-group" style="grid-column: span 2;">
            <label class="form-label" for="edit-title">Title</label>
            <input type="text" id="edit-title" class="form-input" style="width: 100%;" bind:value={editTitle} />
          </div>
          
          <div class="form-group">
            <label class="form-label" for="edit-artist">Artist</label>
            <input type="text" id="edit-artist" class="form-input" style="width: 100%;" bind:value={editArtist} />
          </div>
          
          <div class="form-group">
            <label class="form-label" for="edit-album">Album</label>
            <input type="text" id="edit-album" class="form-input" style="width: 100%;" bind:value={editAlbum} />
          </div>
          
          <div class="form-group">
            <label class="form-label" for="edit-genre">Genre</label>
            <input type="text" id="edit-genre" class="form-input" style="width: 100%;" bind:value={editGenre} />
          </div>
          
          <div class="form-group" style="display: flex; gap: 1rem;">
            <div style="flex: 1;">
              <label class="form-label" for="edit-track">Track #</label>
              <input type="number" id="edit-track" class="form-input" style="width: 100%;" bind:value={editTrackNumber} />
            </div>
            <div style="flex: 1;">
              <label class="form-label" for="edit-disc">Disc #</label>
              <input type="number" id="edit-disc" class="form-input" style="width: 100%;" bind:value={editDiscNumber} />
            </div>
          </div>
        </div>
        
        <div style="display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 1.5rem;">
          <button type="button" class="btn btn-secondary" onclick={() => showEditModal = false} disabled={isSavingMetadata}>
            Cancel
          </button>
          <button type="submit" class="btn btn-primary" style="display: flex; gap: 0.5rem; align-items: center;" disabled={isSavingMetadata}>
            {#if isSavingMetadata}
              <RefreshCw size={14} class="animate-spin" style="animation: spin 1s linear infinite;" /> Saving...
            {:else}
              Save Changes
            {/if}
          </button>
        </div>
      </form>
    </div>
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

  .playlist-dropdown {
    position: absolute;
    right: 0;
    top: 100%;
    z-index: 200;
    min-width: 160px;
    padding: 0.35rem;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    background: #0d0d0f;
    border: 1px solid var(--border-color);
  }

  .dropdown-item {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    padding: 0.4rem 0.65rem;
    border-radius: 4px;
    text-align: left;
    font-size: 0.85rem;
    cursor: pointer;
    width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dropdown-item:hover {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-primary);
  }

  .hover\:text-accent:hover {
    color: var(--accent) !important;
  }

  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal-content {
    width: 100%;
    max-width: 500px;
    padding: 2rem;
    border: 1px solid var(--border-color);
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5);
  }
</style>
