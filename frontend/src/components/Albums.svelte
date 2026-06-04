<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Disc, ArrowLeft, Music, AlertCircle, RefreshCw, Heart, Plus } from '@lucide/svelte';

  let { token, currentPlayingId, isPlaying, likedTrackIds, onPlayTrack, onToggleLike, addToast } = $props<{
    token: string;
    currentPlayingId: number | null;
    isPlaying: boolean;
    likedTrackIds: number[];
    onPlayTrack: (track: { id: number; title: string; artist: string }) => void;
    onToggleLike: (trackId: number) => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  interface Album {
    id: number;
    name: string;
    artist: string | null;
    art_path: string | null;
  }

  interface Track {
    id: number;
    title: string | null;
    artist: string | null;
    album: string | null;
    duration: number | null;
    format: string | null;
    bitrate: number | null;
  }

  interface Playlist {
    id: number;
    name: string;
  }

  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);
  let isLoading = $state(true);
  let openPlaylistDropdownId = $state<number | null>(null);

  // Detail View State
  let selectedAlbum = $state<Album | null>(null);
  let albumTracks = $state<Track[]>([]);
  let isLoadingTracks = $state(false);

  async function fetchAlbums() {
    isLoading = true;
    try {
      const res = await fetch('/api/albums?limit=100', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!res.ok) throw new Error('Failed to load albums');
      albums = await res.json();
    } catch (err: any) {
      addToast(err.message || 'Failed to load albums', 'error');
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

  async function fetchAlbumTracks(albumId: number) {
    isLoadingTracks = true;
    try {
      const res = await fetch(`/api/albums/${albumId}/tracks`, {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!res.ok) throw new Error('Failed to load album tracks');
      albumTracks = await res.json();
    } catch (err: any) {
      addToast(err.message || 'Failed to load tracks', 'error');
    } finally {
      isLoadingTracks = false;
    }
  }

  function selectAlbum(album: Album) {
    selectedAlbum = album;
    fetchAlbumTracks(album.id);
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

  onMount(() => {
    fetchAlbums();
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

{#if !selectedAlbum}
  <div class="page-header">
    <h1 class="page-title">Albums</h1>
    <p class="page-subtitle">Browse albums stored in your music library.</p>
  </div>

  <div class="glass-card">
    {#if isLoading}
      <div style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
        <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
        <span>Loading albums...</span>
      </div>
    {:else if albums.length === 0}
      <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 200px; color: var(--text-secondary); gap: 0.75rem;">
        <Disc size={32} style="color: var(--text-muted); opacity: 0.8;" />
        <span style="font-weight: 500;">No albums found.</span>
        <p style="font-size: 0.85rem; color: var(--text-muted);">Try uploading music containing metadata tags.</p>
      </div>
    {:else}
      <div class="album-grid">
        {#each albums as alb (alb.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="album-card" onclick={() => selectAlbum(alb)}>
            <div class="album-artwork">
              <img 
                src="/api/albums/{alb.id}/artwork" 
                alt="" 
                onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
              />
              <Disc size={40} style="color: var(--text-muted);" />
            </div>
            <div class="album-info">
              <h4 class="album-name-title" title={alb.name}>{alb.name}</h4>
              <p class="album-artist-name" title={alb.artist || 'Unknown Artist'}>{alb.artist || 'Unknown Artist'}</p>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{:else}
  <!-- Album Detail View -->
  <div class="page-header" style="display: flex; align-items: center; gap: 1.5rem;">
    <button onclick={() => selectedAlbum = null} class="btn btn-secondary" style="padding: 0.5rem; border-radius: 50%; display: flex; align-items: center; justify-content: center;">
      <ArrowLeft size={16} />
    </button>
    <div class="album-header-artwork">
      <img 
        src="/api/albums/{selectedAlbum.id}/artwork" 
        alt="" 
        onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
      />
      <Disc size={36} style="color: var(--text-muted);" />
    </div>
    <div>
      <h1 class="page-title">{selectedAlbum.name}</h1>
      <p class="page-subtitle">{selectedAlbum.artist || 'Unknown Artist'} · {albumTracks.length} tracks</p>
    </div>
  </div>

  <div class="glass-card">
    {#if isLoadingTracks}
      <div style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
        <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
        <span>Loading tracks...</span>
      </div>
    {:else if albumTracks.length === 0}
      <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 200px; color: var(--text-secondary); gap: 0.75rem;">
        <AlertCircle size={32} style="color: var(--text-muted); opacity: 0.8;" />
        <span style="font-weight: 500;">No tracks in this album.</span>
      </div>
    {:else}
      <div style="overflow-x: auto;">
        <table class="library-table" style="font-size: 0.95rem;">
          <thead>
            <tr>
              <th style="width: 50px;"></th>
              <th>Title</th>
              <th>Artist</th>
              <th style="width: 80px; text-align: right;">Length</th>
              <th style="width: 80px; text-align: center;">Format</th>
              <th style="width: 100px; text-align: center;">Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each albumTracks as track (track.id)}
              <tr>
                <td>
                  <button 
                    onclick={() => onPlayTrack({ 
                    id: track.id, 
                    title: track.title || 'Unknown Title', 
                    artist: track.artist || 'Unknown Artist',
                    format: track.format,
                    bitrate: track.bitrate
                  })} 
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

  .album-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 1.5rem;
  }

  .album-card {
    background: rgba(255, 255, 255, 0.01);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.75rem;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    transition: all 0.2s ease;
  }

  .album-card:hover {
    border-color: var(--border-color-hover);
    background: rgba(255, 255, 255, 0.03);
  }

  .album-artwork {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    position: relative;
  }

  .album-artwork img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    position: absolute;
    top: 0;
    left: 0;
  }

  .album-header-artwork {
    width: 64px;
    height: 64px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    position: relative;
    flex-shrink: 0;
  }

  .album-header-artwork img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    position: absolute;
    top: 0;
    left: 0;
  }

  .album-info {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .album-name-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 0;
  }

  .album-artist-name {
    font-size: 0.75rem;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 0;
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
