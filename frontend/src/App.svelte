<script lang="ts">
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
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
    Heart,
    RefreshCw,
    AlignLeft,
    Maximize2,
    Minimize2,
    Shuffle,
    SkipBack,
    SkipForward,
    Repeat
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

  let scanStatus = $state({
    isScanning: false,
    filesScanned: 0,
    totalFiles: 0,
    currentFile: ''
  });

  let fetcherStatus = $state({
    isRunning: false,
    tracksProcessed: 0,
    totalTracks: 0,
    currentTrack: '',
    logs: [] as string[]
  });

  async function fetchLibraryStatuses() {
    if (!token) return;
    try {
      const scanRes = await fetch('/api/library/scan-status', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (scanRes.ok) {
        const data = await scanRes.json();
        scanStatus.isScanning = data.is_scanning;
        scanStatus.filesScanned = data.files_scanned;
        scanStatus.totalFiles = data.total_files;
        scanStatus.currentFile = data.current_file;
      }

      const fetchRes = await fetch('/api/library/fetch-status', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (fetchRes.ok) {
        const data = await fetchRes.json();
        fetcherStatus.isRunning = data.is_running;
        fetcherStatus.tracksProcessed = data.tracks_processed;
        fetcherStatus.totalTracks = data.total_tracks;
        fetcherStatus.currentTrack = data.current_track;
        fetcherStatus.logs = data.logs || [];
      }
    } catch (e) {}
  }

  $effect(() => {
    if (token) {
      fetchLikedTrackIds();
      fetchLibraryStatuses();
    }
  });

  // Audio Player State
  interface PlayingTrack {
    id: number;
    title: string;
    artist: string;
    format?: string | null;
    bitrate?: number | null;
    genre?: string | null;
  }
  let playingTrack = $state<PlayingTrack | null>(null);
  let isPlaying = $state(false);
  let duration = $state(0);
  let currentTime = $state(0);
  let volume = $state(0.8);
  let isMuted = $state(false);
  let audioRef = $state<HTMLAudioElement | null>(null);
  let coverFailed = $state(false);
  let isBuffering = $state(false);

  // Lyrics & Fullscreen State
  let isFullScreen = $state(false);
  let showLyricsPanel = $state(false);
  
  // Playback Queue & SSE States
  let playbackQueue = $state<PlayingTrack[]>([]);
  let currentQueueIndex = $state(-1);
  let isShuffle = $state(false);
  let repeatMode = $state<'off' | 'all' | 'one'>('off');
  let isTranscoding = $state(false);
  let eventSource: EventSource | null = null;

  function connectSSE() {
    if (!token) return;
    if (eventSource) {
      eventSource.close();
    }

    const es = new EventSource(`/api/events?token=${token}`);
    eventSource = es;

    es.addEventListener('track.transcoding', (e: any) => {
      try {
        const data = JSON.parse(e.data);
        const payload = data.payload;
        if (playingTrack && payload.id === playingTrack.id) {
          if (payload.status === 'started') {
            isTranscoding = true;
          } else {
            isTranscoding = false;
          }
        }
      } catch (err) {
        // Ignore
      }
    });

    es.addEventListener('scan.started', (e: any) => {
      try {
        const data = JSON.parse(e.data).payload;
        scanStatus.isScanning = true;
        scanStatus.totalFiles = data.total_files;
        scanStatus.filesScanned = 0;
        scanStatus.currentFile = '';
        addToast('Music folder scan started', 'info');
      } catch (err) {}
    });

    es.addEventListener('scan.progress', (e: any) => {
      try {
        const data = JSON.parse(e.data).payload;
        scanStatus.isScanning = data.is_scanning;
        scanStatus.filesScanned = data.files_scanned;
        scanStatus.totalFiles = data.total_files;
        scanStatus.currentFile = data.current_file;
      } catch (err) {}
    });

    es.addEventListener('scan.completed', (e: any) => {
      try {
        const data = JSON.parse(e.data).payload;
        scanStatus.isScanning = false;
        scanStatus.filesScanned = data.files_scanned;
        scanStatus.totalFiles = data.total_files;
        scanStatus.currentFile = '';
        addToast('Music folder scan completed successfully!', 'success');
      } catch (err) {}
    });

    es.addEventListener('fetch.progress', (e: any) => {
      try {
        const data = JSON.parse(e.data).payload;
        fetcherStatus.isRunning = data.is_running;
        fetcherStatus.tracksProcessed = data.tracks_processed;
        fetcherStatus.totalTracks = data.total_tracks;
        fetcherStatus.currentTrack = data.current_track;
        fetcherStatus.logs = data.logs || [];
      } catch (err) {}
    });

    es.addEventListener('fetch.completed', (e: any) => {
      try {
        const data = JSON.parse(e.data).payload;
        fetcherStatus.isRunning = false;
        fetcherStatus.tracksProcessed = data.tracks_processed;
        fetcherStatus.totalTracks = data.total_tracks;
        fetcherStatus.currentTrack = '';
        fetcherStatus.logs = data.logs || [];
        addToast('Metadata auto-fetcher worker completed!', 'success');
      } catch (err) {}
    });

    es.addEventListener('library.reset', () => {
      addToast('Library has been reset.', 'info');
      if (['dashboard', 'library', 'albums', 'artists', 'playlists', 'liked'].includes(activeTab)) {
        window.location.reload();
      }
    });

    es.onerror = () => {
      setTimeout(connectSSE, 5000);
    };
  }

  $effect(() => {
    if (token) {
      connectSSE();
    }
    return () => {
      if (eventSource) {
        eventSource.close();
        eventSource = null;
      }
    };
  });
  
  interface LyricLine {
    time: number;
    text: string;
  }
  let lyricsLines = $state<LyricLine[]>([]);
  let lyricsLoading = $state(false);
  let currentLyricIndex = $state(-1);
  let lyricsContainerRef = $state<HTMLDivElement | null>(null);
  let fsLyricsContainerRef = $state<HTMLDivElement | null>(null);

  function parseLrc(lrcText: string): LyricLine[] {
    const lines = lrcText.split('\n');
    const result: LyricLine[] = [];
    const timeRegex = /\[(\d+):(\d+(?:\.\d+)?)]/;

    for (const line of lines) {
      const match = timeRegex.exec(line);
      if (match) {
        const minutes = parseInt(match[1], 10);
        const seconds = parseFloat(match[2]);
        const time = minutes * 60 + seconds;
        const text = line.replace(timeRegex, '').trim();
        if (!isNaN(time)) {
          result.push({ time, text });
        }
      } else if (line.trim()) {
        const isTag = line.trim().startsWith('[') && line.trim().includes(':') && !/\d/.test(line);
        if (!isTag) {
          result.push({ time: -1, text: line.trim() });
        }
      }
    }

    const timed = result.filter(r => r.time >= 0).sort((a, b) => a.time - b.time);
    const untimed = result.filter(r => r.time < 0);
    
    if (timed.length > 0) {
      return timed;
    } else {
      return untimed.map((u, idx) => ({ time: idx * 4, text: u.text }));
    }
  }

  async function loadLyrics(track: PlayingTrack) {
    lyricsLines = [];
    lyricsLoading = true;
    currentLyricIndex = -1;

    try {
      const res = await fetch(`/api/tracks/${track.id}/lyrics`, {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        const data = await res.json();
        if (data.lyrics) {
          lyricsLines = parseLrc(data.lyrics);
        }
      }
    } catch (e) {
      console.warn('Failed to load lyrics:', e);
    } finally {
      lyricsLoading = false;
    }
  }

  function syncLyricsTime(time: number) {
    if (lyricsLines.length === 0) return;
    
    let activeIdx = -1;
    for (let i = 0; i < lyricsLines.length; i++) {
      if (time >= lyricsLines[i].time) {
        activeIdx = i;
      } else {
        break;
      }
    }
    
    if (activeIdx !== currentLyricIndex) {
      currentLyricIndex = activeIdx;
      scrollToActiveLine();
    }
  }

  function scrollToActiveLine() {
    if (currentLyricIndex < 0) return;
    setTimeout(() => {
      if (showLyricsPanel && lyricsContainerRef) {
        const activeEl = lyricsContainerRef.querySelector('.lyric-line.active') as HTMLElement;
        if (activeEl) {
          const containerHeight = lyricsContainerRef.clientHeight;
          const targetScroll = activeEl.offsetTop - containerHeight / 2 + activeEl.clientHeight / 2;
          lyricsContainerRef.scrollTo({ top: targetScroll, behavior: 'smooth' });
        }
      }

      if (isFullScreen && fsLyricsContainerRef) {
        const activeEl = fsLyricsContainerRef.querySelector('.fullscreen-lyric-line.active') as HTMLElement;
        if (activeEl) {
          const containerHeight = fsLyricsContainerRef.clientHeight;
          const targetScroll = activeEl.offsetTop - containerHeight / 2 + activeEl.clientHeight / 2;
          fsLyricsContainerRef.scrollTo({ top: targetScroll, behavior: 'smooth' });
        }
      }
    }, 50);
  }

  function seekToTime(time: number) {
    if (audioRef && duration > 0) {
      audioRef.currentTime = time;
      currentTime = time;
    }
  }

  $effect(() => {
    if (playingTrack) {
      coverFailed = false;
      isBuffering = true;
      loadLyrics(playingTrack);
    }
  });

  function handleLoginSuccess(newToken: string, newUsername: string) {
    token = newToken;
    username = newUsername;
    localStorage.setItem('audion_admin_token', newToken);
    localStorage.setItem('audion_admin_username', newUsername);
    activeTab = 'dashboard';
  }

  function handleProfileUpdate(newToken: string, newUsername: string) {
    token = newToken;
    username = newUsername;
    localStorage.setItem('audion_admin_token', newToken);
    localStorage.setItem('audion_admin_username', newUsername);
  }

  function handleLogout() {
    token = '';
    username = '';
    localStorage.removeItem('audion_admin_token');
    localStorage.removeItem('audion_admin_username');
    if (audioRef) {
      audioRef.pause();
    }
    playingTrack = null;
    isPlaying = false;
    likedTrackIds = [];
    isFullScreen = false;
    showLyricsPanel = false;
    playbackQueue = [];
    currentQueueIndex = -1;
    isTranscoding = false;
    if (eventSource) {
      eventSource.close();
      eventSource = null;
    }
    addToast('Logged out successfully', 'info');
  }

  function handlePlayTrack(track: PlayingTrack, queue?: PlayingTrack[]) {
    if (queue && queue.length > 0) {
      playbackQueue = queue;
      const idx = queue.findIndex(t => t.id === track.id);
      if (idx !== -1) {
        currentQueueIndex = idx;
      } else {
        playbackQueue = [track];
        currentQueueIndex = 0;
      }
    } else {
      const idx = playbackQueue.findIndex(t => t.id === track.id);
      if (idx !== -1) {
        currentQueueIndex = idx;
      } else {
        playbackQueue = [track];
        currentQueueIndex = 0;
      }
    }

    if (playingTrack?.id === track.id) {
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
      isBuffering = true;
      isTranscoding = false;
      setTimeout(() => {
        audioRef?.load();
        audioRef?.play().catch(err => addToast('Playback failed', 'error'));
      }, 50);
    }
  }

  function skipForward() {
    if (playbackQueue.length === 0) return;
    
    let nextIdx = currentQueueIndex;
    if (isShuffle) {
      nextIdx = Math.floor(Math.random() * playbackQueue.length);
    } else {
      nextIdx = (currentQueueIndex + 1) % playbackQueue.length;
    }
    
    const nextTrack = playbackQueue[nextIdx];
    if (nextTrack) {
      currentQueueIndex = nextIdx;
      handlePlayTrack(nextTrack, playbackQueue);
    }
  }

  function skipBackward() {
    if (playbackQueue.length === 0) return;
    
    let prevIdx = currentQueueIndex;
    if (isShuffle) {
      prevIdx = Math.floor(Math.random() * playbackQueue.length);
    } else {
      prevIdx = (currentQueueIndex - 1 + playbackQueue.length) % playbackQueue.length;
    }
    
    const prevTrack = playbackQueue[prevIdx];
    if (prevTrack) {
      currentQueueIndex = prevIdx;
      handlePlayTrack(prevTrack, playbackQueue);
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
      syncLyricsTime(currentTime);
    }
  }

  function handleLoadedMetadata() {
    if (audioRef) {
      duration = audioRef.duration;
      isBuffering = false;
    }
  }

  function handleAudioEnded() {
    if (repeatMode === 'one') {
      if (audioRef) {
        audioRef.currentTime = 0;
        audioRef.play().catch(err => addToast('Playback failed', 'error'));
        isPlaying = true;
      }
    } else {
      if (playbackQueue.length > 0) {
        let nextIdx = currentQueueIndex;
        if (isShuffle) {
          nextIdx = Math.floor(Math.random() * playbackQueue.length);
        } else {
          nextIdx = currentQueueIndex + 1;
        }
        
        if (nextIdx >= playbackQueue.length) {
          if (repeatMode === 'all') {
            nextIdx = 0;
          } else {
            isPlaying = false;
            currentTime = 0;
            return;
          }
        }
        
        const nextTrack = playbackQueue[nextIdx];
        if (nextTrack) {
          currentQueueIndex = nextIdx;
          playingTrack = nextTrack;
          isPlaying = true;
          currentTime = 0;
          isBuffering = true;
          isTranscoding = false;
          setTimeout(() => {
            audioRef?.load();
            audioRef?.play().catch(err => addToast('Playback failed', 'error'));
          }, 50);
        }
      } else {
        isPlaying = false;
        currentTime = 0;
      }
    }
  }

  function handleAudioError(e: Event) {
    const target = e.currentTarget as HTMLAudioElement;
    const error = target.error;
    let message = 'Playback error occurred';
    if (error) {
      switch (error.code) {
        case error.MEDIA_ERR_ABORTED:
          message = 'Playback aborted by user';
          break;
        case error.MEDIA_ERR_NETWORK:
          message = 'Network error during playback';
          break;
        case error.MEDIA_ERR_DECODE:
          message = 'Audio decoding failed (corrupt file or codec issue)';
          break;
        case error.MEDIA_ERR_SRC_NOT_SUPPORTED:
          message = 'Audio format (like FLAC) is not supported by your browser';
          break;
      }
    }
    addToast(message, 'error');
    isPlaying = false;
    isBuffering = false;
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
  <div class="app-container" class:layout-with-lyrics={showLyricsPanel}>
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
          <span class="nav-text" style="display: flex; align-items: center; justify-content: space-between; width: 100%;">
            Library Manager
            {#if scanStatus.isScanning || fetcherStatus.isRunning}
              <RefreshCw size={12} class="animate-spin" style="animation: spin 1s linear infinite; color: var(--accent); margin-left: 0.5rem;" />
            {/if}
          </span>
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
        <Settings 
          {token} 
          {username} 
          {scanStatus} 
          {fetcherStatus} 
          onLogout={handleLogout} 
          {addToast} 
          onProfileUpdate={handleProfileUpdate} 
        />
      {/if}
    </main>

        <!-- Transcoding Status Bar -->
    {#if isTranscoding && playingTrack}
      <div class="transcoding-bar" in:slide={{ y: 20, duration: 300 }}>
        <RefreshCw size={14} class="animate-spin" style="animation: spin 1s linear infinite;" />
        <span>Converting FLAC to MP3 for streaming...</span>
        <span class="transcoding-track">{playingTrack.title}</span>
        <span style="margin-left: auto; font-size: 0.75rem; opacity: 0.7;">This may take a moment</span>
      </div>
    {/if}

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
            <div style="display: flex; align-items: center; gap: 0.5rem; font-size: 0.75rem; color: var(--text-secondary);">
              <span>{playingTrack.artist}</span>
              {#if playingTrack.format}
                <span style="font-size: 0.65rem; text-transform: uppercase; background: rgba(255,255,255,0.06); padding: 0.1rem 0.35rem; border-radius: 4px; border: 1px solid var(--border-color); color: var(--text-secondary); margin-left: 0.25rem; font-weight: 600; font-family: monospace;">
                  {playingTrack.format}
                  {#if playingTrack.bitrate}
                    · {Math.round(playingTrack.bitrate / 1000)}k
                  {/if}
                </span>
              {/if}
              {#if playingTrack.genre}
                <span class="genre-tag" style="margin-left: 0.25rem;">{playingTrack.genre}</span>
              {/if}
              {#if isTranscoding}
                <span class="converting-badge" style="font-size: 0.65rem; text-transform: uppercase; background: rgba(168, 85, 247, 0.15); border: 1px solid rgba(168, 85, 247, 0.3); padding: 0.1rem 0.35rem; border-radius: 4px; color: rgb(216, 180, 254); font-weight: 600; display: inline-flex; align-items: center; gap: 0.25rem;">
                  <RefreshCw size={8} class="animate-spin" style="animation: spin 1s linear infinite;" /> Converting (FFmpeg)
                </span>
              {/if}
            </div>
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
          <div class="controls-row" style="display: flex; align-items: center; gap: 0.75rem; justify-content: center;">
            <button 
              onclick={() => isShuffle = !isShuffle} 
              class="btn" 
              style="background: transparent; border: none; color: {isShuffle ? 'var(--accent)' : 'var(--text-secondary)'}; padding: 0.25rem; display: flex; align-items: center; cursor: pointer;"
              title="Shuffle: {isShuffle ? 'On' : 'Off'}"
            >
              <Shuffle size={14} />
            </button>

            <button 
              onclick={skipBackward} 
              class="btn" 
              style="background: transparent; border: none; color: var(--text-primary); padding: 0.25rem; display: flex; align-items: center; cursor: pointer;"
              title="Previous"
            >
              <SkipBack size={14} fill="currentColor" />
            </button>

            <button 
              onclick={togglePlay} 
              class="btn-play-pause"
              title={isPlaying ? "Pause" : "Play"}
            >
              {#if isBuffering}
                <RefreshCw size={18} class="animate-spin" style="animation: spin 1s linear infinite;" />
              {:else if isPlaying}
                <Pause size={18} fill="currentColor" />
              {:else}
                <Play size={18} fill="currentColor" style="margin-left: 2px;" />
              {/if}
            </button>

            <button 
              onclick={skipForward} 
              class="btn" 
              style="background: transparent; border: none; color: var(--text-primary); padding: 0.25rem; display: flex; align-items: center; cursor: pointer;"
              title="Next"
            >
              <SkipForward size={14} fill="currentColor" />
            </button>

            <button 
              onclick={() => {
                if (repeatMode === 'off') repeatMode = 'all';
                else if (repeatMode === 'all') repeatMode = 'one';
                else repeatMode = 'off';
              }} 
              class="btn" 
              style="background: transparent; border: none; color: {repeatMode !== 'off' ? 'var(--accent)' : 'var(--text-secondary)'}; padding: 0.25rem; display: flex; align-items: center; position: relative; cursor: pointer;"
              title="Repeat: {repeatMode}"
            >
              <Repeat size={14} />
              {#if repeatMode === 'one'}
                <span style="position: absolute; font-size: 7px; font-weight: bold; background: var(--accent); color: #000000; border-radius: 50%; width: 9px; height: 9px; display: flex; align-items: center; justify-content: center; bottom: -2px; right: -2px;">1</span>
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
          <button 
            onclick={() => showLyricsPanel = !showLyricsPanel} 
            class="btn" 
            style="background: transparent; border: none; color: {showLyricsPanel ? '#ffffff' : 'var(--text-secondary)'}; padding: 0.25rem; margin-right: 0.5rem; display: flex; align-items: center;"
            title="Toggle lyrics panel"
          >
            <AlignLeft size={16} />
          </button>

          <button 
            onclick={() => isFullScreen = true} 
            class="btn" 
            style="background: transparent; border: none; color: var(--text-secondary); padding: 0.25rem; margin-right: 0.75rem; display: flex; align-items: center;"
            title="Enter fullscreen"
          >
            <Maximize2 size={16} />
          </button>

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
          onerror={handleAudioError}
          onwaiting={() => isBuffering = true}
          onplaying={() => isBuffering = false}
          onloadeddata={() => isBuffering = false}
          onseeked={() => isBuffering = false}
          style="display: none;"
        ></audio>
      </div>
    {/if}

    <!-- Lyrics Sidebar -->
    {#if showLyricsPanel && playingTrack}
      <aside class="lyrics-sidebar">
        <div class="lyrics-header">
          <span class="lyrics-title">Lyrics</span>
          <button 
            onclick={() => showLyricsPanel = false} 
            class="btn btn-secondary" 
            style="padding: 0.25rem 0.5rem; font-size: 0.8rem;"
          >
            Close
          </button>
        </div>
        <div class="lyrics-body" bind:this={lyricsContainerRef}>
          {#if lyricsLoading}
            <div class="lyrics-empty">
              <RefreshCw size={24} class="animate-spin" style="animation: spin 1s linear infinite;" />
              <span>Loading lyrics...</span>
            </div>
          {:else if lyricsLines.length > 0}
            {#each lyricsLines as line, i}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div 
                class="lyric-line-wrapper"
                onclick={() => seekToTime(line.time)}
              >
                <div class="lyric-line" class:active={i === currentLyricIndex}>
                  {line.text || '• • •'}
                </div>
              </div>
            {/each}
          {:else}
            <div class="lyrics-empty">
              <span>No lyrics found</span>
            </div>
          {/if}
        </div>
      </aside>
    {/if}
  </div>
{/if}

<!-- Fullscreen Player Overlay -->
{#if isFullScreen && playingTrack}
  <div class="fullscreen-player">
    <div class="bg-canvas">
      <div 
        class="bg-layer active" 
        style="background-image: url('/api/tracks/{playingTrack.id}/cover?token={token}')"
      ></div>
    </div>
    <div class="backdrop-layer"></div>

    <button 
      class="fullscreen-close-btn"
      onclick={() => isFullScreen = false}
      title="Exit fullscreen"
    >
      <Minimize2 size={20} />
    </button>

    <div class="fullscreen-content">
      <div class="fullscreen-left">
        <div class="fullscreen-cover">
          {#if !coverFailed}
            <img 
              src="/api/tracks/{playingTrack.id}/cover?token={token}" 
              alt={playingTrack.title}
              onerror={() => coverFailed = true}
            />
          {:else}
            <div class="fullscreen-cover-placeholder">
              <Music size={128} />
            </div>
          {/if}
        </div>

        <div class="fullscreen-meta">
          <div class="fullscreen-title">{playingTrack.title}</div>
          <div style="display: flex; align-items: center; gap: 0.75rem; justify-content: center; margin-top: 0.25rem;">
            <div class="fullscreen-artist" style="margin: 0;">{playingTrack.artist}</div>
            {#if playingTrack.genre}
              <span class="genre-tag" style="background: rgba(255, 255, 255, 0.08); border-color: rgba(255, 255, 255, 0.15); color: rgba(255, 255, 255, 0.85);">{playingTrack.genre}</span>
            {/if}
                        {#if isTranscoding}
              <div class="fullscreen-transcoding">
                <RefreshCw size={14} class="animate-spin" style="animation: spin 1s linear infinite;" />
                <span>Converting FLAC to MP3 via FFmpeg...</span>
              </div>
            {/if}
          </div>
        </div>

        <div class="fullscreen-controls-panel">
          <div class="fullscreen-progress-row">
            <span class="fullscreen-time">{formatTime(currentTime)}</span>
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="fullscreen-progress-bar" onclick={handleProgressClick}>
              <div 
                class="fullscreen-progress-fill" 
                style="width: {duration > 0 ? (currentTime / duration) * 100 : 0}%"
              ></div>
            </div>
            <span class="fullscreen-time">{formatTime(duration)}</span>
          </div>

          <div class="fullscreen-buttons-row" style="display: flex; align-items: center; gap: 1.5rem; justify-content: center;">
            <button 
              onclick={() => isShuffle = !isShuffle} 
              class="fullscreen-btn" 
              style="color: {isShuffle ? 'var(--accent)' : 'rgba(255,255,255,0.6)'};"
              title="Shuffle: {isShuffle ? 'On' : 'Off'}"
            >
              <Shuffle size={20} />
            </button>

            <button 
              onclick={skipBackward} 
              class="fullscreen-btn"
              title="Previous"
            >
              <SkipBack size={22} fill="currentColor" />
            </button>

            <button 
              onclick={togglePlay}
              class="fullscreen-btn fullscreen-btn-play"
              title={isPlaying ? "Pause" : "Play"}
            >
              {#if isBuffering}
                <RefreshCw size={22} class="animate-spin" style="animation: spin 1s linear infinite;" />
              {:else if isPlaying}
                <Pause size={22} fill="currentColor" />
              {:else}
                <Play size={22} fill="currentColor" style="margin-left: 3px;" />
              {/if}
            </button>

            <button 
              onclick={skipForward} 
              class="fullscreen-btn"
              title="Next"
            >
              <SkipForward size={22} fill="currentColor" />
            </button>

            <button 
              onclick={() => {
                if (repeatMode === 'off') repeatMode = 'all';
                else if (repeatMode === 'all') repeatMode = 'one';
                else repeatMode = 'off';
              }} 
              class="fullscreen-btn" 
              style="color: {repeatMode !== 'off' ? 'var(--accent)' : 'rgba(255,255,255,0.6)'}; position: relative;"
              title="Repeat: {repeatMode}"
            >
              <Repeat size={20} />
              {#if repeatMode === 'one'}
                <span style="position: absolute; font-size: 7px; font-weight: bold; background: var(--accent); color: #000000; border-radius: 50%; width: 9px; height: 9px; display: flex; align-items: center; justify-content: center; bottom: -2px; right: -2px;">1</span>
              {/if}
            </button>

            <button 
              onclick={() => playingTrack && toggleLike(playingTrack.id)}
              class="fullscreen-btn"
              style="color: {likedTrackIds.includes(playingTrack.id) ? 'var(--danger)' : 'rgba(255,255,255,0.6)'};"
              title="Like track"
            >
              <Heart size={20} fill={likedTrackIds.includes(playingTrack.id) ? 'currentColor' : 'none'} />
            </button>
          </div>

          <div class="fullscreen-volume-row">
            <button onclick={toggleMute} class="fullscreen-btn">
              {#if isMuted}
                <VolumeX size={18} />
              {:else}
                <Volume2 size={18} />
              {/if}
            </button>
            <input 
              type="range" 
              min="0" 
              max="1" 
              step="0.01" 
              value={volume} 
              oninput={handleVolumeChange} 
              class="fullscreen-volume-slider" 
            />
          </div>
        </div>
      </div>

      <div class="fullscreen-right">
        <div class="fullscreen-lyrics-container" bind:this={fsLyricsContainerRef}>
          {#if lyricsLoading}
            <div class="lyrics-empty">
              <RefreshCw size={36} class="animate-spin" style="animation: spin 1s linear infinite;" />
              <span style="font-size: 1.25rem;">Loading lyrics...</span>
            </div>
          {:else if lyricsLines.length > 0}
            {#each lyricsLines as line, i}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div 
                class="fullscreen-lyric-line-wrapper"
                onclick={() => seekToTime(line.time)}
              >
                <div class="fullscreen-lyric-line" class:active={i === currentLyricIndex}>
                  {line.text || '• • •'}
                </div>
              </div>
            {/each}
          {:else}
            <div class="lyrics-empty">
              <span style="font-size: 1.5rem;">No lyrics found</span>
            </div>
          {/if}
        </div>
      </div>
    </div>
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
