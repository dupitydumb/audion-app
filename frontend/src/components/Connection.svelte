<script lang="ts">
  import { Eye, EyeOff, Copy, Check, Terminal, Code2, Radio } from '@lucide/svelte';

  let { token, addToast } = $props<{
    token: string;
    addToast: (message: string, type: 'success' | 'error' | 'info') => void;
  }>();

  let showToken = $state(false);
  let copiedUrl = $state(false);
  let copiedToken = $state(false);
  let activeLang = $state<'curl' | 'js'>('curl');

  const serverUrl = typeof window !== 'undefined' ? window.location.origin : 'http://localhost:8080';

  function copyToClipboard(text: string, type: 'url' | 'token') {
    navigator.clipboard.writeText(text);
    addToast(`${type === 'url' ? 'Server URL' : 'Token'} copied to clipboard`, 'success');
    
    if (type === 'url') {
      copiedUrl = true;
      setTimeout(() => copiedUrl = false, 2000);
    } else {
      copiedToken = true;
      setTimeout(() => copiedToken = false, 2000);
    }
  }

  // Code snippets
  let curlUpload = $derived(`$ curl -X POST \\
  -H "Authorization: Bearer \${token ? token.substring(0, 10) + '...' : 'YOUR_TOKEN'}" \\
  -F "file=@song.mp3" \\
  \${serverUrl}/api/tracks`);

  let curlStats = $derived(`$ curl -H "Authorization: Bearer \${token ? token.substring(0, 10) + '...' : 'YOUR_TOKEN'}" \\
  \${serverUrl}/api/stats`);

  let jsFetch = $derived(`fetch('\${serverUrl}/api/stats', {
  headers: {
    'Authorization': 'Bearer \${token ? token.substring(0, 10) + '...' : 'YOUR_TOKEN'}'
  }
})
.then(res => res.json())
.then(stats => console.log(stats));`);
</script>

<div class="page-header">
  <h1 class="page-title">API & Connection</h1>
  <p class="page-subtitle">Manage connection settings and retrieve API credentials for external integrations.</p>
</div>

<div style="display: flex; flex-direction: column; gap: 1.5rem;">
  <div class="glass-card">
    <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600; margin-bottom: 1.25rem;">Server Address</h3>
    <div style="display: flex; gap: 0.75rem;">
      <input type="text" class="form-input" readonly value={serverUrl} style="flex: 1; font-family: monospace;" />
      <button onclick={() => copyToClipboard(serverUrl, 'url')} class="btn btn-secondary" style="padding: 0 1rem;">
        {#if copiedUrl}
          <Check size={18} style="color: var(--success);" />
        {:else}
          <Copy size={18} />
        {/if}
      </button>
    </div>
    <p style="font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.5rem;">
      This is the host address your Audion app will use to sync with this server.
    </p>
  </div>

  <div class="glass-card">
    <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600; margin-bottom: 1.25rem;">JWT Access Token</h3>
    <div style="display: flex; gap: 0.75rem; align-items: center; margin-bottom: 1rem;">
      <div style="position: relative; flex: 1; display: flex;">
        <input 
          type={showToken ? 'text' : 'password'} 
          class="form-input" 
          readonly 
          value={token} 
          style="flex: 1; font-family: monospace; padding-right: 3rem; text-overflow: ellipsis;" 
        />
        <button 
          onclick={() => showToken = !showToken} 
          class="btn" 
          style="position: absolute; right: 4px; top: 4px; bottom: 4px; background: transparent; border: none; padding: 0 0.5rem; color: var(--text-secondary);"
        >
          {#if showToken}
            <EyeOff size={16} />
          {:else}
            <Eye size={16} />
          {/if}
        </button>
      </div>
      <button onclick={() => copyToClipboard(token, 'token')} class="btn btn-secondary" style="padding: 0 1rem;">
        {#if copiedToken}
          <Check size={18} style="color: var(--success);" />
        {:else}
          <Copy size={18} />
        {/if}
      </button>
    </div>
    <p style="font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.5rem; line-height: 1.4;">
      This JSON Web Token (JWT) allows passwordless, secure sync authentication. Keep it private. It is automatically valid for 30 days.
    </p>
  </div>

  <div class="glass-card">
    <div style="display: flex; align-items: center; gap: 0.75rem; margin-bottom: 1.25rem;">
      <Radio size={20} style="color: var(--accent);" />
      <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600; margin: 0;">Subsonic Client Integration</h3>
    </div>
    <p style="font-size: 0.9rem; color: var(--text-secondary); line-height: 1.5; margin-bottom: 1.25rem;">
      Audion Server exposes a standard Subsonic-compliant API at the root level, making it compatible with a wide range of external music streaming clients.
    </p>
    
    <div style="display: grid; grid-template-columns: 120px 1fr; gap: 0.75rem; font-size: 0.9rem; align-items: center; background: rgba(0,0,0,0.15); padding: 1rem; border-radius: 6px; border: 1px solid var(--border-color); font-family: var(--font-mono, monospace);">
      <span style="color: var(--text-secondary); font-weight: 600;">Server URL:</span>
      <span style="color: var(--text-primary); word-break: break-all;">{serverUrl}</span>
      
      <span style="color: var(--text-secondary); font-weight: 600;">Username:</span>
      <span style="color: var(--text-primary);">Your account username</span>

      <span style="color: var(--text-secondary); font-weight: 600;">Password:</span>
      <span style="color: var(--text-primary);">Your account password</span>
    </div>

    <div style="margin-top: 1.25rem; font-size: 0.85rem; color: var(--text-secondary); line-height: 1.5;">
      <h4 style="font-weight: 600; color: var(--text-primary); margin-bottom: 0.5rem;">Supported Mobile & Desktop Clients:</h4>
      <ul style="margin: 0; padding-left: 1.25rem; display: flex; flex-direction: column; gap: 0.25rem;">
        <li><strong>Android:</strong> Symfonium, DSub, Substreamer, Audinaut, UltraSonic</li>
        <li><strong>iOS:</strong> play:Sub, Substreamer, Amuse, AVSub</li>
        <li><strong>Desktop/Web:</strong> Feishin, Sonixd, Sublime Music</li>
      </ul>
    </div>
  </div>

  <div class="glass-card">
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem;">
      <h3 style="font-family: var(--font-heading); font-size: 1.2rem; font-weight: 600;">Developer API Examples</h3>
      <div style="display: flex; gap: 0.25rem; background: rgba(0,0,0,0.2); padding: 0.25rem; border-radius: 6px; border: 1px solid var(--border-color);">
        <button 
          onclick={() => activeLang = 'curl'} 
          class="btn {activeLang === 'curl' ? 'btn-primary' : 'btn-secondary'}" 
          style="padding: 0.25rem 0.75rem; font-size: 0.8rem; border: none;"
        >
          <Terminal size={14} style="margin-right: 0.25rem;" /> cURL
        </button>
        <button 
          onclick={() => activeLang = 'js'} 
          class="btn {activeLang === 'js' ? 'btn-primary' : 'btn-secondary'}" 
          style="padding: 0.25rem 0.75rem; font-size: 0.8rem; border: none;"
        >
          <Code2 size={14} style="margin-right: 0.25rem;" /> Fetch (JS)
        </button>
      </div>
    </div>

    {#if activeLang === 'curl'}
      <div>
        <p style="font-size: 0.85rem; color: var(--text-secondary); margin-bottom: 0.5rem;">Upload audio track:</p>
        <div class="code-block-wrapper">
          <div class="code-block-header">
            <span>bash</span>
          </div>
          <pre class="code-block">{curlUpload}</pre>
        </div>

        <p style="font-size: 0.85rem; color: var(--text-secondary); margin-top: 1rem; margin-bottom: 0.5rem;">Query stats:</p>
        <div class="code-block-wrapper">
          <div class="code-block-header">
            <span>bash</span>
          </div>
          <pre class="code-block">{curlStats}</pre>
        </div>
      </div>
    {:else}
      <div>
        <p style="font-size: 0.85rem; color: var(--text-secondary); margin-bottom: 0.5rem;">Call stats endpoint in JavaScript:</p>
        <div class="code-block-wrapper">
          <div class="code-block-header">
            <span>javascript</span>
          </div>
          <pre class="code-block">{jsFetch}</pre>
        </div>
      </div>
    {/if}
  </div>
</div>
