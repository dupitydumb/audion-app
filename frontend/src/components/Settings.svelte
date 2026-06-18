<script lang="ts">
  import { Shield, Database, Key, HelpCircle, LogOut, RefreshCw, FolderSync, Trash2, User, Users, Cpu, FileText, CheckCircle2, AlertTriangle, Edit2, UserPlus, Check, X, Globe, Copy, ExternalLink, Lock, Search, HardDrive, UploadCloud, Ban, Infinity } from '@lucide/svelte';

  // Props in Svelte 5
  let { token, username, role, listenbrainzToken, scanStatus, fetcherStatus, onLogout, addToast, onProfileUpdate } = $props<{
    token: string;
    username: string;
    role: string;
    listenbrainzToken: string;
    scanStatus: { isScanning: boolean; filesScanned: number; totalFiles: number; currentFile: string | null };
    fetcherStatus: { isRunning: boolean; tracksProcessed: number; totalTracks: number; currentTrack: string | null; logs: string[] };
    onLogout: () => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
    onProfileUpdate: (newToken: string, newUsername: string, newRole: string, newLbToken: string) => void;
  }>();

  let activeTab = $state<'profile' | 'users' | 'library' | 'tunnel' | 'system' | 'storage'>('profile');

  // Profile Form States
  let currentPassword = $state('');
  let newUsername = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let isSavingProfile = $state(false);

  // User search and stats state
  let usersList = $state<any[]>([]);
  let userSearchQuery = $state('');
  let filteredUsers = $derived(
    usersList.filter(u => 
      u.username.toLowerCase().includes(userSearchQuery.toLowerCase()) ||
      (u.role || '').toLowerCase().includes(userSearchQuery.toLowerCase())
    )
  );

  let adminStats = $state<any[]>([]);
  let isLoadingAdminStats = $state(false);

  async function fetchAdminStats() {
    if (role !== 'Admin') return;
    isLoadingAdminStats = true;
    try {
      const res = await fetch('/api/admin/stats', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        adminStats = await res.json();
      }
    } catch (e) {
      console.error('Failed to fetch admin stats', e);
    } finally {
      isLoadingAdminStats = false;
    }
  }

  function formatDateTime(dateTimeStr: string | null | undefined): string {
    if (!dateTimeStr) return 'Unknown';
    try {
      const parts = dateTimeStr.split(' ');
      if (parts.length > 0) {
        return parts[0];
      }
      return dateTimeStr;
    } catch (e) {
      return dateTimeStr;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }


  // Library Wiping Safeguards
  let resetCheck = $state(false);
  let resetText = $state('');
  let isResetting = $state(false);
  let isCleaning = $state(false);
  let fetcherProvider = $state<'deezer' | 'musicbrainz'>('deezer');

  const envs = [
    { name: 'AUDION_ADMIN_USER', desc: 'The administrator username used to access this web UI and sync.', default: 'admin' },
    { name: 'AUDION_ADMIN_PASSWORD', desc: 'The administrator password used to log in.', default: 'changeme' },
    { name: 'AUDION_JWT_SECRET', desc: 'Secret signature key used to encode and sign JWT access tokens.', default: 'your-secret-key-here-change-this-in-production' },
    { name: 'AUDION_DATA_DIR', desc: 'Host storage directory where database (sqlite) and music files are stored.', default: '/data' },
    { name: 'AUDION_PORT', desc: 'The TCP port the server application binds to.', default: '8080' }
  ];

  // Actions
  async function handleUpdateProfile(e: SubmitEvent) {
    e.preventDefault();
    if (!currentPassword) {
      addToast('Current password is required to verify changes', 'error');
      return;
    }

    if (newPassword && newPassword !== confirmPassword) {
      addToast('New password and confirmation do not match', 'error');
      return;
    }

    if (newPassword && newPassword.length < 6) {
      addToast('New password must be at least 6 characters long', 'error');
      return;
    }

    isSavingProfile = true;
    try {
      const res = await fetch('/api/auth/profile', {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({
          current_password: currentPassword,
          new_username: newUsername.trim() || null,
          new_password: newPassword || null
        })
      });

      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || 'Failed to update credentials');
      }

      const data = await res.json();
      onProfileUpdate(data.token, data.user.username, data.user.role, data.user.listenbrainz_token || '');
      addToast('Admin credentials updated successfully', 'success');
      currentPassword = '';
      newUsername = '';
      newPassword = '';
      confirmPassword = '';
    } catch (err: any) {
      addToast(err.message || 'Update failed', 'error');
    } finally {
      isSavingProfile = false;
    }
  }

  async function handleStartScan() {
    try {
      const res = await fetch('/api/library/scan', {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        addToast('Background folder scan started', 'success');
      } else if (res.status === 409) {
        addToast('A folder scan is already in progress', 'info');
      } else {
        throw new Error();
      }
    } catch (e) {
      addToast('Failed to trigger scan', 'error');
    }
  }

  async function handleStartFetcher() {
    try {
      const res = await fetch('/api/library/fetch', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({ provider: fetcherProvider })
      });
      if (res.ok) {
        addToast('Background metadata worker started', 'success');
      } else if (res.status === 409) {
        addToast('Metadata fetcher is already active', 'info');
      } else {
        throw new Error();
      }
    } catch (e) {
      addToast('Failed to trigger metadata fetcher', 'error');
    }
  }

  async function handleCleanLibrary() {
    if (!confirm('This will search the database and remove track entries whose files have been deleted on disk. Continue?')) {
      return;
    }

    isCleaning = true;
    try {
      const res = await fetch('/api/library/clean', {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        const data = await res.json();
        addToast(`Cleanup complete! Pruned ${data.pruned_count} orphaned tracks.`, 'success');
      } else {
        throw new Error();
      }
    } catch (e) {
      addToast('Failed to clean library', 'error');
    } finally {
      isCleaning = false;
    }
  }

  async function handleResetLibrary() {
    isResetting = true;
    try {
      const res = await fetch('/api/library/reset', {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        addToast('Library has been completely reset.', 'success');
        resetCheck = false;
        resetText = '';
      } else {
        throw new Error();
      }
    } catch (e) {
      addToast('Failed to reset library database', 'error');
    } finally {
      isResetting = false;
    }
  }

  // User Management States
  let isLoadingUsers = $state(false);
  let isCreatingUser = $state(false);
  
  // Create Form
  let createUsername = $state('');
  let createPassword = $state('');
  let createRole = $state('User');
  let isCreating = $state(false);

  // Edit Form
  let editingUserId = $state<string | null>(null);
  let editUsername = $state('');
  let editPassword = $state('');
  let editRole = $state('User');
  let editIsEnabled = $state(1);
  let editLbToken = $state('');
  let isUpdating = $state(false);

  // Storage stats fetched from /api/admin/stats
  let adminUserStats = $state<any[]>([]);
  let isLoadingAdminUserStats = $state(false);

  // Quota edit modal
  let quotaModalUserId = $state<string | null>(null);
  let quotaModalUsername = $state('');
  let quotaModalQuotaGB = $state('10');   // numeric GB value
  let quotaModalUnlimited = $state(false); // checkbox: no limit
  let quotaModalCanUpload = $state(true);  // toggle switch
  let isSavingQuota = $state(false);

  async function fetchUsers() {
    isLoadingUsers = true;
    try {
      const res = await fetch('/api/admin/users', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        usersList = await res.json();
      } else {
        throw new Error();
      }
    } catch (e) {
      addToast('Failed to fetch user list', 'error');
    } finally {
      isLoadingUsers = false;
    }
  }

  // ListenBrainz token updater for current user
  let myLbToken = $state(listenbrainzToken);
  let isSavingLbToken = $state(false);

  async function handleUpdateMyLbToken(e: SubmitEvent) {
    e.preventDefault();
    isSavingLbToken = true;
    try {
      // Find my own user ID
      const meRes = await fetch('/api/auth/me', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (!meRes.ok) throw new Error('Failed to resolve current profile');
      const meData = await meRes.json();
      
      const res = await fetch(`/api/admin/users/${meData.id}`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({
          listenbrainz_token: myLbToken.trim() || null
        })
      });

      if (!res.ok) throw new Error('Failed to save ListenBrainz token');
      
      onProfileUpdate(token, username, role, myLbToken.trim());
      addToast('ListenBrainz token updated successfully', 'success');
    } catch (err: any) {
      addToast(err.message || 'Failed to update ListenBrainz token', 'error');
    } finally {
      isSavingLbToken = false;
    }
  }

  async function handleCreateUser(e: SubmitEvent) {
    e.preventDefault();
    if (!createUsername.trim() || !createPassword.trim()) {
      addToast('Username and password are required', 'error');
      return;
    }
    isCreating = true;
    try {
      const res = await fetch('/api/admin/users', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({
          username: createUsername.trim(),
          password: createPassword.trim(),
          role: createRole
        })
      });

      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || 'Failed to create user');
      }

      addToast('User created successfully', 'success');
      createUsername = '';
      createPassword = '';
      createRole = 'User';
      isCreatingUser = false;
      fetchUsers();
    } catch (err: any) {
      addToast(err.message || 'Failed to create user', 'error');
    } finally {
      isCreating = false;
    }
  }

  async function handleUpdateUser(userId: string) {
    isUpdating = true;
    try {
      const payload: any = {
        username: editUsername.trim() || undefined,
        role: editRole || undefined,
        is_enabled: editIsEnabled,
        listenbrainz_token: editLbToken.trim() || null
      };
      if (editPassword.trim()) {
        payload.password = editPassword.trim();
      }

      const res = await fetch(`/api/admin/users/${userId}`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify(payload)
      });

      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || 'Failed to update user');
      }

      addToast('User updated successfully', 'success');
      editingUserId = null;
      editPassword = '';
      fetchUsers();
    } catch (err: any) {
      addToast(err.message || 'Failed to update user', 'error');
    } finally {
      isUpdating = false;
    }
  }

  async function handleDeleteUser(userId: string, targetUsername: string) {
    if (targetUsername === username) {
      addToast('You cannot delete your own account', 'error');
      return;
    }
    if (!confirm(`Are you sure you want to delete user ${targetUsername}?`)) {
      return;
    }
    try {
      const res = await fetch(`/api/admin/users/${userId}`, {
        method: 'DELETE',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        addToast('User deleted successfully', 'success');
        fetchUsers();
      } else {
        const text = await res.text();
        throw new Error(text || 'Failed to delete user');
      }
    } catch (err: any) {
      addToast(err.message || 'Failed to delete user', 'error');
    }
  }

  function startEditing(user: any) {
    editingUserId = user.id;
    editUsername = user.username;
    editPassword = '';
    editRole = user.role;
    editIsEnabled = user.is_enabled;
    editLbToken = user.listenbrainz_token || '';
  }

  async function fetchAdminUserStats() {
    if (role !== 'Admin') return;
    isLoadingAdminUserStats = true;
    try {
      const res = await fetch('/api/admin/stats', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        adminUserStats = await res.json();
      }
    } catch (e) {
      console.error('Failed to fetch admin user stats', e);
    } finally {
      isLoadingAdminUserStats = false;
    }
  }

  function getStatForUser(userId: string): any | null {
    return adminUserStats.find(s => s.user_id === userId) || null;
  }

  function getQuotaBarColor(pct: number): string {
    if (pct >= 95) return 'var(--danger)';
    if (pct >= 80) return 'var(--warning)';
    return 'var(--success)';
  }

  function openQuotaModal(user: any) {
    quotaModalUserId = user.id;
    quotaModalUsername = user.username;
    quotaModalCanUpload = user.can_upload !== 0;
    
    if (user.storage_quota_bytes === null || user.storage_quota_bytes === undefined || user.storage_quota_bytes < 0) {
      quotaModalUnlimited = true;
      quotaModalQuotaGB = '10'; // default input placeholder when toggled back
    } else {
      quotaModalUnlimited = false;
      quotaModalQuotaGB = (user.storage_quota_bytes / 1073741824).toFixed(2);
      // Clean up .00 from decimal format if it's an integer
      if (quotaModalQuotaGB.endsWith('.00')) {
        quotaModalQuotaGB = quotaModalQuotaGB.slice(0, -3);
      }
    }
  }

  async function handleSaveQuota() {
    if (!quotaModalUserId) return;
    isSavingQuota = true;
    try {
      let quotaBytes: number | null = null;
      if (!quotaModalUnlimited) {
        const parsed = parseFloat(quotaModalQuotaGB);
        if (isNaN(parsed) || parsed < 0) {
          addToast('Please enter a valid storage quota', 'error');
          isSavingQuota = false;
          return;
        }
        quotaBytes = Math.round(parsed * 1073741824);
      }

      const res = await fetch(`/api/admin/users/${quotaModalUserId}/quota`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({
          storage_quota_bytes: quotaBytes,
          can_upload: quotaModalCanUpload ? 1 : 0
        })
      });

      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || 'Failed to update quota');
      }

      addToast('Storage quota and upload status updated successfully', 'success');
      quotaModalUserId = null; // Close modal
      // Refresh user lists and stats
      await Promise.all([fetchUsers(), fetchAdminUserStats()]);
    } catch (err: any) {
      addToast(err.message || 'Failed to update storage quota', 'error');
    } finally {
      isSavingQuota = false;
    }
  }

  $effect(() => {
    if (activeTab === 'users' && role === 'Admin') {
      fetchUsers();
      fetchAdminUserStats();
    }
  });

  $effect(() => {
    if (activeTab === 'system' && role === 'Admin') {
      fetchAdminStats();
    }
  });

  // Public Access Tunnel States & Handlers
  let tunnelConfig = $state<{ provider: 'localhost.run' | 'ngrok' | 'cloudflare'; token: string; custom_domain: string; enabled: boolean }>({
    provider: 'localhost.run',
    token: '',
    custom_domain: '',
    enabled: false
  });

  let tunnelStatus = $state<{ active: boolean; provider: 'localhost.run' | 'ngrok' | 'cloudflare' | null; url: string | null; is_connecting: boolean; error: string | null }>({
    active: false,
    provider: null,
    url: null,
    is_connecting: false,
    error: null
  });

  let isSavingTunnel = $state(false);
  let isTogglingTunnel = $state(false);
  let copied = $state(false);
  let pollInterval: any = null;

  async function fetchTunnelInfo() {
    try {
      const res = await fetch('/api/admin/tunnel', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        const data = await res.json();
        tunnelConfig.provider = data.config.provider;
        tunnelConfig.token = data.config.token || '';
        tunnelConfig.custom_domain = data.config.custom_domain || '';
        tunnelConfig.enabled = data.config.enabled;
        
        tunnelStatus.active = data.status.active;
        tunnelStatus.provider = data.status.provider;
        tunnelStatus.url = data.status.url;
        tunnelStatus.is_connecting = data.status.is_connecting;
        tunnelStatus.error = data.status.error;
      }
    } catch (e) {
      console.error('Failed to fetch tunnel info', e);
    }
  }

  async function handleToggleTunnel() {
    isTogglingTunnel = true;
    try {
      const res = await fetch('/api/admin/tunnel/toggle', {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        const data = await res.json();
        tunnelStatus.active = data.status.active;
        tunnelStatus.provider = data.status.provider;
        tunnelStatus.url = data.status.url;
        tunnelStatus.is_connecting = data.status.is_connecting;
        tunnelStatus.error = data.status.error;
        addToast(
          tunnelStatus.active || tunnelStatus.is_connecting ? 'Public access tunnel started!' : 'Public access tunnel stopped.',
          'success'
        );
      } else {
        const text = await res.text();
        throw new Error(text || 'Failed to toggle tunnel');
      }
    } catch (err: any) {
      addToast(err.message || 'Failed to toggle tunnel', 'error');
    } finally {
      isTogglingTunnel = false;
    }
  }

  async function handleSaveTunnelConfig(e: SubmitEvent) {
    e.preventDefault();
    isSavingTunnel = true;
    try {
      const res = await fetch('/api/admin/tunnel', {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({
          provider: tunnelConfig.provider,
          token: tunnelConfig.token.trim() || null,
          custom_domain: tunnelConfig.custom_domain.trim() || null,
          enabled: tunnelConfig.enabled
        })
      });
      if (res.ok) {
        const data = await res.json();
        tunnelConfig.provider = data.config.provider;
        tunnelConfig.token = data.config.token || '';
        tunnelConfig.custom_domain = data.config.custom_domain || '';
        tunnelConfig.enabled = data.config.enabled;
        
        tunnelStatus.active = data.status.active;
        tunnelStatus.provider = data.status.provider;
        tunnelStatus.url = data.status.url;
        tunnelStatus.is_connecting = data.status.is_connecting;
        tunnelStatus.error = data.status.error;
        addToast('Tunnel configuration updated successfully', 'success');
      } else {
        const text = await res.text();
        throw new Error(text || 'Failed to update tunnel config');
      }
    } catch (err: any) {
      addToast(err.message || 'Failed to update config', 'error');
    } finally {
      isSavingTunnel = false;
    }
  }

  function handleCopy(text: string) {
    navigator.clipboard.writeText(text);
    copied = true;
    addToast('URL copied to clipboard!', 'success');
    setTimeout(() => copied = false, 2000);
  }

  $effect(() => {
    if (activeTab === 'tunnel' && role === 'Admin') {
      fetchTunnelInfo();
    }
  });

  $effect(() => {
    if (tunnelStatus.is_connecting && activeTab === 'tunnel') {
      pollInterval = setInterval(fetchTunnelInfo, 1500);
    } else {
      if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
      }
    }
    return () => {
      if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
      }
    };
  });

  // Storage Settings State & Handlers
  let storageConfig = $state({
    storage_type: 'local',
    s3_endpoint: '',
    s3_bucket: '',
    s3_access_key: '',
    s3_secret_key: '',
    s3_region: '',
    s3_force_path_style: false
  });
  let isTestingStorage = $state(false);
  let isSavingStorage = $state(false);

  async function fetchStorageConfig() {
    try {
      const res = await fetch('/api/admin/storage', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        storageConfig = await res.json();
      }
    } catch (e) {
      console.error('Failed to fetch storage configuration', e);
    }
  }

  async function handleTestStorageConnection() {
    isTestingStorage = true;
    try {
      const res = await fetch('/api/admin/storage?test_only=true', {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify(storageConfig)
      });
      if (res.ok) {
        addToast('S3 storage connection test succeeded!', 'success');
      } else {
        const text = await res.text();
        throw new Error(text || 'Connection test failed');
      }
    } catch (err: any) {
      addToast(err.message || 'Connection test failed', 'error');
    } finally {
      isTestingStorage = false;
    }
  }

  async function handleSaveStorageConfig(e: SubmitEvent) {
    e.preventDefault();
    isSavingStorage = true;
    try {
      const res = await fetch('/api/admin/storage', {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify(storageConfig)
      });
      if (res.ok) {
        addToast('Storage settings updated successfully!', 'success');
        fetchStorageConfig();
      } else {
        const text = await res.text();
        throw new Error(text || 'Failed to update storage settings');
      }
    } catch (err: any) {
      addToast(err.message || 'Failed to update storage settings', 'error');
    } finally {
      isSavingStorage = false;
    }
  }

  $effect(() => {
    if (activeTab === 'storage' && role === 'Admin') {
      fetchStorageConfig();
    }
  });
</script>

<div class="page-header">
  <h1 class="page-title">Settings</h1>
  <p class="page-subtitle">Server environment configuration and administration.</p>
</div>

<div class="settings-container">
  <!-- Tab navigation -->
  <div class="settings-tabs">
    <button 
      onclick={() => activeTab = 'profile'} 
      class="tab-btn {activeTab === 'profile' ? 'active' : ''}"
    >
      <User size={16} /> Profile Settings
    </button>
    {#if role === 'Admin'}
      <button 
        onclick={() => activeTab = 'users'} 
        class="tab-btn {activeTab === 'users' ? 'active' : ''}"
      >
        <Users size={16} /> User Management
      </button>
      <button 
        onclick={() => activeTab = 'storage'} 
        class="tab-btn {activeTab === 'storage' ? 'active' : ''}"
      >
        <Database size={16} /> Storage Settings
      </button>
    {/if}
    {#if role !== 'StreamOnly'}
    <button 
      onclick={() => activeTab = 'library'} 
      class="tab-btn {activeTab === 'library' ? 'active' : ''}"
    >
      <FolderSync size={16} /> Library Control
    </button>
    {/if}
    {#if role === 'Admin'}
      <button 
        onclick={() => activeTab = 'tunnel'} 
        class="tab-btn {activeTab === 'tunnel' ? 'active' : ''}"
      >
        <Globe size={16} /> Public Access
      </button>
    {/if}
    <button 
      onclick={() => activeTab = 'system'} 
      class="tab-btn {activeTab === 'system' ? 'active' : ''}"
    >
      <Cpu size={16} /> System & Info
    </button>
  </div>

  <!-- Profile Settings Tab -->
  {#if activeTab === 'profile'}
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      <div class="glass-card" style="display: flex; justify-content: space-between; align-items: center; padding: 1.5rem;">
        <div style="display: flex; align-items: center; gap: 1rem;">
          <div style="background: rgba(168,85,247,0.1); padding: 0.75rem; border-radius: 10px; color: var(--accent);">
            <Shield size={24} />
          </div>
          <div>
            <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600; margin-bottom: 0.25rem;">Active Account</h3>
            <p style="font-size: 0.85rem; color: var(--text-secondary);">Logged in as <strong style="color: var(--text-primary);">{username}</strong> ({role})</p>
          </div>
        </div>
        <button onclick={onLogout} class="btn btn-danger" style="display: flex; gap: 0.5rem; align-items: center;">
          <LogOut size={16} /> Log Out
        </button>
      </div>

      {#if role !== 'StreamOnly'}
      <div class="glass-card" style="padding: 1.5rem;">
        <div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 1.5rem; border-bottom: 1px solid var(--border-color); padding-bottom: 0.5rem;">
          <Key size={18} style="color: var(--accent);" />
          <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600;">Update Profile Credentials</h3>
        </div>

        <form onsubmit={handleUpdateProfile}>
          <div style="display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1rem; margin-bottom: 1.5rem;">
            <div class="form-group" style="grid-column: span 2;">
              <label class="form-label" for="currentPassword">Current Password <span style="color: var(--danger);">*</span></label>
              <input 
                type="password" 
                id="currentPassword" 
                class="form-input" 
                style="width: 100%;" 
                placeholder="Enter current password to authorize changes"
                bind:value={currentPassword}
                required
              />
            </div>

            <div class="form-group" style="grid-column: span 2; height: 1px; background: var(--border-color); margin: 0.5rem 0;"></div>

            <div class="form-group">
              <label class="form-label" for="newUsername">New Username</label>
              <input 
                type="text" 
                id="newUsername" 
                class="form-input" 
                style="width: 100%;" 
                placeholder="Leave blank to keep '{username}'"
                bind:value={newUsername}
              />
            </div>

            <div class="form-group" style="grid-column: span 1;"></div >

            <div class="form-group">
              <label class="form-label" for="newPassword">New Password</label>
              <input 
                type="password" 
                id="newPassword" 
                class="form-input" 
                style="width: 100%;" 
                placeholder="Minimum 6 characters"
                bind:value={newPassword}
              />
            </div>

            <div class="form-group">
              <label class="form-label" for="confirmPassword">Confirm New Password</label>
              <input 
                type="password" 
                id="confirmPassword" 
                class="form-input" 
                style="width: 100%;" 
                placeholder="Repeat new password"
                bind:value={confirmPassword}
              />
            </div>
          </div>

          <div style="display: flex; justify-content: flex-end;">
            <button type="submit" class="btn btn-primary" style="display: flex; gap: 0.5rem; align-items: center;" disabled={isSavingProfile}>
              {#if isSavingProfile}
                <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Saving...
              {:else}
                Save Credentials
              {/if}
            </button>
          </div>
        </form>
      </div>
      {/if}

      <div class="glass-card" style="padding: 1.5rem;">
        <div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 1.5rem; border-bottom: 1px solid var(--border-color); padding-bottom: 0.5rem;">
          <Database size={18} style="color: var(--accent);" />
          <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600;">ListenBrainz Scrobbling</h3>
        </div>

        <form onsubmit={handleUpdateMyLbToken}>
          <div class="form-group" style="margin-bottom: 1.5rem;">
            <label class="form-label" for="myLbToken">User Token</label>
            <input 
              type="password" 
              id="myLbToken" 
              class="form-input" 
              style="width: 100%;" 
              placeholder="Enter your ListenBrainz user token for automatic server scrobbling"
              bind:value={myLbToken}
            />
            <p style="font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.5rem;">
              You can get your user token from your ListenBrainz account profile settings page.
            </p>
          </div>

          <div style="display: flex; justify-content: flex-end;">
            <button type="submit" class="btn btn-primary" style="display: flex; gap: 0.5rem; align-items: center;" disabled={isSavingLbToken}>
              {#if isSavingLbToken}
                <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Saving...
              {:else}
                Save Token
              {/if}
            </button>
          </div>
        </form>
      </div>
    </div>

  <!-- User Management Tab -->
  {:else if activeTab === 'users' && role === 'Admin'}
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      <div class="glass-card" style="padding: 1.5rem; display: flex; justify-content: space-between; align-items: center;">
        <div>
          <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600; margin-bottom: 0.25rem;">User Management</h3>
          <p style="font-size: 0.85rem; color: var(--text-secondary); margin: 0;">Create and manage user accounts, roles, and status.</p>
        </div>
        <button 
          onclick={() => {
            isCreatingUser = !isCreatingUser;
            editingUserId = null;
          }} 
          class="btn btn-primary" 
          style="display: flex; gap: 0.5rem; align-items: center;"
        >
          {#if isCreatingUser}
            Cancel
          {:else}
            <UserPlus size={16} /> Add User
          {/if}
        </button>
      </div>

      {#if isCreatingUser}
        <div class="glass-card" style="padding: 1.5rem;">
          <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600; margin-bottom: 1.25rem;">Create New User Account</h3>
          <form onsubmit={handleCreateUser}>
            <div style="display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1rem; margin-bottom: 1.5rem;">
              <div class="form-group">
                <label class="form-label" for="createUsername">Username</label>
                <input 
                  type="text" 
                  id="createUsername" 
                  class="form-input" 
                  style="width: 100%;" 
                  placeholder="Enter username"
                  bind:value={createUsername}
                  required
                />
              </div>

              <div class="form-group">
                <label class="form-label" for="createPassword">Password</label>
                <input 
                  type="password" 
                  id="createPassword" 
                  class="form-input" 
                  style="width: 100%;" 
                  placeholder="Minimum 6 characters"
                  bind:value={createPassword}
                  required
                />
              </div>

              <div class="form-group" style="grid-column: span 2;">
                <label class="form-label" for="createRole">User Role</label>
                <select 
                  id="createRole"
                  class="form-input" 
                  style="width: 100%; height: auto; padding: 0.5rem 0.75rem;" 
                  bind:value={createRole}
                >
                  <option value="Admin">Admin (Full Control)</option>
                  <option value="User">User (Standard Access)</option>
                  <option value="StreamOnly">StreamOnly (Playback Only)</option>
                </select>
              </div>
            </div>

            <div style="display: flex; justify-content: flex-end; gap: 0.75rem;">
              <button type="button" onclick={() => isCreatingUser = false} class="btn btn-secondary">Cancel</button>
              <button type="submit" class="btn btn-primary" disabled={isCreating}>
                {#if isCreating}
                  Creating...
                {:else}
                  Create User
                {/if}
              </button>
            </div>
          </form>
        </div>
      {/if}

      <!-- User Search filter bar -->
      <div class="glass-card" style="padding: 1rem; display: flex; align-items: center; gap: 0.5rem; position: relative;">
        <Search size={18} style="position: absolute; left: 1.75rem; color: var(--text-secondary);" />
        <input 
          type="text" 
          class="form-input" 
          placeholder="Search users by username or role..." 
          style="width: 100%; padding-left: 2.75rem;" 
          bind:value={userSearchQuery}
        />
      </div>

      <div class="glass-card" style="padding: 1.5rem;">
        {#if isLoadingUsers}
          <div style="text-align: center; padding: 2rem; color: var(--text-secondary);">
            <RefreshCw size={24} class="animate-spin" style="animation: spin 1s linear infinite; margin: 0 auto 1rem;" />
            <span>Loading user list...</span>
          </div>
        {:else if filteredUsers.length === 0}
          <div style="text-align: center; padding: 2rem; color: var(--text-secondary);">
            No matching users found.
          </div>
        {:else}
          <div style="overflow-x: auto;">
            <table class="library-table" style="width: 100%; font-size: 0.9rem; border-top: 1px solid var(--border-color);">
              <thead>
                <tr>
                  <th style="text-align: left; padding: 0.75rem;">Username</th>
                  <th style="text-align: left; padding: 0.75rem;">Role</th>
                  <th style="text-align: left; padding: 0.75rem;">ListenBrainz Token</th>
                  <th style="text-align: left; padding: 0.75rem;">Status</th>
                  <th style="text-align: left; padding: 0.75rem;">Storage</th>
                  <th style="text-align: left; padding: 0.75rem;">Uploads</th>
                  <th style="text-align: left; padding: 0.75rem;">Created At</th>
                  <th style="text-align: right; padding: 0.75rem;">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each filteredUsers as u}
                  {@const stat = getStatForUser(u.id)}
                  {@const used = stat ? stat.total_size_bytes : 0}
                  {@const quota = u.storage_quota_bytes}
                  <tr>
                    <td style="padding: 0.75rem; vertical-align: middle;">
                      {#if editingUserId === u.id}
                        <input type="text" class="form-input" style="padding: 0.35rem 0.5rem; font-size: 0.85rem;" bind:value={editUsername} />
                      {:else}
                        <span style="font-weight: 500; color: var(--text-primary);">{u.username}</span>
                      {/if}
                    </td>
                    <td style="padding: 0.75rem; vertical-align: middle;">
                      {#if editingUserId === u.id}
                        <select class="form-input" style="padding: 0.35rem; font-size: 0.85rem; height: auto;" bind:value={editRole}>
                          <option value="Admin">Admin</option>
                          <option value="User">User</option>
                          <option value="StreamOnly">StreamOnly</option>
                        </select>
                      {:else if u.role === 'Admin'}
                        <span style="font-size: 0.75rem; text-transform: uppercase; background: rgba(168, 85, 247, 0.15); color: #c084fc; border: 1px solid rgba(168, 85, 247, 0.3); padding: 0.15rem 0.5rem; border-radius: 4px; font-weight: 600; letter-spacing: 0.05em;">Admin</span>
                      {:else if u.role === 'User'}
                        <span style="font-size: 0.75rem; text-transform: uppercase; background: rgba(59, 130, 246, 0.15); color: #60a5fa; border: 1px solid rgba(59, 130, 246, 0.3); padding: 0.15rem 0.5rem; border-radius: 4px; font-weight: 600; letter-spacing: 0.05em;">User</span>
                      {:else}
                        <span style="font-size: 0.75rem; text-transform: uppercase; background: rgba(245, 158, 11, 0.15); color: #fbbf24; border: 1px solid rgba(245, 158, 11, 0.3); padding: 0.15rem 0.5rem; border-radius: 4px; font-weight: 600; letter-spacing: 0.05em;">StreamOnly</span>
                      {/if}
                    </td>
                    <td style="padding: 0.75rem; vertical-align: middle;">
                      {#if editingUserId === u.id}
                        <input type="password" class="form-input" style="padding: 0.35rem 0.5rem; font-size: 0.85rem;" placeholder="Change token" bind:value={editLbToken} />
                      {:else if u.listenbrainz_token}
                        <span style="color: var(--success); display: flex; align-items: center; gap: 0.25rem;"><Check size={14} /> Configured</span>
                      {:else}
                        <span style="color: var(--text-muted); display: flex; align-items: center; gap: 0.25rem;"><X size={14} /> None</span>
                      {/if}
                    </td>
                    <td style="padding: 0.75rem; vertical-align: middle;">
                      {#if editingUserId === u.id}
                        <select class="form-input" style="padding: 0.35rem; font-size: 0.85rem; height: auto;" bind:value={editIsEnabled}>
                          <option value={1}>Enabled</option>
                          <option value={0}>Disabled</option>
                        </select>
                      {:else if u.is_enabled === 1}
                        <span style="color: var(--success); font-weight: 500;">Enabled</span>
                      {:else}
                        <span style="color: var(--danger); font-weight: 500;">Disabled</span>
                      {/if}
                    </td>
                    <td style="padding: 0.75rem; vertical-align: middle;">
                      <div style="display: flex; flex-direction: column; gap: 0.25rem; min-width: 140px;">
                        <div style="display: flex; justify-content: space-between; font-size: 0.75rem; color: var(--text-secondary);">
                          <span>{formatBytes(used)}</span>
                          {#if quota === null || quota === undefined || quota < 0}
                            <span style="display: flex; align-items: center; gap: 0.15rem; color: var(--text-muted);">
                              <Infinity size={10} /> Unlimited
                            </span>
                          {:else}
                            <span>/ {formatBytes(quota)}</span>
                          {/if}
                        </div>
                        {#if quota !== null && quota !== undefined && quota > 0}
                          {@const pct = Math.min((used / quota) * 100, 100)}
                          <div class="storage-bar-container">
                            <div class="storage-bar-fill" style="width: {pct}%; background-color: {getQuotaBarColor(pct)};"></div>
                          </div>
                        {/if}
                      </div>
                    </td>
                    <td style="padding: 0.75rem; vertical-align: middle;">
                      {#if u.can_upload === 0}
                        <span class="quota-badge frozen">
                          <Ban size={10} /> Frozen
                        </span>
                      {:else}
                        <span class="quota-badge enabled">
                          <UploadCloud size={10} /> Enabled
                        </span>
                      {/if}
                    </td>
                    <td style="padding: 0.75rem; vertical-align: middle; color: var(--text-secondary); font-family: monospace; font-size: 0.85rem;">
                      {formatDateTime(u.created_at)}
                    </td>
                    <td style="padding: 0.75rem; text-align: right; vertical-align: middle;">
                      {#if editingUserId === u.id}
                        <div style="display: flex; gap: 0.5rem; justify-content: flex-end;">
                          <button onclick={() => handleUpdateUser(u.id)} class="btn btn-primary" style="padding: 0.35rem 0.5rem; font-size: 0.85rem;" disabled={isUpdating}>
                            Save
                          </button>
                          <button onclick={() => editingUserId = null} class="btn btn-secondary" style="padding: 0.35rem 0.5rem; font-size: 0.85rem;">
                            Cancel
                          </button>
                        </div>
                      {:else}
                        <div style="display: flex; gap: 0.5rem; justify-content: flex-end;">
                          <button onclick={() => openQuotaModal(u)} class="btn btn-secondary" style="padding: 0.35rem 0.5rem; font-size: 0.85rem; display: flex; align-items: center; gap: 0.25rem;">
                            <HardDrive size={12} /> Quota
                          </button>
                          <button onclick={() => startEditing(u)} class="btn btn-secondary" style="padding: 0.35rem 0.5rem; font-size: 0.85rem; display: flex; align-items: center; gap: 0.25rem;">
                            <Edit2 size={12} /> Edit
                          </button>
                          {#if u.username !== username}
                            <button onclick={() => handleDeleteUser(u.id, u.username)} class="btn btn-danger" style="padding: 0.35rem 0.5rem; font-size: 0.85rem;">
                              Delete
                            </button>
                          {/if}
                        </div>
                      {/if}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>

      <!-- Quota Edit Modal Overlay -->
      {#if quotaModalUserId !== null}
        {@const userStat = getStatForUser(quotaModalUserId)}
        {@const usedBytes = userStat ? userStat.total_size_bytes : 0}
        <div class="quota-modal-backdrop" onclick={() => quotaModalUserId = null}>
          <div class="glass-card quota-modal" onclick={(e) => e.stopPropagation()}>
            <div style="display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--border-color); padding-bottom: 1rem; margin-bottom: 1.5rem;">
              <div style="display: flex; align-items: center; gap: 0.5rem;">
                <HardDrive size={20} style="color: var(--accent);" />
                <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600; margin: 0;">Storage & Uploads — {quotaModalUsername}</h3>
              </div>
              <button class="btn btn-secondary" style="padding: 0.25rem; border-radius: 50%; width: 28px; height: 28px; display: flex; align-items: center; justify-content: center;" onclick={() => quotaModalUserId = null}>
                <X size={16} />
              </button>
            </div>

            <!-- Current Storage Usage Display -->
            <div style="background: rgba(255, 255, 255, 0.02); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; margin-bottom: 1.5rem;">
              <div style="display: flex; justify-content: space-between; font-size: 0.85rem; margin-bottom: 0.5rem;">
                <span style="color: var(--text-secondary);">Current Usage</span>
                <span style="font-weight: 600; color: var(--text-primary);">
                  {formatBytes(usedBytes)} / {quotaModalUnlimited ? 'Unlimited' : formatBytes(parseFloat(quotaModalQuotaGB || '0') * 1073741824)}
                </span>
              </div>
              
              {#if !quotaModalUnlimited}
                {@const targetQuotaBytes = parseFloat(quotaModalQuotaGB || '0') * 1073741824}
                {@const usagePct = targetQuotaBytes > 0 ? Math.min((usedBytes / targetQuotaBytes) * 100, 100) : 0}
                <div class="storage-bar-container" style="height: 8px; margin-bottom: 0.25rem;">
                  <div class="storage-bar-fill" style="width: {usagePct}%; background-color: {getQuotaBarColor(usagePct)};"></div>
                </div>
                <div style="display: flex; justify-content: flex-end; font-size: 0.75rem; color: {getQuotaBarColor(usagePct)}; font-weight: 500;">
                  {usagePct.toFixed(1)}% Used
                </div>
              {:else}
                <div class="storage-bar-container" style="height: 8px; margin-bottom: 0.25rem;">
                  <div class="storage-bar-fill" style="width: 0%; background-color: var(--success);"></div>
                </div>
                <div style="display: flex; justify-content: flex-end; font-size: 0.75rem; color: var(--text-muted);">
                  No limit enforced
                </div>
              {/if}
            </div>

            <!-- Quota Configuration Form -->
            <div style="display: flex; flex-direction: column; gap: 1.25rem;">
              <div class="form-group">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem;">
                  <label class="form-label" for="quotaLimitInput" style="margin: 0;">Storage Limit</label>
                  <label style="display: flex; align-items: center; gap: 0.35rem; font-size: 0.85rem; color: var(--text-secondary); cursor: pointer;">
                    <input type="checkbox" bind:checked={quotaModalUnlimited} style="accent-color: var(--accent);" />
                    <span>No Limit</span>
                  </label>
                </div>
                
                <div style="position: relative; display: flex; align-items: center;">
                  <input 
                    type="number" 
                    id="quotaLimitInput" 
                    class="form-input" 
                    style="width: 100%; padding-right: 3rem;" 
                    placeholder="Enter quota"
                    min="0.1" 
                    step="any"
                    disabled={quotaModalUnlimited}
                    bind:value={quotaModalQuotaGB}
                  />
                  <span style="position: absolute; right: 1rem; color: var(--text-muted); font-size: 0.85rem; font-weight: 600; pointer-events: none;">GB</span>
                </div>
              </div>

              <!-- Upload restriction toggle switch -->
              <div style="display: flex; align-items: center; justify-content: space-between; background: rgba(255,255,255,0.02); border: 1px solid var(--border-color); border-radius: 8px; padding: 0.85rem 1rem;">
                <div style="display: flex; flex-direction: column; gap: 0.15rem;">
                  <span style="font-weight: 500; font-size: 0.9rem; color: var(--text-primary);">Allow Direct Uploads</span>
                  <span style="font-size: 0.75rem; color: var(--text-secondary);">Permit user to upload tracks directly</span>
                </div>
                <label class="switch-container">
                  <input type="checkbox" bind:checked={quotaModalCanUpload} />
                  <span class="switch-slider"></span>
                </label>
              </div>
            </div>

            <!-- Form Actions -->
            <div style="display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 2rem; border-top: 1px solid var(--border-color); padding-top: 1.25rem;">
              <button class="btn btn-secondary" onclick={() => quotaModalUserId = null}>Cancel</button>
              <button class="btn btn-primary" onclick={handleSaveQuota} disabled={isSavingQuota}>
                {#if isSavingQuota}
                  <RefreshCw size={14} class="animate-spin" style="animation: spin 1s linear infinite; margin-right: 0.25rem;" /> Saving...
                {:else}
                  Save Changes
                {/if}
              </button>
            </div>
          </div>
        </div>
      {/if}
    </div>

  <!-- Library Management Tab -->
  {:else if activeTab === 'library'}
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">

      
      <!-- Scan Music Folder -->
      <div class="glass-card" style="padding: 1.5rem;">
        <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.25rem;">
          <div style="display: flex; align-items: center; gap: 0.5rem;">
            <FolderSync size={20} style="color: var(--accent);" />
            <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600;">Scan Music Directory</h3>
          </div>
          <button 
            onclick={handleStartScan} 
            class="btn btn-primary" 
            style="display: flex; gap: 0.5rem; align-items: center;"
            disabled={scanStatus.isScanning}
          >
            {#if scanStatus.isScanning}
              <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Scanning...
            {:else}
              Scan Music Folder
            {/if}
          </button>
        </div>
        <p style="font-size: 0.9rem; color: var(--text-secondary); line-height: 1.5; margin-bottom: 1rem;">
          Recursively walks the mounted storage folder <code style="background: rgba(0,0,0,0.3); padding: 0.1rem 0.3rem;">data_dir/music</code>, imports new audio tracks, and updates metadata/covers in the database catalog.
        </p>

        {#if scanStatus.isScanning}
          <div class="status-box" style="margin-top: 1rem;">
            <div style="display: flex; justify-content: space-between; font-size: 0.85rem; margin-bottom: 0.5rem; font-weight: 500;">
              <span>Scanning music library files...</span>
              <span style="font-family: monospace;">{scanStatus.filesScanned} / {scanStatus.totalFiles}</span>
            </div>
            
            <div style="height: 6px; background: rgba(255,255,255,0.06); border-radius: 999px; overflow: hidden; margin-bottom: 0.5rem;">
              <div 
                style="height: 100%; width: {scanStatus.totalFiles > 0 ? (scanStatus.filesScanned / scanStatus.totalFiles) * 100 : 0}%; background: var(--accent); transition: width 0.3s ease;"
              ></div>
            </div>
            
            {#if scanStatus.currentFile}
              <div style="font-size: 0.75rem; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                Processing: {scanStatus.currentFile}
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Metadata Auto-Fetcher -->
      <div class="glass-card" style="padding: 1.5rem;">
        <div style="display: flex; align-items: flex-end; justify-content: space-between; margin-bottom: 1.25rem; gap: 1rem; flex-wrap: wrap;">
          <div style="display: flex; align-items: center; gap: 0.5rem;">
            <Database size={20} style="color: var(--accent);" />
            <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600; margin: 0;">Metadata Auto-Fetcher</h3>
          </div>
          <div style="display: flex; align-items: center; gap: 1rem; flex-wrap: wrap;">
            <div style="display: flex; align-items: center; gap: 0.5rem;">
              <label class="form-label" for="provider-select" style="margin: 0; font-size: 0.85rem; color: var(--text-secondary); white-space: nowrap;">Source:</label>
              <select 
                id="provider-select"
                class="form-input" 
                bind:value={fetcherProvider}
                style="padding: 0.35rem 2rem 0.35rem 0.75rem; cursor: pointer; height: auto; font-size: 0.85rem;"
                disabled={fetcherStatus.isRunning}
              >
                <option value="deezer">Deezer (Fast)</option>
                <option value="musicbrainz">MusicBrainz (Detailed)</option>
              </select>
            </div>
            <button 
              onclick={handleStartFetcher} 
              class="btn btn-primary" 
              style="display: flex; gap: 0.5rem; align-items: center;"
              disabled={fetcherStatus.isRunning}
            >
              {#if fetcherStatus.isRunning}
                <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Fetching...
              {:else}
                Start Auto-Fetcher
              {/if}
            </button>
          </div>
        </div>
        <p style="font-size: 0.9rem; color: var(--text-secondary); line-height: 1.5; margin-bottom: 1rem;">
          Identifies tracks missing tags/metadata in the database and queries the selected public API (Deezer or MusicBrainz) to fetch correct titles, artist profiles, album details, genres, track numbers, and high-resolution cover arts.
        </p>

        {#if fetcherStatus.isRunning || fetcherStatus.logs.length > 0}
          <div class="console-box">
            <div style="display: flex; justify-content: space-between; font-size: 0.85rem; font-weight: 600; border-bottom: 1px solid rgba(255,255,255,0.06); padding-bottom: 0.5rem; margin-bottom: 0.75rem;">
              <span style="display: flex; align-items: center; gap: 0.4rem;">
                <FileText size={14} /> Fetcher Output Logs
              </span>
              {#if fetcherStatus.isRunning}
                <span style="font-family: monospace; color: var(--accent);">Processing {fetcherStatus.tracksProcessed} / {fetcherStatus.totalTracks}</span>
              {:else}
                <span style="color: var(--success); display: flex; align-items: center; gap: 0.25rem;"><CheckCircle2 size={14} /> Finished</span>
              {/if}
            </div>

            <div class="logs-container">
              {#each fetcherStatus.logs as log}
                <div class="log-line">{log}</div>
              {/each}
            </div>
          </div>
        {/if}
      </div>

      <!-- Cleanup Library database -->
      <div class="glass-card" style="padding: 1.5rem;">
        <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.5rem;">
          <div style="display: flex; align-items: center; gap: 0.5rem;">
            <Trash2 size={20} style="color: var(--accent);" />
            <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600;">Prune Missing Files</h3>
          </div>
          <button 
            onclick={handleCleanLibrary} 
            class="btn btn-secondary" 
            style="display: flex; gap: 0.5rem; align-items: center;"
            disabled={isCleaning}
          >
            {#if isCleaning}
              <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Pruning...
            {:else}
              Clean Database
            {/if}
          </button>
        </div>
        <p style="font-size: 0.9rem; color: var(--text-secondary); line-height: 1.5;">
          Scans database records and removes entries for music files that have been deleted from your host storage. This will also delete empty album containers and clean up orphaned cover cached files.
        </p>
      </div>

      <!-- Danger Zone - Reset database -->
      <div class="glass-card border-danger" style="padding: 1.5rem; border: 1px solid rgba(239, 68, 68, 0.2); background: rgba(239, 68, 68, 0.02);">
        <div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 1rem;">
          <AlertTriangle size={20} style="color: var(--danger);" />
          <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600; color: var(--danger);">Danger Zone - Reset Library</h3>
        </div>
        
        <p style="font-size: 0.9rem; color: var(--text-secondary); line-height: 1.5; margin-bottom: 1.25rem;">
          Wiping the database will delete all imported tracks, albums, custom playlists, and user likes from the server. **This operation is irreversible.** Your physical audio files in the music folder will remain safe and untouched.
        </p>

        <div style="background: rgba(255,255,255,0.02); border: 1px solid var(--border-color); padding: 1rem; border-radius: 6px; display: flex; flex-direction: column; gap: 1rem; margin-bottom: 1rem;">
          <label style="display: flex; align-items: flex-start; gap: 0.5rem; cursor: pointer; font-size: 0.85rem; color: var(--text-secondary);">
            <input type="checkbox" bind:checked={resetCheck} style="margin-top: 0.2rem;" />
            <span>I understand that this will wipe all library data, custom playlists, and likes from the database.</span>
          </label>

          {#if resetCheck}
            <div class="form-group" style="margin: 0;">
              <label class="form-label" for="reset-confirm" style="font-size: 0.8rem; margin-bottom: 0.35rem;">To confirm, type <strong style="color: var(--text-primary);">RESET</strong> below:</label>
              <input 
                type="text" 
                id="reset-confirm"
                class="form-input" 
                style="width: 100%; max-width: 250px; font-weight: 600;" 
                placeholder="RESET"
                bind:value={resetText}
              />
            </div>
          {/if}
        </div>

        <button 
          onclick={handleResetLibrary} 
          class="btn btn-danger" 
          disabled={!resetCheck || resetText !== 'RESET' || isResetting}
          style="display: flex; gap: 0.5rem; align-items: center;"
        >
          {#if isResetting}
            <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Wiping database...
          {:else}
            Wipe Library Database
          {/if}
        </button>
      </div>

    </div>

  {:else if activeTab === 'tunnel' && role === 'Admin'}
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      
      <!-- Tunnel Status & Control Card -->
      <div class="glass-card" style="padding: 1.5rem; display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 1rem;">
        <div>
          <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600; margin-bottom: 0.25rem; display: flex; align-items: center; gap: 0.5rem;">
            <Globe size={20} style="color: var(--accent);" /> Public Access Tunnel
          </h3>
          <p style="font-size: 0.85rem; color: var(--text-secondary); margin: 0;">Expose your local server securely to the public internet.</p>
        </div>
        
        <div style="display: flex; align-items: center; gap: 1rem;">
          {#if tunnelStatus.active}
            <span style="background: rgba(34,197,94,0.1); color: #22c55e; padding: 0.35rem 0.75rem; border-radius: 999px; font-size: 0.8rem; font-weight: 600; display: inline-flex; align-items: center; gap: 0.4rem; border: 1px solid rgba(34,197,94,0.2);">
              <span style="width: 8px; height: 8px; border-radius: 50%; background: #22c55e; display: inline-block;"></span> Active
            </span>
          {:else if tunnelStatus.is_connecting}
            <span style="background: rgba(168,85,247,0.1); color: var(--accent); padding: 0.35rem 0.75rem; border-radius: 999px; font-size: 0.8rem; font-weight: 600; display: inline-flex; align-items: center; gap: 0.4rem; border: 1px solid rgba(168,85,247,0.2);">
              <span style="width: 8px; height: 8px; border-radius: 50%; background: var(--accent); display: inline-block; animation: pulse 1.5s infinite;"></span> Connecting...
            </span>
          {:else}
            <span style="background: rgba(239,68,68,0.1); color: #ef4444; padding: 0.35rem 0.75rem; border-radius: 999px; font-size: 0.8rem; font-weight: 600; display: inline-flex; align-items: center; gap: 0.4rem; border: 1px solid rgba(239,68,68,0.2);">
              <span style="width: 8px; height: 8px; border-radius: 50%; background: #ef4444; display: inline-block;"></span> Inactive
            </span>
          {/if}

          <button 
            onclick={handleToggleTunnel} 
            class="btn {tunnelStatus.active || tunnelStatus.is_connecting ? 'btn-danger' : 'btn-primary'}" 
            style="display: flex; gap: 0.5rem; align-items: center;"
            disabled={isTogglingTunnel}
          >
            {#if isTogglingTunnel}
              <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Loading...
            {:else if tunnelStatus.active || tunnelStatus.is_connecting}
              Stop Tunnel
            {:else}
              Start Tunnel
            {/if}
          </button>
        </div>
      </div>

      <!-- Error Message Card -->
      {#if tunnelStatus.error}
        <div class="glass-card border-danger" style="padding: 1rem 1.5rem; background: rgba(239, 68, 68, 0.04); border: 1px solid rgba(239, 68, 68, 0.2); display: flex; align-items: center; gap: 0.75rem;">
          <AlertTriangle size={20} style="color: #ef4444; flex-shrink: 0;" />
          <div style="font-size: 0.85rem; color: var(--text-primary);">
            <strong style="color: #ef4444;">Tunnel Connection Error:</strong> {tunnelStatus.error}
          </div>
        </div>
      {/if}

      <!-- Tunnel Information Card (When Active) -->
      {#if tunnelStatus.active && tunnelStatus.url}
        <div class="glass-card" style="padding: 1.5rem;">
          <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600; margin-bottom: 1rem;">Connection Information</h3>
          
          <div style="display: flex; flex-direction: column; gap: 1.25rem;">
            <!-- Public URL row -->
            <div style="background: rgba(0,0,0,0.2); border: 1px solid var(--border-color); padding: 1rem; border-radius: 8px; display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 1rem;">
              <div>
                <div style="font-size: 0.75rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 0.25rem;">Public URL</div>
                <a href={tunnelStatus.url} target="_blank" rel="noreferrer" style="font-family: monospace; font-size: 1rem; color: var(--accent); font-weight: 600; text-decoration: none; display: inline-flex; align-items: center; gap: 0.35rem;">
                  {tunnelStatus.url} <ExternalLink size={14} />
                </a>
              </div>
              <button onclick={() => handleCopy(tunnelStatus.url || '')} class="btn btn-secondary" style="display: flex; gap: 0.5rem; align-items: center; padding: 0.5rem 0.75rem; font-size: 0.85rem;">
                {#if copied}
                  <Check size={16} /> Copied!
                {:else}
                  <Copy size={16} /> Copy Link
                {/if}
              </button>
            </div>

            <!-- QR Code and Subsonic setup columns -->
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 1.5rem;">
              <!-- QR Code Card -->
              <div style="background: rgba(255,255,255,0.02); border: 1px solid var(--border-color); padding: 1.25rem; border-radius: 8px; text-align: center; display: flex; flex-direction: column; align-items: center; justify-content: center;">
                <h4 style="font-family: var(--font-heading); font-size: 0.95rem; font-weight: 600; margin-bottom: 0.75rem;">Scan on Mobile</h4>
                <div style="background: white; padding: 8px; border-radius: 8px; display: inline-block;">
                  <img 
                    src="https://api.qrserver.com/v1/create-qr-code/?size=150x150&color=000000&data={encodeURIComponent(tunnelStatus.url)}" 
                    alt="Scan to open on mobile" 
                    style="display: block; width: 140px; height: 140px;" 
                  />
                </div>
                <p style="font-size: 0.75rem; color: var(--text-secondary); margin-top: 0.75rem; max-width: 200px;">Scan this QR code to instantly open the player on your phone.</p>
              </div>

              <!-- Subsonic setup instructions -->
              <div style="background: rgba(255,255,255,0.02); border: 1px solid var(--border-color); padding: 1.25rem; border-radius: 8px; display: flex; flex-direction: column; justify-content: center;">
                <h4 style="font-family: var(--font-heading); font-size: 0.95rem; font-weight: 600; margin-bottom: 0.75rem; display: flex; align-items: center; gap: 0.4rem;">
                  <Lock size={16} style="color: var(--accent);" /> Subsonic Integration
                </h4>
                <p style="font-size: 0.8rem; color: var(--text-secondary); line-height: 1.4; margin-bottom: 0.75rem;">
                  To stream using external apps (like Symfonium or play:Sub) on cellular or remote networks, use these details:
                </p>
                <div style="font-size: 0.8rem; background: rgba(0,0,0,0.15); padding: 0.75rem; border-radius: 6px; border: 1px solid var(--border-color); font-family: monospace; display: flex; flex-direction: column; gap: 0.35rem;">
                  <div>Server URL: <span style="color: var(--accent); word-break: break-all;">{tunnelStatus.url}</span></div>
                  <div>Username: <span style="color: var(--text-primary);">{username}</span></div>
                  <div>Password: <span style="color: var(--text-muted);">[Your Account Password]</span></div>
                </div>
              </div>
            </div>

          </div>
        </div>
      {/if}

      <!-- Configuration Card -->
      <div class="glass-card" style="padding: 1.5rem;">
        <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600; margin-bottom: 1.25rem;">Tunnel Configuration</h3>
        
        <form onsubmit={handleSaveTunnelConfig}>
          <div style="display: flex; flex-direction: column; gap: 1.25rem;">
            
            <div class="form-group">
              <label class="form-label" for="tunnel-provider">Tunnel Provider</label>
              <select 
                id="tunnel-provider" 
                class="form-input" 
                bind:value={tunnelConfig.provider}
                style="width: 100%; height: auto; padding: 0.5rem 0.75rem;"
              >
                <option value="localhost.run">localhost.run (One-Click, Free, Anonymous)</option>
                <option value="ngrok">ngrok (Free/Paid Account, Authtoken Required)</option>
                <option value="cloudflare">Cloudflare Tunnel (Free Account, Token Required)</option>
              </select>
            </div>

            {#if tunnelConfig.provider === 'localhost.run'}
              <div style="background: rgba(255,255,255,0.02); border: 1px solid var(--border-color); padding: 1rem; border-radius: 6px; font-size: 0.85rem; line-height: 1.4; color: var(--text-secondary);">
                <strong>About localhost.run:</strong>
                <ul style="margin: 0.5rem 0 0; padding-left: 1.25rem; display: flex; flex-direction: column; gap: 0.25rem;">
                  <li>Does not require any accounts, installations, or key configurations.</li>
                  <li>Generates a dynamic public HTTPS domain (e.g., <code style="background: rgba(0,0,0,0.25); padding: 0.1rem 0.3rem;">*.lhr.life</code>) every time you start the tunnel.</li>
                </ul>
              </div>
            {/if}

            {#if tunnelConfig.provider === 'ngrok'}
              <div class="form-group">
                <label class="form-label" for="ngrok-token">ngrok Authtoken <span style="color: var(--danger);">*</span></label>
                <input 
                  type="password" 
                  id="ngrok-token" 
                  class="form-input" 
                  style="width: 100%;" 
                  placeholder="Paste your ngrok Authtoken here" 
                  bind:value={tunnelConfig.token}
                  required
                />
                <p style="font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.35rem;">
                  Get your Authtoken by signing up at <a href="https://ngrok.com" target="_blank" rel="noreferrer" style="color: var(--accent);">ngrok.com</a> (Free tier supports custom or dynamic subdomains).
                </p>
              </div>
            {/if}

            {#if tunnelConfig.provider === 'cloudflare'}
              <div style="display: flex; flex-direction: column; gap: 1rem;">
                <div class="form-group">
                  <label class="form-label" for="cf-token">Cloudflare Tunnel Token <span style="color: var(--danger);">*</span></label>
                  <input 
                    type="password" 
                    id="cf-token" 
                    class="form-input" 
                    style="width: 100%;" 
                    placeholder="Paste your Cloudflare Tunnel Token here" 
                    bind:value={tunnelConfig.token}
                    required
                  />
                  <p style="font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.35rem;">
                    Obtained from Cloudflare Zero Trust Dashboard under Access &rarr; Tunnels. Select 'Cloudflared' connector to copy the token.
                  </p>
                </div>

                <div class="form-group">
                  <label class="form-label" for="cf-domain">Custom Domain Name (Optional)</label>
                  <input 
                    type="text" 
                    id="cf-domain" 
                    class="form-input" 
                    style="width: 100%;" 
                    placeholder="music.yourdomain.com" 
                    bind:value={tunnelConfig.custom_domain}
                  />
                  <p style="font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.35rem;">
                    Display name for the connection. Note that you must still map this domain to your tunnel inside the Cloudflare Dashboard.
                  </p>
                </div>
              </div>
            {/if}

            <div style="display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 0.5rem;">
              <button type="submit" class="btn btn-primary" style="display: flex; gap: 0.5rem; align-items: center;" disabled={isSavingTunnel}>
                {#if isSavingTunnel}
                  <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Saving...
                {:else}
                  Save Configuration
                {/if}
              </button>
            </div>

          </div>
        </form>
      </div>

    </div>

  <!-- Storage Settings Tab -->
  {:else if activeTab === 'storage' && role === 'Admin'}
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      <div class="glass-card" style="padding: 1.5rem;">
        <div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 1.25rem; border-bottom: 1px solid var(--border-color); padding-bottom: 0.5rem;">
          <Database size={20} style="color: var(--accent);" />
          <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600;">Media Storage Configuration</h3>
        </div>

        <form onsubmit={handleSaveStorageConfig}>
          <div style="display: flex; flex-direction: column; gap: 1.25rem;">
            
            <div class="form-group">
              <label class="form-label" for="storage-type">Storage Backend</label>
              <select 
                id="storage-type" 
                class="form-input" 
                bind:value={storageConfig.storage_type}
                style="width: 100%; height: auto; padding: 0.5rem 0.75rem;"
              >
                <option value="local">Local Host File System (Default)</option>
                <option value="s3">S3-Compatible Object Storage (AWS, MinIO, Cloudflare R2, etc.)</option>
              </select>
            </div>

            {#if storageConfig.storage_type === 'local'}
              <div style="background: rgba(255,255,255,0.02); border: 1px solid var(--border-color); padding: 1rem; border-radius: 6px; font-size: 0.85rem; line-height: 1.4; color: var(--text-secondary);">
                <strong>Local Storage Active:</strong> All uploaded audio tracks and metadata images will be written to the host container's local directory: <code style="background: rgba(0,0,0,0.25); padding: 0.1rem 0.3rem;">./data/</code>.
              </div>
            {/if}

            {#if storageConfig.storage_type === 's3'}
              <div style="display: flex; flex-direction: column; gap: 1rem; border-top: 1px solid rgba(255,255,255,0.05); padding-top: 1rem;">
                
                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 1rem;">
                  <div class="form-group">
                    <label class="form-label" for="s3-endpoint">Endpoint URL</label>
                    <input 
                      type="text" 
                      id="s3-endpoint" 
                      class="form-input" 
                      style="width: 100%;" 
                      placeholder="e.g. https://<account-id>.r2.cloudflarestorage.com" 
                      bind:value={storageConfig.s3_endpoint}
                    />
                    <p style="font-size: 0.78rem; color: var(--text-secondary); margin-top: 0.3rem;">
                      Do <strong>not</strong> include the bucket name in the URL.
                    </p>
                  </div>

                  <div class="form-group">
                    <label class="form-label" for="s3-bucket">Bucket Name <span style="color: var(--danger);">*</span></label>
                    <input 
                      type="text" 
                      id="s3-bucket" 
                      class="form-input" 
                      style="width: 100%;" 
                      placeholder="my-music-bucket" 
                      bind:value={storageConfig.s3_bucket}
                      required={storageConfig.storage_type === 's3'}
                    />
                  </div>
                </div>

                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 1rem;">
                  <div class="form-group">
                    <label class="form-label" for="s3-access-key">Access Key</label>
                    <input 
                      type="text" 
                      id="s3-access-key" 
                      class="form-input" 
                      style="width: 100%;" 
                      placeholder="S3 access key" 
                      bind:value={storageConfig.s3_access_key}
                    />
                  </div>

                  <div class="form-group">
                    <label class="form-label" for="s3-secret-key">Secret Key</label>
                    <input 
                      type="password" 
                      id="s3-secret-key" 
                      class="form-input" 
                      style="width: 100%;" 
                      placeholder={storageConfig.s3_secret_key ? "********" : "S3 secret key"}
                      bind:value={storageConfig.s3_secret_key}
                    />
                  </div>
                </div>

                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 1rem;">
                  <div class="form-group">
                    <label class="form-label" for="s3-region">Region</label>
                    <input 
                      type="text" 
                      id="s3-region" 
                      class="form-input" 
                      style="width: 100%;" 
                      placeholder="us-east-1 (use 'auto' for Cloudflare R2)" 
                      bind:value={storageConfig.s3_region}
                    />
                  </div>

                  <div class="form-group" style="display: flex; align-items: center; margin-top: 1.75rem;">
                    <label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer; font-size: 0.85rem; color: var(--text-secondary);">
                      <input 
                        type="checkbox" 
                        bind:checked={storageConfig.s3_force_path_style} 
                      />
                      <span>Force Path-Style Access (Required for Cloudflare R2, MinIO, LocalStack)</span>
                    </label>
                  </div>
                </div>

                <!-- Cloudflare R2 helper box -->
                <div style="background: rgba(249, 115, 22, 0.05); border: 1px solid rgba(249, 115, 22, 0.2); border-radius: 6px; padding: 0.9rem 1rem; font-size: 0.82rem; color: var(--text-secondary); line-height: 1.5;">
                  <strong style="color: #fb923c;">☁ Cloudflare R2 Quick Setup:</strong>
                  <ul style="margin: 0.4rem 0 0; padding-left: 1.25rem; display: flex; flex-direction: column; gap: 0.2rem;">
                    <li><strong>Endpoint:</strong> <code style="background: rgba(0,0,0,0.3); padding: 0.1rem 0.3rem;">https://&lt;ACCOUNT_ID&gt;.r2.cloudflarestorage.com</code> — do <em>not</em> append the bucket name.</li>
                    <li><strong>Region:</strong> <code style="background: rgba(0,0,0,0.3); padding: 0.1rem 0.3rem;">auto</code></li>
                    <li><strong>Force Path-Style Access:</strong> must be <strong>enabled ✓</strong></li>
                    <li>Use an <strong>R2 API Token</strong> (not your Cloudflare Global API Key) for Access Key / Secret Key.</li>
                  </ul>
                </div>

              </div>
            {/if}

            <div style="display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; border-top: 1px solid rgba(255,255,255,0.05); padding-top: 1rem;">
              {#if storageConfig.storage_type === 's3'}
                <button 
                  type="button" 
                  onclick={handleTestStorageConnection} 
                  class="btn btn-secondary" 
                  style="display: flex; gap: 0.5rem; align-items: center;" 
                  disabled={isTestingStorage || isSavingStorage}
                >
                  {#if isTestingStorage}
                    <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Testing...
                  {:else}
                    Test Connection
                  {/if}
                </button>
              {/if}

              <button 
                type="submit" 
                class="btn btn-primary" 
                style="display: flex; gap: 0.5rem; align-items: center;" 
                disabled={isSavingStorage || isTestingStorage}
              >
                {#if isSavingStorage}
                  <RefreshCw size={16} class="animate-spin" style="animation: spin 1s linear infinite;" /> Saving...
                {:else}
                  Save Settings
                {/if}
              </button>
            </div>

          </div>
        </form>
      </div>
    </div>

  <!-- System Info Tab -->
  {:else if activeTab === 'system'}
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      {#if role === 'Admin'}
        <div class="glass-card" style="padding: 1.5rem;">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.25rem;">
            <div style="display: flex; align-items: center; gap: 0.5rem;">
              <Users size={20} style="color: var(--accent);" />
              <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600; margin: 0;">User Storage & Library Stats</h3>
            </div>
            <button onclick={fetchAdminStats} class="btn btn-secondary" style="display: flex; gap: 0.4rem; align-items: center; padding: 0.4rem 0.75rem; font-size: 0.8rem;" disabled={isLoadingAdminStats}>
              <RefreshCw size={14} class={isLoadingAdminStats ? 'animate-spin' : ''} style={isLoadingAdminStats ? 'animation: spin 1s linear infinite;' : ''} /> Refresh Stats
            </button>
          </div>

          {#if isLoadingAdminStats && adminStats.length === 0}
            <div style="text-align: center; padding: 2rem; color: var(--text-secondary);">
              <RefreshCw size={24} class="animate-spin" style="animation: spin 1s linear infinite; margin: 0 auto 1rem;" />
              <span>Fetching user stats...</span>
            </div>
          {:else if adminStats.length === 0}
            <div style="text-align: center; padding: 2rem; color: var(--text-secondary);">
              No user stats available.
            </div>
          {:else}
            <div style="overflow-x: auto;">
              <table class="library-table" style="width: 100%; font-size: 0.9rem; border-top: 1px solid var(--border-color);">
                <thead>
                  <tr>
                    <th style="text-align: left; padding: 0.75rem;">Username</th>
                    <th style="text-align: left; padding: 0.75rem;">Role</th>
                    <th style="text-align: right; padding: 0.75rem;">Total Tracks</th>
                    <th style="text-align: right; padding: 0.75rem;">Storage Used</th>
                  </tr>
                </thead>
                <tbody>
                  {#each adminStats as stat}
                    <tr>
                      <td style="padding: 0.75rem; font-weight: 600; color: var(--text-primary);">{stat.username}</td>
                      <td style="padding: 0.75rem;">
                        {#if stat.role === 'Admin'}
                          <span style="font-size: 0.7rem; text-transform: uppercase; background: rgba(168, 85, 247, 0.15); color: #c084fc; border: 1px solid rgba(168, 85, 247, 0.2); padding: 0.1rem 0.35rem; border-radius: 4px; font-weight: 600;">Admin</span>
                        {:else if stat.role === 'User'}
                          <span style="font-size: 0.7rem; text-transform: uppercase; background: rgba(59, 130, 246, 0.15); color: #60a5fa; border: 1px solid rgba(59, 130, 246, 0.2); padding: 0.1rem 0.35rem; border-radius: 4px; font-weight: 600;">User</span>
                        {:else}
                          <span style="font-size: 0.7rem; text-transform: uppercase; background: rgba(245, 158, 11, 0.15); color: #fbbf24; border: 1px solid rgba(245, 158, 11, 0.2); padding: 0.1rem 0.35rem; border-radius: 4px; font-weight: 600;">StreamOnly</span>
                        {/if}
                      </td>
                      <td style="padding: 0.75rem; text-align: right; font-family: monospace;">{stat.track_count}</td>
                      <td style="padding: 0.75rem; text-align: right; font-family: monospace;">{formatBytes(stat.total_size_bytes)}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        </div>
      {/if}

      <div class="glass-card" style="padding: 1.5rem;">
        <div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 1.25rem;">
          <Database size={20} style="color: var(--accent);" />
          <h3 style="font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600;">Docker Environment Variables</h3>
        </div>
        <p style="font-size: 0.9rem; color: var(--text-secondary); line-height: 1.4; margin-bottom: 1.25rem;">
          These options are configured during Docker container startup. To change them, edit your <code style="background: rgba(0,0,0,0.3); padding: 0.1rem 0.3rem;">docker-compose.yml</code> file and restart the container.
        </p>

        <div style="overflow-x: auto;">
          <table class="library-table" style="font-size: 0.9rem; border-top: 1px solid var(--border-color);">
            <thead>
              <tr>
                <th style="width: 30%;">Variable</th>
                <th style="width: 50%;">Description</th>
                <th style="width: 20%;">Default</th>
              </tr>
            </thead>
            <tbody>
              {#each envs as env}
                <tr>
                  <td style="font-family: monospace; font-weight: 600; color: var(--text-primary);">{env.name}</td>
                  <td style="color: var(--text-secondary);">{env.desc}</td>
                  <td style="font-family: monospace; color: var(--text-muted); font-size: 0.8rem; word-break: break-all;">{env.default}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

      <div class="glass-card" style="display: flex; gap: 1rem; align-items: flex-start; padding: 1.5rem;">
        <div style="background: rgba(168,85,247,0.1); padding: 0.75rem; border-radius: 10px; color: var(--accent); flex-shrink: 0;">
          <HelpCircle size={24} />
        </div>
        <div style="font-size: 0.9rem; line-height: 1.5;">
          <h4 style="font-family: var(--font-heading); font-size: 1.05rem; font-weight: 600; margin-bottom: 0.25rem; color: var(--text-primary);">Configured File System Layout</h4>
          <p style="color: var(--text-secondary); margin-bottom: 0.5rem;">
            The SQLite database is named <code style="background: rgba(0,0,0,0.3); padding: 0.1rem 0.3rem;">audion.sqlite</code> and is stored in the volume folder.
          </p>
          <div style="font-family: monospace; font-size: 0.8rem; background: rgba(0,0,0,0.2); padding: 0.75rem; border-radius: 6px; border: 1px solid var(--border-color); color: var(--text-muted); display: flex; flex-direction: column; gap: 0.35rem;">
            <div>Database Path: <span style="color: var(--text-secondary);">./data/db/audion.sqlite</span></div>
            <div>Music Directory: <span style="color: var(--text-secondary);">./data/music/</span></div>
            <div>Artwork Directory: <span style="color: var(--text-secondary);">./data/artwork/</span></div>
            <div>Transcoded Cache: <span style="color: var(--text-secondary);">./data/transcoded/</span></div>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .settings-container {
    display: grid;
    grid-template-columns: 1fr;
    gap: 1.5rem;
    width: 100%;
    max-width: 1200px;
  }

  .settings-tabs {
    display: flex;
    gap: 0.35rem;
    overflow-x: auto;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 0.5rem;
    width: 100%;
    scrollbar-width: none;
  }

  .settings-tabs::-webkit-scrollbar {
    display: none;
  }

  .tab-btn {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    padding: 0.6rem 1rem;
    border-radius: 6px;
    font-size: 0.9rem;
    font-weight: 500;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    white-space: nowrap;
    transition: all 0.2s ease;
  }

  .tab-btn:hover {
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-primary);
  }

  .tab-btn.active {
    background: rgba(168, 85, 247, 0.1);
    color: var(--accent);
    font-weight: 600;
  }

  @media (min-width: 768px) {
    .settings-container {
      grid-template-columns: 220px 1fr;
      gap: 2rem;
    }

    .settings-tabs {
      flex-direction: column;
      border-bottom: none;
      border-right: 1px solid var(--border-color);
      padding-bottom: 0;
      padding-right: 1.5rem;
      overflow-x: visible;
      height: fit-content;
    }

    .tab-btn {
      width: 100%;
      justify-content: flex-start;
      padding: 0.75rem 1rem;
    }
  }

  .console-box {
    margin-top: 1rem;
    background: #060608;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 1rem;
    font-family: monospace;
    font-size: 0.85rem;
    color: #e4e4e7;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.8);
  }

  .logs-container {
    max-height: 180px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .log-line {
    white-space: pre-wrap;
    line-height: 1.4;
    word-break: break-all;
    border-left: 2px solid rgba(168,85,247,0.3);
    padding-left: 0.5rem;
  }

  .status-box {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 1rem;
  }

  .quota-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.75rem;
    text-transform: uppercase;
    border-radius: 4px;
    font-weight: 600;
    letter-spacing: 0.05em;
    padding: 0.15rem 0.5rem;
    border: 1px solid transparent;
  }
  .quota-badge.enabled {
    background: rgba(16, 185, 129, 0.15); /* success */
    color: var(--success);
    border-color: rgba(16, 185, 129, 0.3);
  }
  .quota-badge.frozen {
    background: rgba(239, 68, 68, 0.15); /* danger */
    color: var(--danger);
    border-color: rgba(239, 68, 68, 0.3);
  }
  
  .storage-bar-container {
    height: 6px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 999px;
    overflow: hidden;
    width: 100%;
  }
  .storage-bar-fill {
    height: 100%;
    border-radius: 999px;
    transition: width 0.3s ease, background-color 0.3s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slideUp {
    from { transform: translateY(20px); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
  }

  .quota-modal-backdrop {
    position: fixed;
    inset: 0;
    background-color: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    z-index: 500;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    animation: fadeIn 0.2s ease-out forwards;
  }

  .quota-modal {
    width: 100%;
    max-width: 480px;
    background: #0d0d0f;
    border: 1px solid var(--border-color);
    padding: 1.5rem;
    border-radius: 12px;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5), 0 10px 10px -5px rgba(0, 0, 0, 0.5);
    animation: slideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  /* Switch toggle styling */
  .switch-container {
    position: relative;
    display: inline-block;
    width: 46px;
    height: 24px;
    cursor: pointer;
  }

  .switch-container input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(255, 255, 255, 0.08);
    border: 1px solid var(--border-color);
    transition: .3s;
    border-radius: 24px;
  }

  .switch-slider:before {
    position: absolute;
    content: "";
    height: 16px;
    width: 16px;
    left: 3px;
    bottom: 3px;
    background-color: var(--text-muted);
    transition: .3s;
    border-radius: 50%;
  }

  .switch-container input:checked + .switch-slider {
    background-color: rgba(168, 85, 247, 0.2);
    border-color: var(--accent);
  }

  .switch-container input:checked + .switch-slider:before {
    transform: translateX(22px);
    background-color: var(--accent);
  }

  .switch-container input:focus + .switch-slider {
    box-shadow: 0 0 1px var(--accent);
  }
</style>
