<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Music, Disc, AlertCircle, RefreshCw, Heart, Plus } from '@lucide/svelte';

  let { token, currentPlayingId, isPlaying, likedTrackIds, onPlayTrack, onToggleLike, addToast } = $props<{
    token: string;
    currentPlayingId: number | null;
    isPlaying: boolean;
    likedTrackIds: number[];
    onPlayTrack: (track: { id: number; title: string; artist: string }) => void;
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
  }

  interface Playlist {
    id: number;
    name: string;
  }

  let likedTracks = $state<Track[]>([]);
  let playlists = $state<Playlist[]>([]);
  let isLoading = $state(true);
  let openPlaylistDropdownId = $state<number | null>(null);

  async function fetchLikedTracks() {
    isLoading = true;
    try {
      const res = await fetch('/api/liked', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!res.ok) throw new Error('Failed to load liked tracks');
      likedTracks = await res.json();
    } catch (err: any) {
      addToast(err.message || 'Failed to load liked tracks', 'error');
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

  async function handleToggleLikeLocal(trackId: number) {
    // Optimistic local state update first so it vanishes immediately from liked list
    likedTracks = likedTracks.filter(t => t.id !== trackId);
    await onToggleLike(trackId);
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

  function formatDuration(secs: number | null): string {
    if (secs === null) return '--:--';
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60).toString().padStart(2, '0');
    return `${m}:${s}`;
  }

  // Refresh when the list changes globally
  $effect(() => {
    // If the count of global likedTrackIds has changed compared to our list, refresh
    if (token && likedTrackIds) {
      // Check if we need to sync
      const localIds = likedTracks.map(t => t.id);
      const isSynced = likedTrackIds.length === localIds.length && likedTrackIds.every((id: number) => localIds.includes(id));
      if (!isSynced && !isLoading) {
        fetchLikedTracks();
      }
    }
  });

  onMount(() => {
    fetchLikedTracks();
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
  <h1 class="page-title">Liked Tracks</h1>
  <p class="page-subtitle">Your personally favorited songs, automatically synchronized.</p>
</div>

<div class="glass-card">
  {#if isLoading}
    <div style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
      <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
      <span>Loading liked tracks...</span>
    </div>
  {:else if likedTracks.length === 0}
    <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 200px; color: var(--text-secondary); gap: 0.75rem;">
      <Heart size={32} style="color: var(--text-muted); opacity: 0.8;" />
      <span style="font-weight: 500;">No liked tracks yet.</span>
      <p style="font-size: 0.85rem; color: var(--text-muted);">Heart your favorite songs in the Library to see them here.</p>
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
            <th style="width: 100px; text-align: center;">Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each likedTracks as track (track.id)}
            <tr>
              <td>
                <button 
                  onclick={() => onPlayTrack({ id: track.id, title: track.title || 'Unknown Title', artist: track.artist || 'Unknown Artist' })} 
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
                    onclick={() => handleToggleLikeLocal(track.id)} 
                    class="btn" 
                    style="background: transparent; border: none; color: var(--danger); padding: 0.25rem;"
                    title="Unlike track"
                  >
                    <Heart size={16} fill="currentColor" />
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
</style>
