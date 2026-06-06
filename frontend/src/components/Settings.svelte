<script lang="ts">
  import { Shield, Database, Key, HelpCircle, LogOut, RefreshCw, FolderSync, Trash2, User, Users, Cpu, FileText, CheckCircle2, AlertTriangle, Edit2, UserPlus, Check, X } from '@lucide/svelte';

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

  let activeTab = $state<'profile' | 'users' | 'library' | 'system'>('profile');

  // Profile Form States
  let currentPassword = $state('');
  let newUsername = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let isSavingProfile = $state(false);

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
  let usersList = $state<any[]>([]);
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

  $effect(() => {
    if (activeTab === 'users' && role === 'Admin') {
      fetchUsers();
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
    {/if}
    {#if role === 'Admin'}
    <button 
      onclick={() => activeTab = 'library'} 
      class="tab-btn {activeTab === 'library' ? 'active' : ''}"
    >
      <FolderSync size={16} /> Library Control
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

      <div class="glass-card" style="padding: 1.5rem;">
        {#if isLoadingUsers}
          <div style="text-align: center; padding: 2rem; color: var(--text-secondary);">
            <RefreshCw size={24} class="animate-spin" style="animation: spin 1s linear infinite; margin: 0 auto 1rem;" />
            <span>Loading user list...</span>
          </div>
        {:else if usersList.length === 0}
          <div style="text-align: center; padding: 2rem; color: var(--text-secondary);">
            No users registered.
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
                  <th style="text-align: right; padding: 0.75rem;">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each usersList as u}
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
                      {:else}
                        <span style="font-size: 0.8rem; text-transform: uppercase; background: rgba(255,255,255,0.06); padding: 0.15rem 0.4rem; border-radius: 4px; border: 1px solid var(--border-color); color: var(--text-secondary);">{u.role}</span>
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
                    <td style="padding: 0.75rem; text-align: right; vertical-align: middle;">
                      {#if editingUserId === u.id}
                        <div style="display: flex; gap: 0.5rem; justify-content: flex-end;">
                          <button onclick={() => handleUpdateUser(u.id)} class="btn btn-primary" style="padding: 0.35rem 0.5rem; font-size: 0.8rem;" disabled={isUpdating}>
                            Save
                          </button>
                          <button onclick={() => editingUserId = null} class="btn btn-secondary" style="padding: 0.35rem 0.5rem; font-size: 0.8rem;">
                            Cancel
                          </button>
                        </div>
                      {:else}
                        <div style="display: flex; gap: 0.5rem; justify-content: flex-end;">
                          <button onclick={() => startEditing(u)} class="btn btn-secondary" style="padding: 0.35rem 0.5rem; font-size: 0.8rem; display: flex; align-items: center; gap: 0.25rem;">
                            <Edit2 size={12} /> Edit
                          </button>
                          {#if u.username !== username}
                            <button onclick={() => handleDeleteUser(u.id, u.username)} class="btn btn-danger" style="padding: 0.35rem 0.5rem; font-size: 0.8rem;">
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
    </div>

  <!-- Library Management Tab -->
  {:else if activeTab === 'library' && role === 'Admin'}
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

  <!-- System Info Tab -->
  {:else if activeTab === 'system'}
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
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

  .settings-container {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    width: 100%;
    max-width: 800px;
  }

  .settings-tabs {
    display: flex;
    gap: 0.5rem;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 0.5rem;
    width: 100%;
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
</style>
