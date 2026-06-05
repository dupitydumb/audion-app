<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Users, Disc, ArrowLeft, Music, AlertCircle, RefreshCw, Heart, Plus } from '@lucide/svelte';

  let { token, currentPlayingId, isPlaying, likedTrackIds, onPlayTrack, onToggleLike, addToast } = $props<{
    token: string;
    currentPlayingId: number | null;
    isPlaying: boolean;
    likedTrackIds: number[];
    onPlayTrack: (track: { id: number; title: string; artist: string; format?: string | null; bitrate?: number | null }, queue?: any[]) => void;
    onToggleLike: (trackId: number) => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  interface Artist {
    name: string;
    track_count: number;
    album_count: number;
  }

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
    genre?: string | null;
  }

  interface Playlist {
    id: number;
    name: string;
  }

  let artists = $state<Artist[]>([]);
  let playlists = $state<Playlist[]>([]);
  let isLoading = $state(true);
  let openPlaylistDropdownId = $state<number | null>(null);

  // Detail View State
  let selectedArtistName = $state<string | null>(null);
  let artistAlbums = $state<Album[]>([]);
  let artistTracks = $state<Track[]>([]);
  let isLoadingDetail = $state(false);

  async function fetchArtists() {
    isLoading = true;
    try {
      const res = await fetch('/api/artists', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!res.ok) throw new Error('Failed to load artists');
      artists = await res.json();
    } catch (err: any) {
      addToast(err.message || 'Failed to load artists', 'error');
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

  async function fetchArtistDetails(name: string) {
    isLoadingDetail = true;
    try {
      const [albumsRes, tracksRes] = await Promise.all([
        fetch(`/api/artists/${encodeURIComponent(name)}/albums`, {
          headers: { 'Authorization': `Bearer ${token}` }
        }),
        fetch(`/api/artists/${encodeURIComponent(name)}/tracks`, {
          headers: { 'Authorization': `Bearer ${token}` }
        })
      ]);

      if (!albumsRes.ok || !tracksRes.ok) throw new Error('Failed to load artist details');
      
      artistAlbums = await albumsRes.json();
      artistTracks = await tracksRes.json();
    } catch (err: any) {
      addToast(err.message || 'Failed to load details', 'error');
    } finally {
      isLoadingDetail = false;
    }
  }

  function selectArtist(name: string) {
    selectedArtistName = name;
    fetchArtistDetails(name);
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
    fetchArtists();
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

{#if !selectedArtistName}
  <div class="page-header">
    <h1 class="page-title">Artists</h1>
    <p class="page-subtitle">Browse artists stored in your music library.</p>
  </div>

  <div class="glass-card">
    {#if isLoading}
      <div style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
        <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
        <span>Loading artists...</span>
      </div>
    {:else if artists.length === 0}
      <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 200px; color: var(--text-secondary); gap: 0.75rem;">
        <Users size={32} style="color: var(--text-muted); opacity: 0.8;" />
        <span style="font-weight: 500;">No artists found.</span>
        <p style="font-size: 0.85rem; color: var(--text-muted);">Try uploading music containing metadata tags.</p>
      </div>
    {:else}
      <div class="artist-list-container">
        {#each artists as art}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="artist-row" onclick={() => selectArtist(art.name)}>
            <div class="artist-avatar-wrapper">
              <Users size={16} style="color: var(--text-secondary);" />
            </div>
            <div class="artist-row-info">
              <span class="artist-row-name">{art.name}</span>
              <span class="artist-row-meta">{art.album_count} albums · {art.track_count} tracks</span>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{:else}
  <!-- Artist Detail View -->
  <div class="page-header" style="display: flex; align-items: center; gap: 1rem;">
    <button onclick={() => selectedArtistName = null} class="btn btn-secondary" style="padding: 0.5rem; border-radius: 50%; display: flex; align-items: center; justify-content: center;">
      <ArrowLeft size={16} />
    </button>
    <div>
      <h1 class="page-title">{selectedArtistName}</h1>
      <p class="page-subtitle">Artist · {artistAlbums.length} albums · {artistTracks.length} tracks</p>
    </div>
  </div>

  {#if isLoadingDetail}
    <div class="glass-card" style="display: flex; justify-content: center; align-items: center; min-height: 200px; gap: 0.5rem; color: var(--text-secondary);">
      <RefreshCw size={20} class="animate-spin" style="animation: spin 1s linear infinite;" />
      <span>Loading artist details...</span>
    </div>
  {:else}
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      <!-- Artist Albums -->
      {#if artistAlbums.length > 0}
        <div class="glass-card">
          <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600; margin-bottom: 1rem; display: flex; align-items: center; gap: 0.5rem;">
            <Disc size={18} style="color: var(--text-muted);" /> Albums
          </h3>
          <div class="mini-album-grid">
            {#each artistAlbums as alb}
              <div class="mini-album-card">
                <div class="mini-album-artwork">
                  <img 
                    src="/api/albums/{alb.id}/artwork" 
                    alt="" 
                    onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
                  />
                  <Disc size={20} style="color: var(--text-muted);" />
                </div>
                <span class="mini-album-title" title={alb.name}>{alb.name}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Artist Tracks -->
      <div class="glass-card">
        <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600; margin-bottom: 1rem; display: flex; align-items: center; gap: 0.5rem;">
          <Music size={18} style="color: var(--text-muted);" /> Tracks
        </h3>
        {#if artistTracks.length === 0}
          <p style="font-size: 0.9rem; color: var(--text-muted);">No tracks found for this artist.</p>
        {:else}
          <div style="overflow-x: auto;">
            <table class="library-table" style="font-size: 0.95rem;">
              <thead>
                <tr>
                  <th style="width: 50px;"></th>
                  <th>Title</th>
                  <th>Album</th>
                  <th style="width: 80px; text-align: right;">Length</th>
                  <th style="width: 80px; text-align: center;">Format</th>
                  <th style="width: 100px; text-align: center;">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each artistTracks as track (track.id)}
                  <tr>
                    <td>
                      <button 
                        onclick={() => onPlayTrack({ 
                          id: track.id, 
                          title: track.title || 'Unknown Title', 
                          artist: track.artist || 'Unknown Artist',
                          format: track.format,
                          bitrate: track.bitrate
                        }, artistTracks.map(t => ({
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
                      {track.album || 'Unknown Album'}
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
    </div>
  {/if}
{/if}

<style>
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .artist-list-container {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .artist-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    border: 1px solid transparent;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .artist-row:hover {
    background: rgba(255, 255, 255, 0.02);
    border-color: var(--border-color);
  }

  .artist-avatar-wrapper {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .artist-row-info {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .artist-row-name {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .artist-row-meta {
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .mini-album-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 1rem;
  }

  .mini-album-card {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-width: 0;
  }

  .mini-album-artwork {
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

  .mini-album-artwork img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    position: absolute;
    top: 0;
    left: 0;
  }

  .mini-album-title {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: center;
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
