<script lang="ts">
  import { Music } from '@lucide/svelte';

  // Props in Svelte 5
  let { onLoginSuccess, addToast } = $props<{
    onLoginSuccess: (token: string, username: string) => void;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  let username = $state('');
  let password = $state('');
  let isLoading = $state(false);

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (!username || !password) {
      addToast('Please enter both username and password', 'error');
      return;
    }

    isLoading = true;
    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password })
      });

      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || 'Invalid username or password');
      }

      const data = await res.json();
      onLoginSuccess(data.token, data.user.username);
      addToast(`Welcome back, ${data.user.username}!`, 'success');
    } catch (err: any) {
      addToast(err.message || 'Connection failed', 'error');
    } finally {
      isLoading = false;
    }
  }
</script>

<div class="login-container">
  <div class="login-card glass-card">
    <div class="brand-section" style="justify-content: center; margin-bottom: 2rem;">
      <div class="brand-logo">
        <Music size={20} />
      </div>
      <h1 class="brand-name" style="margin: 0;">Audion Server</h1>
    </div>

    <h2 style="text-align: center; margin-bottom: 1.5rem; font-family: var(--font-heading); font-weight: 600;">Admin Portal</h2>

    <form onsubmit={handleSubmit}>
      <div class="form-group">
        <label class="form-label" for="username">Username</label>
        <input
          type="text"
          id="username"
          class="form-input"
          placeholder="Enter username"
          bind:value={username}
          disabled={isLoading}
        />
      </div>

      <div class="form-group" style="margin-bottom: 2rem;">
        <label class="form-label" for="password">Password</label>
        <input
          type="password"
          id="password"
          class="form-input"
          placeholder="Enter password"
          bind:value={password}
          disabled={isLoading}
        />
      </div>

      <button type="submit" class="btn btn-primary" style="width: 100%;" disabled={isLoading}>
        {isLoading ? 'Signing In...' : 'Sign In'}
      </button>
    </form>
  </div>
</div>
