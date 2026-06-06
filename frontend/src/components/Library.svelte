<script lang="ts">
  import { onMount } from 'svelte';
  import { Play, Pause, Trash2, Search, Music, Disc, AlertCircle, RefreshCw, Heart, Plus, Pencil, MoreVertical } from '@lucide/svelte';

  let { token, currentPlayingId, isPlaying, likedTrackIds, onPlayTrack, onToggleLike, addToast, isMobile, openActionSheet } = $props<{
    token: string;
    currentPlayingId: number | null;
    isPlaying: boolean;
    likedTrackIds: number[];
    onPlayTrack: (track: { id: number; title: string; artist: string; format?: string | null; bitrate?: number | null }, queue?: any[]) => void;
    onToggleLike: (trackId: number) => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
    isMobile: boolean;
    openActionSheet: (track: any, type: 'library' | 'liked' | 'playlist' | 'artist' | 'album', callbacks?: any) => void;
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
    metadata_json?: string | null;
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
  let showFullMetadata = $state(false);
  let isFetchingSingle = $state<'musicbrainz' | 'deezer' | null>(null);

  async function handleSingleFetch(provider: 'musicbrainz' | 'deezer') {
    if (!editingTrack) return;
    isFetchingSingle = provider;
    try {
      const res = await fetch(`/api/tracks/${editingTrack.id}/fetch`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({ provider })
      });

      if (!res.ok) {
        const errorText = await res.text();
        throw new Error(errorText || `Failed to fetch metadata from ${provider}`);
      }

      const updatedTrack = await res.json();
      
      editTitle = updatedTrack.title || '';
      editArtist = updatedTrack.artist || '';
      editAlbum = updatedTrack.album || '';
      editGenre = updatedTrack.genre || '';
      editTrackNumber = updatedTrack.track_number || null;
      editDiscNumber = updatedTrack.disc_number || null;
      
      editingTrack = updatedTrack;
      tracks = tracks.map(t => t.id === updatedTrack.id ? updatedTrack : t);
      
      addToast(`Metadata fetched from ${provider === 'musicbrainz' ? 'MusicBrainz' : 'Deezer'} successfully!`, 'success');
    } catch (err: any) {
      addToast(err.message || 'Failed to fetch metadata', 'error');
    } finally {
      isFetchingSingle = null;
    }
  }

  let parsedMetadata = $derived.by(() => {
    if (!editingTrack || !editingTrack.metadata_json) return [];
    try {
      const obj = JSON.parse(editingTrack.metadata_json);
      return Object.entries(obj).map(([key, val]) => ({ key, value: String(val) }));
    } catch (e) {
      return [];
    }
  });

  function openEditModal(track: Track) {
    editingTrack = track;
    editTitle = track.title || '';
    editArtist = track.artist || '';
    editAlbum = track.album || '';
    editGenre = track.genre || '';
    editTrackNumber = track.track_number || null;
    editDiscNumber = track.disc_number || null;
    showFullMetadata = false;
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

  let selectedTrackIds = $state<number[]>([]);
  let isProcessingBulk = $state(false);
  let showBulkPlaylistDropdown = $state(false);
  let eventSource: EventSource | null = null;
  let bulkProgress = $state<{ current: number; total: number; action: string } | null>(null);

  function connectSSE() {
    if (!token) return;
    if (eventSource) {
      eventSource.close();
    }

    const es = new EventSource(`/api/events?token=${token}`);
    eventSource = es;

    es.addEventListener('bulk.progress', (e: any) => {
      try {
        const data = JSON.parse(e.data).payload;
        bulkProgress = {
          current: data.current,
          total: data.total,
          action: data.action
        };
      } catch (err) {}
    });

    es.addEventListener('bulk.completed', () => {
      bulkProgress = null;
    });

    es.onerror = () => {
      setTimeout(connectSSE, 5000);
    };
  }

  function toggleTrackSelection(trackId: number) {
    if (selectedTrackIds.includes(trackId)) {
      selectedTrackIds = selectedTrackIds.filter(id => id !== trackId);
    } else {
      selectedTrackIds = [...selectedTrackIds, trackId];
    }
  }

  function toggleSelectAll() {
    if (selectedTrackIds.length === tracks.length) {
      selectedTrackIds = [];
    } else {
      selectedTrackIds = tracks.map(t => t.id);
    }
  }

  function clearSelection() {
    selectedTrackIds = [];
    showBulkPlaylistDropdown = false;
  }

  async function handleBulkDelete() {
    if (selectedTrackIds.length === 0) return;
    if (!confirm(`Are you sure you want to delete the ${selectedTrackIds.length} selected tracks?`)) {
      return;
    }
    isProcessingBulk = true;
    try {
      const res = await fetch('/api/tracks/bulk/delete', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({ track_ids: selectedTrackIds })
      });
      if (!res.ok) {
        throw new Error('Failed to delete tracks in bulk');
      }
      addToast(`Successfully deleted ${selectedTrackIds.length} tracks`, 'success');
      clearSelection();
      fetchTracks();
    } catch (err: any) {
      addToast(err.message || 'Bulk delete failed', 'error');
    } finally {
      isProcessingBulk = false;
    }
  }

  async function handleBulkFetch(provider: 'musicbrainz' | 'deezer') {
    if (selectedTrackIds.length === 0) return;
    isProcessingBulk = true;
    try {
      const res = await fetch('/api/tracks/bulk/fetch', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({
          track_ids: selectedTrackIds,
          provider
        })
      });
      if (!res.ok) {
        throw new Error(`Failed to fetch metadata from ${provider}`);
      }
      addToast(`Bulk metadata fetch completed! Refreshing track details...`, 'success');
      clearSelection();
      fetchTracks();
    } catch (err: any) {
      addToast(err.message || 'Bulk fetch failed', 'error');
    } finally {
      isProcessingBulk = false;
    }
  }

  async function handleBulkAddToPlaylist(playlistId: number) {
    if (selectedTrackIds.length === 0) return;
    isProcessingBulk = true;
    try {
      const res = await fetch(`/api/playlists/${playlistId}/tracks/bulk`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({ track_ids: selectedTrackIds })
      });
      if (!res.ok) {
        throw new Error('Failed to add tracks to playlist in bulk');
      }
      addToast(`Successfully added ${selectedTrackIds.length} tracks to playlist`, 'success');
      clearSelection();
    } catch (err: any) {
      addToast(err.message || 'Bulk add to playlist failed', 'error');
    } finally {
      isProcessingBulk = false;
      showBulkPlaylistDropdown = false;
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
    connectSSE();

    const handleGlobalClick = () => {
      openPlaylistDropdownId = null;
      showBulkPlaylistDropdown = false;
    };
    window.addEventListener('click', handleGlobalClick);
    return () => {
      window.removeEventListener('click', handleGlobalClick);
      if (eventSource) {
        eventSource.close();
      }
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
      {#if isMobile}
        <div class="mobile-track-list">
          {#each tracks as track (track.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div 
              class="mobile-track-item" 
              class:active={currentPlayingId === track.id}
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
            >
              <div class="track-thumbnail" style="width: 40px; height: 40px; border-radius: 6px;">
                <img 
                  src="/api/tracks/{track.id}/cover?token={token}" 
                  alt="" 
                  onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
                />
                <Music size={16} style="color: var(--text-muted);" />
              </div>
              
              <div class="mobile-track-item-info">
                <div class="mobile-track-item-title">{track.title || 'Unknown Title'}</div>
                <div class="mobile-track-item-artist">{track.artist || 'Unknown Artist'}</div>
                <div class="mobile-track-item-meta">
                  {#if track.genre}
                    <span class="genre-tag" style="font-size: 0.6rem; padding: 0.05rem 0.25rem; margin-right: 0.25rem;">{track.genre}</span>
                  {/if}
                  <span>{formatDuration(track.duration)}</span>
                </div>
              </div>

              <div class="mobile-track-item-actions" onclick={(e) => e.stopPropagation()}>
                <button 
                  onclick={() => openActionSheet(
                    { id: track.id, title: track.title || 'Unknown Title', artist: track.artist || 'Unknown Artist', genre: track.genre, duration: track.duration },
                    'library',
                    {
                      onDelete: () => handleDelete(track.id, track.title),
                      onEdit: () => openEditModal(track)
                    }
                  )}
                  class="mobile-action-trigger"
                  aria-label="Track actions"
                >
                  <MoreVertical size={18} />
                </button>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        {#if tracks.length > 0 && selectedTrackIds.length === 0}
          <div class="bulk-hint" style="display: flex; align-items: center; gap: 0.5rem; padding: 0.5rem 1rem; margin-bottom: 1rem; background: rgba(255, 255, 255, 0.02); border: 1px solid var(--border-color); border-radius: 8px; font-size: 0.8rem; color: var(--text-muted);">
            <AlertCircle size={14} />
            <span>Select tracks using the checkboxes to perform bulk actions like metadata fetching, playlist adding, or deletion.</span>
          </div>
        {/if}
        <div style="overflow-x: auto;">
          <table class="library-table" style="font-size: 0.95rem;">
            <thead>
              <tr>
                <th style="width: 40px; text-align: center; vertical-align: middle;">
                  <input 
                    type="checkbox" 
                    checked={tracks.length > 0 && selectedTrackIds.length === tracks.length}
                    onchange={toggleSelectAll}
                    style="cursor: pointer; accent-color: var(--accent); scale: 1.1;"
                  />
                </th>
                <th style="width: 50px;"></th>
                <th>Title</th>
                <th>Artist</th>
                <th class="hide-mobile">Album</th>
                <th style="width: 80px; text-align: right;">Length</th>
                <th style="width: 60px; text-align: center;" class="hide-mobile">Format</th>
                <th style="width: 120px; text-align: center;">Actions</th>
              </tr>
            </thead>
            <tbody>
              {#each tracks as track (track.id)}
                <tr>
                  <td style="text-align: center; vertical-align: middle;">
                    <input 
                      type="checkbox" 
                      checked={selectedTrackIds.includes(track.id)}
                      onchange={() => toggleTrackSelection(track.id)}
                      style="cursor: pointer; accent-color: var(--accent); scale: 1.1;"
                    />
                  </td>
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
                  <td class="hide-mobile" style="color: var(--text-secondary); max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                    <div style="display: flex; align-items: center; gap: 0.4rem;">
                      <Disc size={14} style="color: var(--text-muted);" />
                      <span>{track.album || 'Unknown Album'}</span>
                    </div>
                  </td>
                  <td style="color: var(--text-secondary); text-align: right; font-family: monospace;">{formatDuration(track.duration)}</td>
                  <td class="hide-mobile" style="text-align: center;">
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
        
        {#if parsedMetadata.length > 0}
          <div style="margin-bottom: 1.5rem; border-top: 1px solid var(--border-color); padding-top: 1rem;">
            <button 
              type="button" 
              class="btn" 
              style="width: 100%; background: rgba(255,255,255,0.03); border: 1px solid var(--border-color); display: flex; justify-content: space-between; align-items: center; padding: 0.5rem 0.75rem; border-radius: 4px; font-size: 0.85rem; color: var(--text-secondary);"
              onclick={() => showFullMetadata = !showFullMetadata}
            >
              <span>All Audio Metadata ({parsedMetadata.length} fields)</span>
              <span>{showFullMetadata ? '▼' : '▶'}</span>
            </button>
            
            {#if showFullMetadata}
              <div class="metadata-scroll-container" style="max-height: 150px; overflow-y: auto; margin-top: 0.5rem; border: 1px solid var(--border-color); border-radius: 4px; background: rgba(0,0,0,0.2); font-size: 0.8rem; font-family: monospace;">
                <table style="width: 100%; border-collapse: collapse; text-align: left;">
                  <tbody>
                    {#each parsedMetadata as item}
                      <tr style="border-bottom: 1px solid rgba(255,255,255,0.03);">
                        <td style="padding: 0.4rem 0.6rem; color: var(--text-muted); font-weight: 600; width: 40%; vertical-align: top; word-break: break-all;">{item.key}</td>
                        <td style="padding: 0.4rem 0.6rem; color: var(--text-primary); vertical-align: top; word-break: break-all;">{item.value}</td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
          </div>
        {/if}

        <div style="display: flex; justify-content: space-between; align-items: center; gap: 0.75rem; margin-top: 1.5rem; border-top: 1px solid var(--border-color); padding-top: 1.25rem;">
          <div style="display: flex; gap: 0.5rem;">
            <button type="button" class="btn btn-secondary" style="font-size: 0.8rem; padding: 0.45rem 0.75rem;" onclick={() => handleSingleFetch('musicbrainz')} disabled={isSavingMetadata || !!isFetchingSingle}>
              {#if isFetchingSingle === 'musicbrainz'}
                <RefreshCw size={12} class="animate-spin" style="animation: spin 1s linear infinite;" /> Fetching MB...
              {:else}
                Fetch MusicBrainz
              {/if}
            </button>
            <button type="button" class="btn btn-secondary" style="font-size: 0.8rem; padding: 0.45rem 0.75rem;" onclick={() => handleSingleFetch('deezer')} disabled={isSavingMetadata || !!isFetchingSingle}>
              {#if isFetchingSingle === 'deezer'}
                <RefreshCw size={12} class="animate-spin" style="animation: spin 1s linear infinite;" /> Fetching Dz...
              {:else}
                Fetch Deezer
              {/if}
            </button>
          </div>
          <div style="display: flex; gap: 0.75rem;">
            <button type="button" class="btn btn-secondary" onclick={() => showEditModal = false} disabled={isSavingMetadata || !!isFetchingSingle}>
              Cancel
            </button>
            <button type="submit" class="btn btn-primary" style="display: flex; gap: 0.5rem; align-items: center;" disabled={isSavingMetadata || !!isFetchingSingle}>
              {#if isSavingMetadata}
                <RefreshCw size={14} class="animate-spin" style="animation: spin 1s linear infinite;" /> Saving...
              {:else}
                Save Changes
              {/if}
            </button>
          </div>
        </div>
      </form>
    </div>
  </div>
{/if}

{#if selectedTrackIds.length > 0}
  <div class="bulk-bar-container">
    <div class="bulk-bar-info">
      <span>Selected</span>
      <span class="bulk-bar-badge">{selectedTrackIds.length}</span>
    </div>
    <div class="bulk-actions-group" style="flex: 1; display: flex; align-items: center; justify-content: space-between;">
      {#if isProcessingBulk}
        <div class="bulk-progress-container">
          <div class="bulk-progress-text">
            <RefreshCw size={14} class="animate-spin" style="animation: spin 1s linear infinite; margin-right: 0.5rem;" />
            {#if bulkProgress}
              {bulkProgress.action === 'delete' ? 'Deleting tracks' : bulkProgress.action === 'playlist' ? 'Adding to playlist' : 'Fetching metadata'} ({bulkProgress.current}/{bulkProgress.total})
            {:else}
              Processing bulk actions...
            {/if}
          </div>
          <div class="bulk-progress-bar-bg">
            <div class="bulk-progress-bar" style="width: {bulkProgress ? (bulkProgress.current / bulkProgress.total) * 100 : 0}%"></div>
          </div>
        </div>
      {:else}
        <div style="display: flex; gap: 0.75rem; align-items: center;">
          <button 
            type="button" 
            class="btn-bulk" 
            onclick={() => handleBulkFetch('musicbrainz')} 
          >
            <RefreshCw size={14} style="margin-right: 0.2rem;" />
            Fetch MB
          </button>
          <button 
            type="button" 
            class="btn-bulk" 
            onclick={() => handleBulkFetch('deezer')} 
          >
            <RefreshCw size={14} style="margin-right: 0.2rem;" />
            Fetch Deezer
          </button>
          
          <div style="position: relative; display: inline-block;">
            <button 
              type="button" 
              class="btn-bulk" 
              onclick={(e) => { e.stopPropagation(); showBulkPlaylistDropdown = !showBulkPlaylistDropdown; }} 
            >
              <Plus size={14} style="margin-right: 0.2rem;" />
              Add to Playlist
            </button>
            {#if showBulkPlaylistDropdown}
              <div class="bulk-playlist-menu glass-card">
                {#if playlists.length === 0}
                  <div style="padding: 0.5rem; font-size: 0.75rem; color: var(--text-muted); text-align: center;">No playlists.</div>
                {:else}
                  {#each playlists as pl}
                    <button 
                      type="button"
                      onclick={() => handleBulkAddToPlaylist(pl.id)} 
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
            type="button" 
            class="btn-bulk btn-bulk-danger" 
            onclick={handleBulkDelete} 
          >
            <Trash2 size={14} style="margin-right: 0.2rem;" />
            Delete Selected
          </button>
          
          <button 
            type="button" 
            class="btn-bulk" 
            onclick={clearSelection} 
            style="border: none; background: transparent;"
          >
            Cancel
          </button>
        </div>
      {/if}
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

  /* Bulk Actions Floating Bar */
    .bulk-bar-container {
    position: fixed;
    bottom: calc(2rem + var(--player-height));
    left: 50%;
    transform: translateX(-50%);
    z-index: 900;
    display: flex;
    align-items: center;
    gap: 1.25rem;
    padding: 0.85rem 1.75rem;
    border-radius: 100px;
    max-width: calc(100vw - 2rem);
    flex-wrap: wrap;
    justify-content: center;
    background: rgba(18, 18, 22, 0.85);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 10px 40px -10px rgba(0, 0, 0, 0.7), 
                0 0 20px 0 rgba(168, 85, 247, 0.15); /* Purple accent glow */
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .bulk-bar-info {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    border-right: 1px solid rgba(255, 255, 255, 0.1);
    padding-right: 1.25rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .bulk-bar-badge {
    background: var(--accent, #a855f7);
    color: #fff;
    font-size: 0.8rem;
    padding: 0.15rem 0.5rem;
    border-radius: 20px;
    font-weight: 700;
  }

  .bulk-actions-group {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .btn-bulk {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
    font-weight: 500;
    padding: 0.5rem 1.25rem;
    border-radius: 50px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .btn-bulk:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .btn-bulk-danger {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
    border-color: rgba(239, 68, 68, 0.2);
  }

  .btn-bulk-danger:hover:not(:disabled) {
    background: rgba(239, 68, 68, 0.25);
    color: #ef4444;
    border-color: rgba(239, 68, 68, 0.4);
  }

  .bulk-playlist-menu {
    position: absolute;
    bottom: calc(100% + 10px);
    right: 0;
    background: #0d0d0f;
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.35rem;
    min-width: 180px;
    max-height: 200px;
    overflow-y: auto;
    z-index: 950;
    box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.5);
  }

  /* Bulk progress styles */
  .bulk-progress-container {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-width: 250px;
    flex: 1;
    margin-right: 1.5rem;
  }

  .bulk-progress-text {
    font-size: 0.85rem;
    color: var(--text-primary);
    display: flex;
    align-items: center;
  }

  .bulk-progress-bar-bg {
    width: 100%;
    height: 6px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
    overflow: hidden;
  }

  .bulk-progress-bar {
    height: 100%;
    background: var(--accent, #a855f7);
    border-radius: 3px;
    transition: width 0.25s ease-out;
  }
</style>
