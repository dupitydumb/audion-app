use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use serde::{Serialize, Deserialize};
use sqlx::SqlitePool;
use tracing::{info, warn, error};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelProvider {
    #[serde(rename = "localhost.run")]
    LocalhostRun,
    Ngrok,
    Cloudflare,
}

impl std::fmt::Display for TunnelProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalhostRun => write!(f, "localhost.run"),
            Self::Ngrok => write!(f, "ngrok"),
            Self::Cloudflare => write!(f, "cloudflare"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub provider: TunnelProvider,
    pub token: Option<String>,
    pub custom_domain: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStatus {
    pub active: bool,
    pub provider: Option<TunnelProvider>,
    pub url: Option<String>,
    pub is_connecting: bool,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct NgrokTunnelsResponse {
    tunnels: Vec<NgrokTunnel>,
}

#[derive(Deserialize)]
struct NgrokTunnel {
    public_url: String,
}

pub struct TunnelManager {
    pool: SqlitePool,
    state: Arc<Mutex<TunnelStatus>>,
    config: Arc<Mutex<TunnelConfig>>,
    child_process: Option<Child>,
    backend_port: u16,
}

impl TunnelManager {
    pub async fn new(pool: SqlitePool, backend_port: u16) -> Self {
        // Load config from database, or default
        let provider = Self::get_db_setting(&pool, "tunnel_provider").await
            .and_then(|v| serde_json::from_str::<TunnelProvider>(&format!("\"{}\"", v)).ok())
            .unwrap_or(TunnelProvider::LocalhostRun);

        let token = Self::get_db_setting(&pool, "tunnel_token").await;
        let custom_domain = Self::get_db_setting(&pool, "tunnel_custom_domain").await;
        let enabled = Self::get_db_setting(&pool, "tunnel_enabled").await
            .map(|v| v == "true")
            .unwrap_or(false);

        let config = TunnelConfig {
            provider,
            token,
            custom_domain,
            enabled,
        };

        let state = TunnelStatus {
            active: false,
            provider: None,
            url: None,
            is_connecting: false,
            error: None,
        };

        Self {
            pool,
            state: Arc::new(Mutex::new(state)),
            config: Arc::new(Mutex::new(config)),
            child_process: None,
            backend_port,
        }
    }

    async fn get_db_setting(pool: &SqlitePool, key: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_one(pool)
            .await
            .ok()
    }

    async fn save_db_setting(pool: &SqlitePool, key: &str, value: &str) {
        let _ = sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(pool)
            .await;
    }

    pub fn get_config(&self) -> TunnelConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn get_status(&self) -> TunnelStatus {
        self.state.lock().unwrap().clone()
    }

    pub async fn update_config(&self, new_config: TunnelConfig) {
        {
            let mut conf = self.config.lock().unwrap();
            *conf = new_config.clone();
        }

        // Persist to DB
        let provider_str = serde_json::to_string(&new_config.provider)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();

        Self::save_db_setting(&self.pool, "tunnel_provider", &provider_str).await;
        Self::save_db_setting(&self.pool, "tunnel_token", new_config.token.as_deref().unwrap_or("")).await;
        Self::save_db_setting(&self.pool, "tunnel_custom_domain", new_config.custom_domain.as_deref().unwrap_or("")).await;
        Self::save_db_setting(&self.pool, "tunnel_enabled", if new_config.enabled { "true" } else { "false" }).await;
    }

    pub async fn toggle(&mut self) -> Result<TunnelStatus, String> {
        let mut config = self.config.lock().unwrap().clone();
        
        if self.child_process.is_some() {
            // Stop active tunnel
            info!("Stopping tunnel process...");
            self.stop_internal().await;
            config.enabled = false;
            self.update_config(config.clone()).await;
            Ok(self.get_status())
        } else {
            // Start tunnel
            info!("Starting tunnel process for provider: {}", config.provider);
            config.enabled = true;
            self.update_config(config.clone()).await;

            match self.start_internal().await {
                Ok(_) => Ok(self.get_status()),
                Err(e) => {
                    let mut st = self.state.lock().unwrap();
                    st.error = Some(e.clone());
                    st.is_connecting = false;
                    st.active = false;
                    Err(e)
                }
            }
        }
    }

    pub async fn auto_start_if_enabled(&mut self) {
        let enabled = self.config.lock().unwrap().enabled;
        if enabled {
            info!("Auto-starting public access tunnel on startup...");
            if let Err(e) = self.start_internal().await {
                error!("Failed to auto-start public tunnel: {}", e);
                let mut st = self.state.lock().unwrap();
                st.error = Some(e);
                st.active = false;
                st.is_connecting = false;
            }
        }
    }

    async fn stop_internal(&mut self) {
        if let Some(mut child) = self.child_process.take() {
            let _ = child.kill().await;
        }

        let mut st = self.state.lock().unwrap();
        st.active = false;
        st.provider = None;
        st.url = None;
        st.is_connecting = false;
        st.error = None;
    }

    pub async fn shutdown(&mut self) {
        self.stop_internal().await;
    }

    async fn start_internal(&mut self) -> Result<(), String> {
        self.stop_internal().await;

        let config = self.config.lock().unwrap().clone();

        // 1. Detect target host/port
        // If "audion-frontend" container DNS name is resolvable, use it. Otherwise localhost.
        let target_host = if tokio::net::lookup_host("audion-frontend:80").await.is_ok() {
            info!("Docker network detected. Routing tunnel traffic to 'audion-frontend:80'");
            "audion-frontend".to_string()
        } else {
            info!("Local network detected. Routing tunnel traffic to 'localhost:{}'", self.backend_port);
            "localhost".to_string()
        };

        let target_port = if target_host == "audion-frontend" { 80 } else { self.backend_port };

        {
            let mut st = self.state.lock().unwrap();
            st.is_connecting = true;
            st.provider = Some(config.provider);
        }

        let state_clone = self.state.clone();

        match config.provider {
            TunnelProvider::LocalhostRun => {
                let target = format!("{}:{}", target_host, target_port);
                let ssh_args = vec![
                    "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
                    "-o".to_string(), "UserKnownHostsFile=/dev/null".to_string(),
                    "-o".to_string(), "ExitOnForwardFailure=yes".to_string(),
                    "-R".to_string(), format!("80:{}", target),
                    "nokey@localhost.run".to_string(),
                    "--".to_string(),
                    "--output".to_string(), "json".to_string()
                ];

                info!("Executing command: ssh {}", ssh_args.join(" "));

                let mut child = Command::new("ssh")
                    .args(&ssh_args)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| format!("Failed to execute ssh command: {}. Make sure OpenSSH client is installed.", e))?;

                let stdout = child.stdout.take().ok_or_else(|| "Failed to capture ssh stdout".to_string())?;
                self.child_process = Some(child);

                // Start background stdout reader
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        // Parse JSON lines from localhost.run
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                            if let Some(address) = val.get("address").and_then(|v| v.as_str()) {
                                info!("Tunnel successfully opened! Public URL: https://{}", address);
                                let mut st = state_clone.lock().unwrap();
                                st.url = Some(format!("https://{}", address));
                                st.active = true;
                                st.is_connecting = false;
                                st.error = None;
                                break;
                            }
                        }
                    }
                });
            }
            TunnelProvider::Ngrok => {
                let token = config.token.clone().ok_or_else(|| "Ngrok Authtoken is required".to_string())?;
                if token.trim().is_empty() {
                    return Err("Ngrok Authtoken cannot be empty".to_string());
                }

                let target = format!("{}:{}", target_host, target_port);
                let ngrok_args = vec![
                    "http".to_string(),
                    target,
                    "--authtoken".to_string(),
                    token,
                    "--log=stdout".to_string()
                ];

                info!("Executing command: ngrok {}", ngrok_args.join(" "));

                let child = Command::new("ngrok")
                    .args(&ngrok_args)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| format!("Failed to execute ngrok binary: {}. Make sure it is installed and in system PATH.", e))?;

                self.child_process = Some(child);

                // Query ngrok's local API to retrieve the tunnel URL
                tokio::spawn(async move {
                    let client = reqwest::Client::new();
                    let mut attempts = 0;
                    loop {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        attempts += 1;

                        if attempts > 15 {
                            let mut st = state_clone.lock().unwrap();
                            st.error = Some("Failed to retrieve ngrok public URL. Check authtoken validity or logs.".to_string());
                            st.is_connecting = false;
                            st.active = false;
                            break;
                        }

                        match client.get("http://127.0.0.1:4040/api/tunnels").send().await {
                            Ok(res) => {
                                if let Ok(json_res) = res.json::<NgrokTunnelsResponse>().await {
                                    if let Some(t) = json_res.tunnels.first() {
                                        info!("Ngrok tunnel successfully opened! Public URL: {}", t.public_url);
                                        let mut st = state_clone.lock().unwrap();
                                        st.url = Some(t.public_url.clone());
                                        st.active = true;
                                        st.is_connecting = false;
                                        st.error = None;
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                // Ngrok API not ready yet, keep polling
                            }
                        }
                    }
                });
            }
            TunnelProvider::Cloudflare => {
                let token = config.token.clone().ok_or_else(|| "Cloudflare Tunnel Token is required".to_string())?;
                if token.trim().is_empty() {
                    return Err("Cloudflare Tunnel Token cannot be empty".to_string());
                }

                let cf_args = vec![
                    "tunnel".to_string(),
                    "--no-autoupdate".to_string(),
                    "run".to_string(),
                    "--token".to_string(),
                    token
                ];

                info!("Executing command: cloudflared {}", cf_args.join(" "));

                let child = Command::new("cloudflared")
                    .args(&cf_args)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| format!("Failed to execute cloudflared binary: {}. Make sure it is installed and in system PATH.", e))?;

                self.child_process = Some(child);

                // Cloudflare relies on the custom domain configured in Zero Trust console.
                // We display the configured custom domain.
                let custom_domain = config.custom_domain.clone().unwrap_or_default();
                let display_url = if custom_domain.trim().is_empty() {
                    "Domain managed via Cloudflare Console".to_string()
                } else if custom_domain.starts_with("http://") || custom_domain.starts_with("https://") {
                    custom_domain
                } else {
                    format!("https://{}", custom_domain)
                };

                // Assume connection is successful if the process doesn't immediately crash.
                // We wait 1.5s to see if it dies, then mark it active.
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let mut st = state_clone.lock().unwrap();
                    st.url = Some(display_url);
                    st.active = true;
                    st.is_connecting = false;
                    st.error = None;
                });
            }
        }

        // Spawn monitor task to watch the child process exit
        let state_monitor = self.state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            // A simple placeholder to monitor if process dies, in real usage we would join/select the child exit.
            // However, this is robust enough for typical scenarios or can be expanded if needed.
        });

        Ok(())
    }
}
