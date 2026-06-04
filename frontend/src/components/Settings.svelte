<script lang="ts">
  import { Shield, Database, Key, HelpCircle, LogOut } from '@lucide/svelte';

  let { username, onLogout } = $props<{
    username: string;
    onLogout: () => void;
  }>();

  const envs = [
    { name: 'AUDION_ADMIN_USER', desc: 'The administrator username used to access this web UI and sync.', default: 'admin' },
    { name: 'AUDION_ADMIN_PASSWORD', desc: 'The administrator password used to log in.', default: 'changeme' },
    { name: 'AUDION_JWT_SECRET', desc: 'Secret signature key used to encode and sign JWT access tokens.', default: 'your-secret-key-here-change-this-in-production' },
    { name: 'AUDION_DATA_DIR', desc: 'Host storage directory where database (sqlite) and music files are stored.', default: '/data' },
    { name: 'AUDION_PORT', desc: 'The TCP port the server application binds to.', default: '8080' }
  ];
</script>

<div class="page-header">
  <h1 class="page-title">Settings</h1>
  <p class="page-subtitle">Server environment configuration and administration.</p>
</div>

<div style="display: flex; flex-direction: column; gap: 1.5rem;">
  <div class="glass-card" style="display: flex; justify-content: space-between; align-items: center;">
    <div style="display: flex; align-items: center; gap: 1rem;">
      <div style="background: rgba(168,85,247,0.1); padding: 0.75rem; border-radius: 10px; color: var(--accent);">
        <Shield size={24} />
      </div>
      <div>
        <h3 style="font-family: var(--font-heading); font-size: 1.1rem; font-weight: 600; margin-bottom: 0.25rem;">Active Administrator</h3>
        <p style="font-size: 0.85rem; color: var(--text-secondary);">Logged in as <strong style="color: var(--text-primary);">{username}</strong></p>
      </div>
    </div>
    <button onclick={onLogout} class="btn btn-danger" style="display: flex; gap: 0.5rem; align-items: center;">
      <LogOut size={16} /> Log Out
    </button>
  </div>

  <div class="glass-card">
    <div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 1.25rem;">
      <Database size={20} style="color: var(--accent);" />
      <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600;">Environment Variables</h3>
    </div>
    <p style="font-size: 0.9rem; color: var(--text-secondary); line-height: 1.4; margin-bottom: 1rem;">
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

  <div class="glass-card" style="display: flex; gap: 1rem; align-items: flex-start;">
    <div style="background: rgba(168,85,247,0.1); padding: 0.75rem; border-radius: 10px; color: var(--accent); flex-shrink: 0;">
      <HelpCircle size={24} />
    </div>
    <div style="font-size: 0.9rem; line-height: 1.5;">
      <h4 style="font-family: var(--font-heading); font-size: 1.05rem; font-weight: 600; margin-bottom: 0.25rem; color: var(--text-primary);">Need to reset the database?</h4>
      <p style="color: var(--text-secondary);">
        The SQLite database is named <code style="background: rgba(0,0,0,0.3); padding: 0.1rem 0.3rem;">audion.db</code> and is stored in the volume folder. You can reset the server by deleting this file (or resetting the docker volume).
      </p>
    </div>
  </div>
</div>
