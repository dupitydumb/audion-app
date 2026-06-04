<script lang="ts">
  import { onMount } from 'svelte';
  import { 
    LayoutDashboard, 
    UploadCloud, 
    KeyRound, 
    BookOpen, 
    Settings as SettingsIcon, 
    Library as LibraryIcon, 
    Music, 
    Play, 
    Pause, 
    Volume2, 
    VolumeX,
    LogOut,
    Menu,
    Disc,
    Users,
    ListMusic,
    Heart
  } from '@lucide/svelte';

  // Import components
  import Login from './components/Login.svelte';
  import Dashboard from './components/Dashboard.svelte';
  import Upload from './components/Upload.svelte';
  import Connection from './components/Connection.svelte';
  import GettingStarted from './components/GettingStarted.svelte';
  import Settings from './components/Settings.svelte';
  import Library from './components/Library.svelte';
  import Playlists from './components/Playlists.svelte';
  import Albums from './components/Albums.svelte';
  import Artists from './components/Artists.svelte';
  import Liked from './components/Liked.svelte';
  
  import { configureUploadQueue, summarizeQueue, uploadQueue } from './stores/uploadQueue';

  // Authentication State
  let token = $state(localStorage.getItem('audion_admin_token') || '');
  let username = $state(localStorage.getItem('audion_admin_username') || '');
  let isLoggedIn = $derived(!!token);

  // Layout State
  let activeTab = $state('dashboard');
  let sidebarOpen = $state(false);

  // Toast Notifications
  interface Toast {
    id: number;
    message: string;
    type: 'success' | 'error' | 'info';
  }
  let toasts = $state<Toast[]>([]);
  let toastId = 0;

  function addToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
    const id = toastId++;
    toasts = [...toasts, { id, message, type }];
    setTimeout(() => {
      toasts = toasts.filter(t => t.id !== id);
    }, 4000);
  }

  let uploadSummary = $derived(summarizeQueue($uploadQueue));
  let uploadCompletion = $derived(
    uploadSummary.total
      ? Math.round(((uploadSummary.success + uploadSummary.duplicate + uploadSummary.error) / uploadSummary.total) * 100)
      : 0
  );

  $effect(() => {
    if (typeof document !== 'undefined') {
      document.documentElement.style.setProperty(
        '--player-height',
        playingTrack ? '96px' : '0px'
      );
    }
  });

  $effect(() => {
    configureUploadQueue({ token, addToast });
  });

  // Global Liked Tracks State for real-time synchronization
  let likedTrackIds = $state<number[]>([]);
  async function fetchLikedTrackIds() {
    if (!token) return;
    try {
      const res = await fetch('/api/liked', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        const data = await res.json();
        likedTrackIds = data.map((t: any) => t.id);
      }
    } catch (e) {
      // Ignore
    }
  }

  async function toggleLike(trackId: number) {
    if (!token) return;
    const isCurrentlyLiked = likedTrackIds.includes(trackId);
    try {
      const method = isCurrentlyLiked ? 'DELETE' : 'POST';
      const res = await fetch(`/api/liked/${trackId}`, {
        method,
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        if (isCurrentlyLiked) {
          likedTrackIds = likedTrackIds.filter(id => id !== trackId);
          addToast('Removed from Liked Tracks', 'info');
        } else {
          likedTrackIds = [...likedTrackIds, trackId];
          addToast('Added to Liked Tracks', 'success');
        }
      } else {
        throw new Error();
      }
    } catch (err) {
      addToast('Failed to update liked status', 'error');
    }
  }

  $effect(() => {
    if (token) {
      fetchLikedTrackIds();
    }
  });

  // Audio Player State
  interface PlayingTrack {
    id: number;
    title: string;
    artist: string;
  }
  let playingTrack = $state<PlayingTrack | null>(null);
  let isPlaying = $state(false);
  let duration = $state(0);
  let currentTime = $state(0);
  let volume = $state(0.8);
  let isMuted = $state(false);
  let audioRef = $state<HTMLAudioElement | null>(null);
  let coverFailed = $state(false);

  $effect(() => {
    if (playingTrack) {
      coverFailed = false;
    }
  });

  function handleLoginSuccess(newToken: string, newUsername: string) {
    token = newToken;
    username = newUsername;
    localStorage.setItem('audion_admin_token', newToken);
    localStorage.setItem('audion_admin_username', newUsername);
    activeTab = 'dashboard';
  }

  function handleLogout() {
    token = '';
    username = '';
    localStorage.removeItem('audion_admin_token');
    localStorage.removeItem('audion_admin_username');
    // Stop audio
    if (audioRef) {
      audioRef.pause();
    }
    playingTrack = null;
    isPlaying = false;
    likedTrackIds = [];
    addToast('Logged out successfully', 'info');
  }

  function handlePlayTrack(track: PlayingTrack) {
    if (playingTrack?.id === track.id) {
      // Toggle
      if (isPlaying) {
        audioRef?.pause();
        isPlaying = false;
      } else {
        audioRef?.play().catch(err => addToast('Playback failed', 'error'));
        isPlaying = true;
      }
    } else {
      playingTrack = track;
      isPlaying = true;
      currentTime = 0;
      // Wait for audio src change
      setTimeout(() => {
        audioRef?.load();
        audioRef?.play().catch(err => addToast('Playback failed', 'error'));
      }, 50);
    }
  }

  function togglePlay() {
    if (!audioRef) return;
    if (isPlaying) {
      audioRef.pause();
      isPlaying = false;
    } else {
      audioRef.play().catch(err => addToast('Playback failed', 'error'));
      isPlaying = true;
    }
  }

  function handleTimeUpdate() {
    if (audioRef) {
      currentTime = audioRef.currentTime;
    }
  }

  function handleLoadedMetadata() {
    if (audioRef) {
      duration = audioRef.duration;
    }
  }

  // Auto-play next track if we can, or just stop
  function handleAudioEnded() {
    isPlaying = false;
    currentTime = 0;
  }

  function handleVolumeChange(e: Event) {
    const target = e.target as HTMLInputElement;
    volume = parseFloat(target.value);
    if (audioRef) {
      audioRef.volume = volume;
      isMuted = volume === 0;
    }
  }

  function toggleMute() {
    isMuted = !isMuted;
    if (audioRef) {
      audioRef.muted = isMuted;
    }
  }

  function handleProgressClick(e: MouseEvent) {
    if (!audioRef || duration === 0) return;
    const rect = (e.currentTarget as HTMLDivElement).getBoundingClientRect();
    const clickX = e.clientX - rect.left;
    const width = rect.width;
    const newPercentage = clickX / width;
    audioRef.currentTime = newPercentage * duration;
  }

  function formatTime(secs: number): string {
    if (isNaN(secs)) return '0:00';
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60).toString().padStart(2, '0');
    return `${m}:${s}`;
  }

  function getActiveTabLabel() {
    switch (activeTab) {
      case 'dashboard':
        return 'Dashboard';
      case 'upload':
        return 'Upload Music';
      case 'library':
        return 'Library Manager';
      case 'albums':
        return 'Albums';
      case 'artists':
        return 'Artists';
      case 'playlists':
        return 'Playlists';
      case 'liked':
        return 'Liked Tracks';
      case 'connection':
        return 'API & Credentials';
      case 'started':
        return 'Getting Started';
      case 'settings':
        return 'Settings';
      default:
        return 'Dashboard';
    }
  }
</script>

{#if !isLoggedIn}
  <Login onLoginSuccess={handleLoginSuccess} {addToast} />
{:else}
  <div class="app-container">
    <!-- Sidebar Navigation -->
    <aside class="sidebar {sidebarOpen ? 'is-open' : ''}">
      <div class="brand-section">
        <div class="brand-logo">
          <Music size={16} />
        </div>
        <span class="brand-name">Audion</span>
      </div>

      <nav class="nav-links">
        <button 
          onclick={() => { activeTab = 'dashboard'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'dashboard' ? 'active' : ''}"
        >
          <LayoutDashboard size={18} />
          <span class="nav-text">Dashboard</span>
        </button>

        <button 
          onclick={() => { activeTab = 'upload'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'upload' ? 'active' : ''}"
        >
          <UploadCloud size={18} />
          <span class="nav-text">Upload Music</span>
        </button>

        <button 
          onclick={() => { activeTab = 'library'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'library' ? 'active' : ''}"
        >
          <LibraryIcon size={18} />
          <span class="nav-text">Library Manager</span>
        </button>

        <button 
          onclick={() => { activeTab = 'albums'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'albums' ? 'active' : ''}"
        >
          <Disc size={18} />
          <span class="nav-text">Albums</span>
        </button>

        <button 
          onclick={() => { activeTab = 'artists'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'artists' ? 'active' : ''}"
        >
          <Users size={18} />
          <span class="nav-text">Artists</span>
        </button>

        <button 
          onclick={() => { activeTab = 'playlists'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'playlists' ? 'active' : ''}"
        >
          <ListMusic size={18} />
          <span class="nav-text">Playlists</span>
        </button>

        <button 
          onclick={() => { activeTab = 'liked'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'liked' ? 'active' : ''}"
        >
          <Heart size={18} />
          <span class="nav-text">Liked Tracks</span>
        </button>

        <div style="height: 1px; background: var(--border-color); margin: 0.5rem 0;"></div>

        <button 
          onclick={() => { activeTab = 'connection'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'connection' ? 'active' : ''}"
        >
          <KeyRound size={18} />
          <span class="nav-text">API & Credentials</span>
        </button>

        <button 
          onclick={() => { activeTab = 'started'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'started' ? 'active' : ''}"
        >
          <BookOpen size={18} />
          <span class="nav-text">Getting Started</span>
        </button>

        <button 
          onclick={() => { activeTab = 'settings'; sidebarOpen = false; }} 
          class="nav-item {activeTab === 'settings' ? 'active' : ''}"
        >
          <SettingsIcon size={18} />
          <span class="nav-text">Settings</span>
        </button>
      </nav>

      {#if uploadSummary.total > 0}
        <div class="glass-card" style="margin: 1rem 0; padding: 0.9rem; display: flex; flex-direction: column; gap: 0.6rem;">
          <div style="display: flex; justify-content: space-between; align-items: center;">
            <span style="font-size: 0.8rem; font-weight: 600;">Upload Status</span>
            <span style="font-size: 0.75rem; color: var(--text-secondary);">{uploadCompletion}%</span>
          </div>
          <div style="height: 4px; background: rgba(255,255,255,0.06); border-radius: 999px; overflow: hidden;">
            <div style="height: 100%; width: {uploadCompletion}%; background: #ffffff;"></div>
          </div>
          <div style="display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.35rem; font-size: 0.7rem; color: var(--text-secondary);">
            <span>Uploading: {uploadSummary.uploading}</span>
            <span>Pending: {uploadSummary.pending}</span>
            <span>Success: {uploadSummary.success}</span>
            <span>Failed: {uploadSummary.error}</span>
          </div>
          <button onclick={() => activeTab = 'upload'} class="btn btn-secondary" style="padding: 0.35rem 0.5rem; font-size: 0.75rem;">
            View Uploads
          </button>
        </div>
      {/if}

      <div class="sidebar-footer">
        <div class="user-badge">
          <div class="user-avatar">
            {username ? username.substring(0, 2).toUpperCase() : 'AD'}
          </div>
          <span style="font-weight: 500;">{username}</span>
        </div>
        <button onclick={handleLogout} class="btn btn-secondary" style="font-size: 0.8rem; padding: 0.45rem; width: 100%; display: flex; gap: 0.5rem; justify-content: center; align-items: center;">
          <LogOut size={14} /> Log Out
        </button>
      </div>
    </aside>

    <div class="sidebar-overlay {sidebarOpen ? 'show' : ''}" onclick={() => sidebarOpen = false}></div>

    <!-- Main Content Area -->
    <main class="main-content" style="margin-bottom: var(--player-height);">
      <div class="mobile-topbar">
        <button class="icon-button" onclick={() => sidebarOpen = !sidebarOpen} aria-label="Toggle navigation">
          <Menu size={18} />
        </button>
        <span class="mobile-title">{getActiveTabLabel()}</span>
        <div style="width: 40px;"></div>
      </div>
      {#if activeTab === 'dashboard'}
        <Dashboard {token} setActiveTab={(tab) => activeTab = tab} {addToast} />
      {:else if activeTab === 'upload'}
        <Upload />
      {:else if activeTab === 'library'}
        <Library 
          {token} 
          currentPlayingId={playingTrack?.id || null} 
          {isPlaying}
          {likedTrackIds}
          onPlayTrack={handlePlayTrack} 
          onToggleLike={toggleLike}
          {addToast} 
        />
      {:else if activeTab === 'albums'}
        <Albums 
          {token} 
          currentPlayingId={playingTrack?.id || null} 
          {isPlaying}
          {likedTrackIds}
          onPlayTrack={handlePlayTrack} 
          onToggleLike={toggleLike}
          {addToast} 
        />
      {:else if activeTab === 'artists'}
        <Artists 
          {token} 
          currentPlayingId={playingTrack?.id || null} 
          {isPlaying}
          {likedTrackIds}
          onPlayTrack={handlePlayTrack} 
          onToggleLike={toggleLike}
          {addToast} 
        />
      {:else if activeTab === 'playlists'}
        <Playlists 
          {token} 
          currentPlayingId={playingTrack?.id || null} 
          {isPlaying}
          onPlayTrack={handlePlayTrack} 
          {addToast} 
        />
      {:else if activeTab === 'liked'}
        <Liked 
          {token} 
          currentPlayingId={playingTrack?.id || null} 
          {isPlaying}
          {likedTrackIds}
          onPlayTrack={handlePlayTrack} 
          onToggleLike={toggleLike}
          {addToast} 
        />
      {:else if activeTab === 'connection'}
        <Connection {token} {addToast} />
      {:else if activeTab === 'started'}
        <GettingStarted {addToast} />
      {:else if activeTab === 'settings'}
        <Settings {username} onLogout={handleLogout} />
      {/if}
    </main>

    <!-- Global Audio Player for preview -->
    {#if playingTrack}
      <div class="mini-player">
        <div class="mini-player-info">
          <div class="mini-player-cover">
            {#if !coverFailed}
              <img 
                src="/api/tracks/{playingTrack.id}/cover?token={token}" 
                alt={playingTrack.title}
                onerror={() => coverFailed = true}
              />
            {:else}
              <Music size={18} style="color: var(--text-secondary);" />
            {/if}
          </div>
          <div class="mini-player-text">
            <div class="mini-player-title">{playingTrack.title}</div>
            <div class="mini-player-artist">{playingTrack.artist}</div>
          </div>
          <button 
            onclick={() => playingTrack && toggleLike(playingTrack.id)} 
            class="btn" 
            style="background: transparent; border: none; padding: 0.25rem; color: {likedTrackIds.includes(playingTrack.id) ? 'var(--danger)' : 'var(--text-muted)'}; margin-left: 0.5rem;"
            title={likedTrackIds.includes(playingTrack.id) ? 'Unlike track' : 'Like track'}
          >
            <Heart size={16} fill={likedTrackIds.includes(playingTrack.id) ? 'currentColor' : 'none'} />
          </button>
        </div>

        <div class="mini-player-controls">
          <div class="controls-row">
            <button 
              onclick={togglePlay} 
              class="btn" 
              style="background: #ffffff; border: none; border-radius: 50%; width: 36px; height: 36px; display: flex; align-items: center; justify-content: center; color: #000000;"
            >
              {#if isPlaying}
                <Pause size={14} fill="currentColor" />
              {:else}
                <Play size={14} fill="currentColor" style="margin-left: 2px;" />
              {/if}
            </button>
          </div>

          <div class="player-progress-container">
            <span class="player-progress-time">{formatTime(currentTime)}</span>
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="player-progress-bar" onclick={handleProgressClick}>
              <div 
                class="player-progress-fill" 
                style="width: {duration > 0 ? (currentTime / duration) * 100 : 0}%"
              ></div>
            </div>
            <span class="player-progress-time">{formatTime(duration)}</span>
          </div>
        </div>

        <div class="mini-player-volume">
          <button onclick={toggleMute} class="btn" style="background: transparent; border: none; color: var(--text-secondary); padding: 0.25rem;">
            {#if isMuted}
              <VolumeX size={16} />
            {:else}
              <Volume2 size={16} />
            {/if}
          </button>
          <input 
            type="range" 
            min="0" 
            max="1" 
            step="0.01" 
            value={volume} 
            oninput={handleVolumeChange} 
            class="volume-slider" 
          />
        </div>

        <audio 
          bind:this={audioRef}
          src="/api/tracks/{playingTrack.id}/stream?token={token}"
          ontimeupdate={handleTimeUpdate}
          onloadedmetadata={handleLoadedMetadata}
          onended={handleAudioEnded}
          style="display: none;"
        ></audio>
      </div>
    {/if}
  </div>
{/if}

<!-- Toast Notifications -->
<div class="toast-container" role="status" aria-live="polite" aria-atomic="false">
  {#each toasts as toast (toast.id)}
    <div class="toast toast-{toast.type}" role={toast.type === 'error' ? 'alert' : 'status'}>
      <span>{toast.message}</span>
    </div>
  {/each}
</div>
