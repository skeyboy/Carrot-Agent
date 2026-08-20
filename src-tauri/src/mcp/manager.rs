use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rmcp::ClientHandler;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResponse};
use rmcp::service::{NotificationContext, RoleClient, RunningService};
use rmcp::transport::TokioChildProcess;
use rmcp::transport::auth::{
    AuthClient, AuthError, AuthorizationManager, AuthorizationRequest,
    CredentialStore as RmcpCredentialStore, OAuthState, StoredCredentials,
};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio_util::sync::CancellationToken;

use crate::credentials::CredentialStore;
use crate::domain::mcp::{
    McpAuthKind, McpCatalogSnapshot, McpConnectionState, McpPresetKind, McpServerConfig,
    McpServerSummary, McpSystemSettings, McpToolDescriptor, McpToolPolicy, McpToolSummary,
    McpTransportKind,
};
use crate::tools::{AgentTool, ToolRegistry};

use super::adapter::McpToolAdapter;
use super::config::{McpConfigError, McpConfigStore};
use super::isolation::isolated_command;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(15);
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESULT_BYTES: usize = 256 * 1024;
const MAX_TOOLS_PER_SERVER: usize = 128;
const MAX_CATALOG_SCHEMA_BYTES: usize = 1024 * 1024;
const MAX_SSE_EVENT_BYTES: usize = 256 * 1024;
const FILESYSTEM_PACKAGE: &str = "@modelcontextprotocol/server-filesystem@2026.7.10";
const BRAVE_SEARCH_PACKAGE: &str = "@brave/brave-search-mcp-server@2.1.0";

type ClientService = RunningService<RoleClient, CatalogClientHandler>;

#[derive(Clone)]
struct CatalogClientHandler {
    server_id: String,
    refresh_tx: mpsc::UnboundedSender<String>,
}

impl ClientHandler for CatalogClientHandler {
    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        let _ = self.refresh_tx.send(self.server_id.clone());
    }
}

struct McpConnection {
    service: Mutex<Option<ClientService>>,
    tools: RwLock<Vec<McpToolDescriptor>>,
    revision: AtomicU64,
}

#[derive(Debug, Clone)]
struct RuntimeStatus {
    state: McpConnectionState,
    error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("MCP server '{0}' was not found")]
    ServerNotFound(String),
    #[error("MCP server '{0}' is not connected")]
    NotConnected(String),
    #[error("MCP configuration error: {0}")]
    Configuration(String),
    #[error("MCP server failed to start: {0}")]
    Start(String),
    #[error("MCP protocol error: {0}")]
    Protocol(String),
    #[error("MCP tool execution failed: {0}")]
    Tool(String),
    #[error("MCP policy rejected the operation: {0}")]
    Policy(String),
    #[error("MCP authorization failed: {0}")]
    Authorization(String),
    #[error("MCP request was cancelled")]
    Cancelled,
    #[error("MCP request timed out")]
    Timeout,
}

pub struct McpClientManager {
    store: McpConfigStore,
    configs: RwLock<Vec<McpServerConfig>>,
    system: RwLock<McpSystemSettings>,
    connections: RwLock<HashMap<String, Arc<McpConnection>>>,
    statuses: RwLock<HashMap<String, RuntimeStatus>>,
    credentials: Arc<dyn CredentialStore>,
    refresh_tx: mpsc::UnboundedSender<String>,
    catalog_revision: AtomicU64,
    oauth_sessions: Mutex<HashMap<String, OAuthSession>>,
}

struct OAuthSession {
    state: OAuthState,
    redirect_uri: String,
}

#[derive(Clone)]
struct KeychainOAuthStore {
    credentials: Arc<dyn CredentialStore>,
    reference: String,
}

#[async_trait::async_trait]
impl RmcpCredentialStore for KeychainOAuthStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let value = self
            .credentials
            .get(&self.reference)
            .await
            .map_err(|error| AuthError::InternalError(error.to_string()))?;
        value
            .map(|value| {
                serde_json::from_str(&value)
                    .map_err(|error| AuthError::InternalError(error.to_string()))
            })
            .transpose()
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let value = serde_json::to_string(&credentials)
            .map_err(|error| AuthError::InternalError(error.to_string()))?;
        self.credentials
            .set(&self.reference, value)
            .await
            .map_err(|error| AuthError::InternalError(error.to_string()))
    }

    async fn clear(&self) -> Result<(), AuthError> {
        self.credentials
            .delete(&self.reference)
            .await
            .map_err(|error| AuthError::InternalError(error.to_string()))
    }
}

impl McpClientManager {
    pub async fn install_preset(
        self: &Arc<Self>,
        preset: McpPresetKind,
        workspace_path: Option<String>,
    ) -> Result<McpCatalogSnapshot, McpError> {
        #[cfg(not(target_os = "macos"))]
        return Err(McpError::Policy(
            "curated MCP preset installation currently supports macOS only".to_owned(),
        ));

        #[cfg(target_os = "macos")]
        {
            let (package, binary) = match preset {
                McpPresetKind::WorkspaceFilesystem => (FILESYSTEM_PACKAGE, "mcp-server-filesystem"),
                McpPresetKind::BraveSearch => (BRAVE_SEARCH_PACKAGE, "brave-search-mcp-server"),
            };
            let package_root = self
                .store
                .path()
                .parent()
                .unwrap_or_else(|| Path::new("/tmp"))
                .join("mcp-packages");
            tokio::fs::create_dir_all(&package_root)
                .await
                .map_err(|error| McpError::Start(error.to_string()))?;
            let package_root = std::fs::canonicalize(&package_root)
                .map_err(|error| McpError::Start(error.to_string()))?;
            let npm = find_executable("npm")?;
            let output = tokio::process::Command::new(npm)
                .args([
                    "install",
                    "--ignore-scripts",
                    "--no-audit",
                    "--no-fund",
                    "--save-exact",
                    "--prefix",
                ])
                .arg(&package_root)
                .arg(package)
                .output()
                .await
                .map_err(|error| McpError::Start(format!("npm install failed: {error}")))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(McpError::Start(format!(
                    "trusted package installation failed: {}",
                    stderr.trim().chars().take(500).collect::<String>()
                )));
            }
            let executable = package_root.join("node_modules/.bin").join(binary);
            let workspace = match preset {
                McpPresetKind::WorkspaceFilesystem => {
                    let raw = workspace_path.ok_or_else(|| {
                        McpError::Configuration("workspace path is required".to_owned())
                    })?;
                    let path = std::fs::canonicalize(&raw).map_err(|error| {
                        McpError::Configuration(format!(
                            "workspace '{raw}' cannot be resolved: {error}"
                        ))
                    })?;
                    if !path.is_dir() {
                        return Err(McpError::Configuration(
                            "workspace path must be a directory".to_owned(),
                        ));
                    }
                    Some(path.to_string_lossy().into_owned())
                }
                McpPresetKind::BraveSearch => None,
            };
            let config = match preset {
                McpPresetKind::WorkspaceFilesystem => McpServerConfig {
                    id: "workspace-files".to_owned(),
                    label: "Workspace Files".to_owned(),
                    enabled: true,
                    transport: McpTransportKind::Stdio,
                    executable: executable.to_string_lossy().into_owned(),
                    arguments: vec![workspace.clone().expect("workspace validated")],
                    working_directory: Some(package_root.to_string_lossy().into_owned()),
                    url: None,
                    auth: McpAuthKind::None,
                    oauth_client_id: None,
                    oauth_scopes: Vec::new(),
                    preset: Some(preset),
                    secret_environment_variable: None,
                    read_directories: vec![workspace.expect("workspace validated")],
                    allowed_directories: Vec::new(),
                    allowed_domains: Vec::new(),
                    allow_network: false,
                    tool_policies: Vec::new(),
                },
                McpPresetKind::BraveSearch => McpServerConfig {
                    id: "brave-search".to_owned(),
                    label: "Brave Search".to_owned(),
                    enabled: true,
                    transport: McpTransportKind::Stdio,
                    executable: executable.to_string_lossy().into_owned(),
                    arguments: vec!["--transport".to_owned(), "stdio".to_owned()],
                    working_directory: Some(package_root.to_string_lossy().into_owned()),
                    url: None,
                    auth: McpAuthKind::Bearer,
                    oauth_client_id: None,
                    oauth_scopes: Vec::new(),
                    preset: Some(preset),
                    secret_environment_variable: Some("BRAVE_API_KEY".to_owned()),
                    read_directories: Vec::new(),
                    allowed_directories: Vec::new(),
                    allowed_domains: Vec::new(),
                    allow_network: true,
                    tool_policies: Vec::new(),
                },
            };
            self.create_server(config).await?;
            let connection = self
                .connections
                .read()
                .await
                .get(match preset {
                    McpPresetKind::WorkspaceFilesystem => "workspace-files",
                    McpPresetKind::BraveSearch => "brave-search",
                })
                .cloned();
            if let Some(connection) = connection {
                let policies = connection
                    .tools
                    .read()
                    .await
                    .iter()
                    .filter(|tool| {
                        preset == McpPresetKind::BraveSearch || tool.read_only_hint == Some(true)
                    })
                    .map(|tool| McpToolPolicy {
                        name: tool.remote_name.clone(),
                        enabled: true,
                        risk: crate::tools::ToolRisk::ReadOnly,
                        idempotent: true,
                        reconcile: true,
                    })
                    .collect::<Vec<_>>();
                let mut configs = self.configs.read().await.clone();
                if let Some(config) = configs
                    .iter_mut()
                    .find(|config| config.preset == Some(preset))
                {
                    config.tool_policies = policies;
                }
                self.save_configs(configs).await?;
                self.catalog_revision.fetch_add(1, Ordering::Relaxed);
            }
            Ok(self.snapshot().await)
        }
    }

    pub async fn initialize(
        store: McpConfigStore,
        credentials: Arc<dyn CredentialStore>,
    ) -> Result<Arc<Self>, McpConfigError> {
        let stored = store.ensure_and_load_all().await?;
        let configs = stored.servers;
        let (refresh_tx, mut refresh_rx) = mpsc::unbounded_channel();
        let manager = Arc::new(Self {
            store,
            configs: RwLock::new(configs.clone()),
            system: RwLock::new(stored.system),
            connections: RwLock::new(HashMap::new()),
            statuses: RwLock::new(HashMap::new()),
            credentials,
            refresh_tx,
            catalog_revision: AtomicU64::new(1),
            oauth_sessions: Mutex::new(HashMap::new()),
        });
        let weak = Arc::downgrade(&manager);
        tokio::spawn(async move {
            while let Some(server_id) = refresh_rx.recv().await {
                let Some(manager) = weak.upgrade() else { break };
                if !manager.system.read().await.dynamic_updates {
                    continue;
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
                if let Err(error) = manager.refresh_server_tools(&server_id).await {
                    manager
                        .set_status(
                            &server_id,
                            McpConnectionState::Degraded,
                            Some(format!("tool catalog refresh failed: {error}")),
                        )
                        .await;
                    manager.recover_connection(&server_id).await;
                }
            }
        });
        for config in configs {
            manager.statuses.write().await.insert(
                config.id.clone(),
                RuntimeStatus {
                    state: if config.enabled {
                        McpConnectionState::Disconnected
                    } else {
                        McpConnectionState::Disabled
                    },
                    error: None,
                },
            );
        }
        let enabled = manager
            .configs
            .read()
            .await
            .iter()
            .filter(|config| config.enabled)
            .map(|config| config.id.clone())
            .collect::<Vec<_>>();
        for id in enabled {
            let _ = manager.connect(&id).await;
        }
        Ok(manager)
    }

    pub fn config_path(&self) -> String {
        self.store.path().display().to_string()
    }

    pub async fn set_auth_secret(
        self: &Arc<Self>,
        server_id: &str,
        secret: String,
    ) -> Result<(), McpError> {
        let config = self
            .configs
            .read()
            .await
            .iter()
            .find(|config| config.id == server_id)
            .cloned()
            .ok_or_else(|| McpError::ServerNotFound(server_id.to_owned()))?;
        if config.auth == McpAuthKind::None {
            return Err(McpError::Configuration(
                "server does not use authentication".to_owned(),
            ));
        }
        if !self.system.read().await.secure_auth {
            return Err(McpError::Policy(
                "MCP Keychain authentication support is disabled".to_owned(),
            ));
        }
        self.credentials
            .set(&credential_reference(server_id), secret)
            .await
            .map_err(|error| McpError::Authorization(error.to_string()))?;
        if config.enabled {
            self.connect(server_id).await?;
        }
        Ok(())
    }

    pub async fn clear_auth(self: &Arc<Self>, server_id: &str) -> Result<(), McpError> {
        if !self
            .configs
            .read()
            .await
            .iter()
            .any(|config| config.id == server_id)
        {
            return Err(McpError::ServerNotFound(server_id.to_owned()));
        }
        self.credentials
            .delete(&credential_reference(server_id))
            .await
            .map_err(|error| McpError::Authorization(error.to_string()))?;
        self.disconnect(server_id).await?;
        Ok(())
    }

    pub async fn begin_oauth(
        &self,
        server_id: &str,
        redirect_uri: String,
    ) -> Result<crate::domain::mcp::McpOAuthStart, McpError> {
        if !self.system.read().await.secure_auth {
            return Err(McpError::Policy(
                "MCP Keychain authentication support is disabled".to_owned(),
            ));
        }
        validate_oauth_redirect(&redirect_uri)?;
        let config = self
            .configs
            .read()
            .await
            .iter()
            .find(|config| config.id == server_id)
            .cloned()
            .ok_or_else(|| McpError::ServerNotFound(server_id.to_owned()))?;
        if config.transport != McpTransportKind::StreamableHttp || config.auth != McpAuthKind::Oauth
        {
            return Err(McpError::Configuration(
                "OAuth requires an OAuth-enabled Streamable HTTP server".to_owned(),
            ));
        }
        let url = config
            .url
            .as_deref()
            .ok_or_else(|| McpError::Configuration("HTTP server URL is missing".to_owned()))?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| McpError::Authorization(error.to_string()))?;
        let mut state = OAuthState::new(url, Some(client))
            .await
            .map_err(|error| McpError::Authorization(error.to_string()))?;
        if let OAuthState::Unauthorized(manager) = &mut state {
            manager.set_credential_store(self.oauth_store(server_id));
        }
        let mut request = AuthorizationRequest::new(redirect_uri.clone())
            .with_client_name("Carrot Desktop")
            .with_scopes(config.oauth_scopes);
        if let Some(client_id) = config
            .oauth_client_id
            .filter(|value| !value.trim().is_empty())
        {
            request = request.with_preregistered_client(client_id);
        }
        state
            .start_authorization(request)
            .await
            .map_err(|error| McpError::Authorization(error.to_string()))?;
        let authorization_url = state
            .get_authorization_url()
            .await
            .map_err(|error| McpError::Authorization(error.to_string()))?;
        self.oauth_sessions.lock().await.insert(
            server_id.to_owned(),
            OAuthSession {
                state,
                redirect_uri,
            },
        );
        Ok(crate::domain::mcp::McpOAuthStart {
            server_id: server_id.to_owned(),
            authorization_url,
        })
    }

    pub async fn complete_oauth(
        self: &Arc<Self>,
        server_id: &str,
        callback_url: String,
    ) -> Result<McpCatalogSnapshot, McpError> {
        if !self.system.read().await.secure_auth {
            return Err(McpError::Policy(
                "MCP Keychain authentication support is disabled".to_owned(),
            ));
        }
        let mut session = self
            .oauth_sessions
            .lock()
            .await
            .remove(server_id)
            .ok_or_else(|| {
                McpError::Authorization("OAuth session expired; start again".to_owned())
            })?;
        validate_oauth_callback(&callback_url, &session.redirect_uri)?;
        session
            .state
            .handle_callback_url(&callback_url)
            .await
            .map_err(|error| McpError::Authorization(error.to_string()))?;
        let _credentials = session
            .state
            .get_credentials()
            .await
            .map_err(|error| McpError::Authorization(error.to_string()))?;
        self.connect(server_id).await?;
        Ok(self.snapshot().await)
    }

    fn oauth_store(&self, server_id: &str) -> KeychainOAuthStore {
        KeychainOAuthStore {
            credentials: self.credentials.clone(),
            reference: credential_reference(server_id),
        }
    }

    pub async fn snapshot(&self) -> McpCatalogSnapshot {
        let configs = self.configs.read().await.clone();
        let statuses = self.statuses.read().await.clone();
        let connections = self.connections.read().await;
        let mut servers = Vec::new();
        for config in configs {
            let status = statuses.get(&config.id).cloned().unwrap_or(RuntimeStatus {
                state: McpConnectionState::Disconnected,
                error: None,
            });
            let mut tools = if let Some(connection) = connections.get(&config.id) {
                connection
                    .tools
                    .read()
                    .await
                    .iter()
                    .map(|tool| {
                        let policy = policy_for(&config, &tool.remote_name);
                        McpToolSummary {
                            name: tool.remote_name.clone(),
                            alias: tool.alias.clone(),
                            title: tool.title.clone(),
                            description: tool.description.clone(),
                            schema_hash: tool.schema_hash.clone(),
                            read_only_hint: tool.read_only_hint,
                            enabled: policy.is_some_and(|policy| policy.enabled),
                            risk: policy
                                .map_or(crate::tools::ToolRisk::ExternalSideEffect, |policy| {
                                    policy.risk
                                }),
                            idempotent: policy.is_some_and(|policy| policy.idempotent),
                            reconcile: policy.is_some_and(|policy| policy.reconcile),
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            tools.sort_by(|left, right| left.name.cmp(&right.name));
            let auth_configured = if config.auth == McpAuthKind::None {
                true
            } else {
                self.credentials
                    .contains(&credential_reference(&config.id))
                    .await
                    .unwrap_or(false)
            };
            let catalog_revision = connections
                .get(&config.id)
                .map_or(0, |connection| connection.revision.load(Ordering::Relaxed))
                .to_string();
            servers.push(McpServerSummary {
                config,
                state: status.state,
                error: status.error,
                tools,
                auth_configured,
                catalog_revision,
            });
        }
        servers.sort_by(|left, right| left.config.label.cmp(&right.config.label));
        McpCatalogSnapshot {
            config_path: self.config_path(),
            system: self.system.read().await.clone(),
            servers,
            revision: self.catalog_revision.load(Ordering::Relaxed).to_string(),
        }
    }

    pub async fn create_server(
        self: &Arc<Self>,
        config: McpServerConfig,
    ) -> Result<McpCatalogSnapshot, McpError> {
        let mut configs = self.configs.read().await.clone();
        if configs.iter().any(|item| item.id == config.id) {
            return Err(McpError::Configuration(format!(
                "server '{}' already exists",
                config.id
            )));
        }
        configs.push(config.clone());
        self.save_configs(configs).await?;
        self.statuses.write().await.insert(
            config.id.clone(),
            RuntimeStatus {
                state: if config.enabled {
                    McpConnectionState::Disconnected
                } else {
                    McpConnectionState::Disabled
                },
                error: None,
            },
        );
        if config.enabled {
            let _ = self.connect(&config.id).await;
        }
        Ok(self.snapshot().await)
    }

    pub async fn update_system_settings(
        self: &Arc<Self>,
        settings: McpSystemSettings,
    ) -> Result<McpCatalogSnapshot, McpError> {
        let configs = self.configs.read().await.clone();
        self.store
            .save_all(&settings, &configs)
            .await
            .map_err(|error| McpError::Configuration(error.to_string()))?;
        *self.system.write().await = settings;
        self.catalog_revision.fetch_add(1, Ordering::Relaxed);
        self.reconnect_enabled().await;
        Ok(self.snapshot().await)
    }

    pub async fn update_server(
        self: &Arc<Self>,
        config: McpServerConfig,
    ) -> Result<McpCatalogSnapshot, McpError> {
        let mut configs = self.configs.read().await.clone();
        let current = configs
            .iter_mut()
            .find(|item| item.id == config.id)
            .ok_or_else(|| McpError::ServerNotFound(config.id.clone()))?;
        let auth_boundary_changed = current.url != config.url
            || current.auth != config.auth
            || current.oauth_client_id != config.oauth_client_id;
        *current = config.clone();
        self.save_configs(configs).await?;
        self.disconnect(&config.id).await?;
        if auth_boundary_changed {
            self.credentials
                .delete(&credential_reference(&config.id))
                .await
                .map_err(|error| McpError::Authorization(error.to_string()))?;
        }
        if config.enabled {
            let _ = self.connect(&config.id).await;
        } else {
            self.set_status(&config.id, McpConnectionState::Disabled, None)
                .await;
        }
        Ok(self.snapshot().await)
    }

    pub async fn delete_server(self: &Arc<Self>, id: &str) -> Result<McpCatalogSnapshot, McpError> {
        let mut configs = self.configs.read().await.clone();
        if !configs.iter().any(|item| item.id == id) {
            return Err(McpError::ServerNotFound(id.to_owned()));
        }
        self.disconnect(id).await?;
        if configs
            .iter()
            .any(|config| config.id == id && config.auth != McpAuthKind::None)
        {
            self.credentials
                .delete(&credential_reference(id))
                .await
                .map_err(|error| McpError::Authorization(error.to_string()))?;
        }
        configs.retain(|item| item.id != id);
        self.save_configs(configs).await?;
        self.statuses.write().await.remove(id);
        Ok(self.snapshot().await)
    }

    pub async fn set_tool_policy(
        self: &Arc<Self>,
        server_id: &str,
        policy: McpToolPolicy,
    ) -> Result<McpCatalogSnapshot, McpError> {
        let connection = self.connections.read().await.get(server_id).cloned();
        let known = if let Some(connection) = connection {
            connection
                .tools
                .read()
                .await
                .iter()
                .any(|tool| tool.remote_name == policy.name)
        } else {
            false
        };
        if !known {
            return Err(McpError::Tool(format!(
                "tool '{}' is not available from server '{server_id}'",
                policy.name
            )));
        }
        let mut configs = self.configs.read().await.clone();
        let config = configs
            .iter_mut()
            .find(|item| item.id == server_id)
            .ok_or_else(|| McpError::ServerNotFound(server_id.to_owned()))?;
        config.tool_policies.retain(|item| item.name != policy.name);
        config.tool_policies.push(policy);
        config
            .tool_policies
            .sort_by(|left, right| left.name.cmp(&right.name));
        self.save_configs(configs).await?;
        self.catalog_revision.fetch_add(1, Ordering::Relaxed);
        self.connect(server_id).await?;
        Ok(self.snapshot().await)
    }

    async fn save_configs(&self, configs: Vec<McpServerConfig>) -> Result<(), McpError> {
        let system = self.system.read().await.clone();
        let saved = self
            .store
            .save_all(&system, &configs)
            .await
            .map_err(|error| McpError::Configuration(error.to_string()))?;
        *self.configs.write().await = saved.servers;
        Ok(())
    }

    pub async fn connect(self: &Arc<Self>, id: &str) -> Result<(), McpError> {
        let config = self
            .configs
            .read()
            .await
            .iter()
            .find(|config| config.id == id)
            .cloned()
            .ok_or_else(|| McpError::ServerNotFound(id.to_owned()))?;
        self.disconnect(id).await?;
        self.set_status(id, McpConnectionState::Connecting, None)
            .await;
        match self.start_connection(&config).await {
            Ok(connection) => {
                self.connections
                    .write()
                    .await
                    .insert(id.to_owned(), Arc::new(connection));
                self.set_status(id, McpConnectionState::Ready, None).await;
                self.catalog_revision.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(error) => {
                self.set_status(id, McpConnectionState::Failed, Some(error.to_string()))
                    .await;
                Err(error)
            }
        }
    }

    async fn start_connection(&self, config: &McpServerConfig) -> Result<McpConnection, McpError> {
        let system = self.system.read().await.clone();
        match config.transport {
            McpTransportKind::Stdio
                if !system.controlled_local_tools
                    && config.tool_policies.iter().any(|policy| {
                        policy.enabled && policy.risk != crate::tools::ToolRisk::ReadOnly
                    }) =>
            {
                return Err(McpError::Policy(
                    "controlled local write and script support is disabled".to_owned(),
                ));
            }
            McpTransportKind::StreamableHttp if !system.remote_http => {
                return Err(McpError::Policy(
                    "Streamable HTTP support is disabled".to_owned(),
                ));
            }
            McpTransportKind::StreamableHttp
                if config.auth != McpAuthKind::None && !system.secure_auth =>
            {
                return Err(McpError::Policy(
                    "MCP Keychain authentication support is disabled".to_owned(),
                ));
            }
            McpTransportKind::Stdio
                if config.auth == McpAuthKind::Bearer && !system.secure_auth =>
            {
                return Err(McpError::Policy(
                    "MCP Keychain authentication support is disabled".to_owned(),
                ));
            }
            _ => {}
        }
        let handler = CatalogClientHandler {
            server_id: config.id.clone(),
            refresh_tx: self.refresh_tx.clone(),
        };
        let service = match config.transport {
            McpTransportKind::Stdio => {
                let executable = Path::new(&config.executable);
                if !tokio::fs::try_exists(executable)
                    .await
                    .map_err(|error| McpError::Start(error.to_string()))?
                {
                    return Err(McpError::Start(format!(
                        "executable '{}' does not exist",
                        config.executable
                    )));
                }
                let mut command = isolated_command(config).await?;
                if config.auth == McpAuthKind::Bearer {
                    let variable =
                        config
                            .secret_environment_variable
                            .as_deref()
                            .ok_or_else(|| {
                                McpError::Configuration(
                                    "credential environment variable is missing".to_owned(),
                                )
                            })?;
                    let secret = self
                        .credentials
                        .get(&credential_reference(&config.id))
                        .await
                        .map_err(|error| McpError::Authorization(error.to_string()))?
                        .ok_or_else(|| {
                            McpError::Authorization(
                                "server credential is not configured".to_owned(),
                            )
                        })?;
                    command.env(variable, access_token(&secret)?);
                }
                let stderr_mode = if cfg!(test) {
                    Stdio::inherit()
                } else {
                    Stdio::piped()
                };
                let (transport, stderr) = TokioChildProcess::builder(command)
                    .stderr(stderr_mode)
                    .spawn()
                    .map_err(|error| McpError::Start(error.to_string()))?;
                if let Some(stderr) = stderr {
                    tokio::spawn(async move {
                        let mut stderr = stderr;
                        let mut buffer = [0_u8; 8192];
                        while matches!(stderr.read(&mut buffer).await, Ok(size) if size > 0) {}
                    });
                }
                tokio::time::timeout(CONNECT_TIMEOUT, handler.serve(transport))
                    .await
                    .map_err(|_| McpError::Timeout)?
                    .map_err(|error| McpError::Protocol(error.to_string()))?
            }
            McpTransportKind::StreamableHttp => {
                let url = config.url.as_deref().ok_or_else(|| {
                    McpError::Configuration("HTTP server URL is missing".to_owned())
                })?;
                let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url)
                    .max_sse_event_size(MAX_SSE_EVENT_BYTES)
                    .reinit_on_expired_session(false);
                if config.auth == McpAuthKind::Bearer {
                    let token = self
                        .credentials
                        .get(&credential_reference(&config.id))
                        .await
                        .map_err(|error| McpError::Authorization(error.to_string()))?
                        .ok_or_else(|| {
                            McpError::Authorization(
                                "server credential is not configured".to_owned(),
                            )
                        })?;
                    transport_config = transport_config.auth_header(access_token(&token)?);
                }
                let client = reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .connect_timeout(Duration::from_secs(10))
                    .timeout(Duration::from_secs(30))
                    .build()
                    .map_err(|error| McpError::Start(error.to_string()))?;
                if config.auth == McpAuthKind::Oauth {
                    let mut auth_manager = AuthorizationManager::new(url)
                        .await
                        .map_err(|error| McpError::Authorization(error.to_string()))?;
                    auth_manager.set_credential_store(self.oauth_store(&config.id));
                    let transport = StreamableHttpClientTransport::with_client(
                        AuthClient::new(client, auth_manager),
                        transport_config,
                    );
                    tokio::time::timeout(CONNECT_TIMEOUT, handler.clone().serve(transport))
                        .await
                        .map_err(|_| McpError::Timeout)?
                        .map_err(|error| {
                            if error.is_authorization_required() {
                                McpError::Authorization(
                                    error
                                        .auth_challenge()
                                        .unwrap_or("authorization required")
                                        .to_owned(),
                                )
                            } else {
                                McpError::Protocol(error.to_string())
                            }
                        })?
                } else {
                    let transport =
                        StreamableHttpClientTransport::with_client(client, transport_config);
                    tokio::time::timeout(CONNECT_TIMEOUT, handler.serve(transport))
                        .await
                        .map_err(|_| McpError::Timeout)?
                        .map_err(|error| {
                            if error.is_authorization_required() {
                                McpError::Authorization(
                                    error
                                        .auth_challenge()
                                        .unwrap_or("authorization required")
                                        .to_owned(),
                                )
                            } else {
                                McpError::Protocol(error.to_string())
                            }
                        })?
                }
            }
        };
        let tools = discover_tools(config, &service).await?;
        Ok(McpConnection {
            service: Mutex::new(Some(service)),
            tools: RwLock::new(tools),
            revision: AtomicU64::new(1),
        })
    }

    pub async fn refresh_server_tools(&self, id: &str) -> Result<(), McpError> {
        let config = self
            .configs
            .read()
            .await
            .iter()
            .find(|config| config.id == id)
            .cloned()
            .ok_or_else(|| McpError::ServerNotFound(id.to_owned()))?;
        let connection = self
            .connections
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| McpError::NotConnected(id.to_owned()))?;
        let service = connection.service.lock().await;
        let service = service
            .as_ref()
            .ok_or_else(|| McpError::NotConnected(id.to_owned()))?;
        let tools = discover_tools(&config, service).await?;
        *connection.tools.write().await = tools;
        connection.revision.fetch_add(1, Ordering::Relaxed);
        self.catalog_revision.fetch_add(1, Ordering::Relaxed);
        self.set_status(id, McpConnectionState::Ready, None).await;
        Ok(())
    }

    pub async fn disconnect(&self, id: &str) -> Result<(), McpError> {
        let connection = self.connections.write().await.remove(id);
        if let Some(connection) = connection {
            let mut service = connection.service.lock().await;
            if let Some(mut service) = service.take() {
                service
                    .close()
                    .await
                    .map_err(|error| McpError::Protocol(error.to_string()))?;
            }
        }
        if self
            .configs
            .read()
            .await
            .iter()
            .any(|config| config.id == id)
        {
            self.set_status(id, McpConnectionState::Disconnected, None)
                .await;
        }
        Ok(())
    }

    pub async fn disconnect_all(&self) {
        let ids = self
            .connections
            .read()
            .await
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for id in ids {
            let _ = self.disconnect(&id).await;
        }
    }

    pub async fn reconnect_enabled(self: &Arc<Self>) {
        let ids = self
            .configs
            .read()
            .await
            .iter()
            .filter(|config| config.enabled)
            .map(|config| config.id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            let _ = self.connect(&id).await;
        }
    }

    async fn recover_connection(self: &Arc<Self>, id: &str) {
        if !self.system.read().await.dynamic_updates {
            return;
        }
        const BACKOFF: [Duration; 3] = [
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
        ];
        let mut last_error = None;
        for delay in BACKOFF {
            self.set_status(id, McpConnectionState::Reconnecting, last_error.clone())
                .await;
            tokio::time::sleep(delay).await;
            match self.connect(id).await {
                Ok(()) => return,
                Err(error) => last_error = Some(error.to_string()),
            }
        }
        self.set_status(
            id,
            McpConnectionState::Failed,
            Some(last_error.unwrap_or_else(|| "automatic reconnect failed".to_owned())),
        )
        .await;
    }

    async fn set_status(&self, id: &str, state: McpConnectionState, error: Option<String>) {
        self.statuses
            .write()
            .await
            .insert(id.to_owned(), RuntimeStatus { state, error });
    }

    pub async fn tool_registry(self: &Arc<Self>) -> ToolRegistry {
        let configs = self.configs.read().await.clone();
        let connections = self.connections.read().await;
        let mut tools: Vec<Arc<dyn AgentTool>> = Vec::new();
        for config in configs.iter().filter(|config| config.enabled) {
            let Some(connection) = connections.get(&config.id) else {
                continue;
            };
            let descriptors = connection.tools.read().await;
            for descriptor in descriptors.iter() {
                let Some(policy) =
                    policy_for(config, &descriptor.remote_name).filter(|policy| policy.enabled)
                else {
                    continue;
                };
                tools.push(Arc::new(McpToolAdapter::new(
                    descriptor.clone(),
                    self.clone(),
                    policy.clone(),
                )));
            }
        }
        ToolRegistry::built_in().extend(tools)
    }

    pub async fn call_tool(
        self: &Arc<Self>,
        server_id: &str,
        tool_name: &str,
        arguments: Value,
        cancellation: CancellationToken,
    ) -> Result<Value, McpError> {
        let config = self
            .configs
            .read()
            .await
            .iter()
            .find(|config| config.id == server_id)
            .cloned()
            .ok_or_else(|| McpError::ServerNotFound(server_id.to_owned()))?;
        enforce_domain_policy(&arguments, &config.allowed_domains)?;
        let connection = self
            .connections
            .read()
            .await
            .get(server_id)
            .cloned()
            .ok_or_else(|| McpError::NotConnected(server_id.to_owned()))?;
        let arguments = arguments
            .as_object()
            .cloned()
            .ok_or_else(|| McpError::Tool("tool arguments must be an object".to_owned()))?;
        let service_guard = connection.service.lock().await;
        let service = service_guard
            .as_ref()
            .ok_or_else(|| McpError::NotConnected(server_id.to_owned()))?;
        let request = service.call_tool_once(
            CallToolRequestParams::new(tool_name.to_owned()).with_arguments(arguments),
        );
        let response = tokio::select! {
            _ = cancellation.cancelled() => Err(McpError::Cancelled),
            result = tokio::time::timeout(CALL_TIMEOUT, request) => {
                result.map_err(|_| McpError::Timeout)
                    .and_then(|result| result.map_err(|error| McpError::Protocol(error.to_string())))
            }
        };
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                drop(service_guard);
                if matches!(error, McpError::Protocol(_)) {
                    self.set_status(
                        server_id,
                        McpConnectionState::Degraded,
                        Some(error.to_string()),
                    )
                    .await;
                    let manager = self.clone();
                    let server_id = server_id.to_owned();
                    tokio::spawn(async move { manager.recover_connection(&server_id).await });
                }
                return Err(error);
            }
        };
        let CallToolResponse::Complete(result) = response else {
            return Err(McpError::Tool(
                "tool requested unsupported additional input".to_owned(),
            ));
        };
        let value =
            serde_json::to_value(result).map_err(|error| McpError::Protocol(error.to_string()))?;
        let size = serde_json::to_vec(&value)
            .map_err(|error| McpError::Protocol(error.to_string()))?
            .len();
        if size > MAX_RESULT_BYTES {
            return Err(McpError::Tool(format!(
                "tool result exceeds the {MAX_RESULT_BYTES} byte limit"
            )));
        }
        if value.get("isError").and_then(Value::as_bool) == Some(true) {
            return Err(McpError::Tool(value.to_string()));
        }
        enforce_domain_policy(&value, &config.allowed_domains)?;
        Ok(value)
    }

    pub async fn preview_file_change(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Option<String>, McpError> {
        let config = self
            .configs
            .read()
            .await
            .iter()
            .find(|config| config.id == server_id)
            .cloned()
            .ok_or_else(|| McpError::ServerNotFound(server_id.to_owned()))?;
        if config.preset != Some(McpPresetKind::WorkspaceFilesystem) {
            return Ok(None);
        }
        let Some(path) = arguments.get("path").and_then(Value::as_str) else {
            return Ok(None);
        };
        let proposed = if let Some(content) = arguments.get("content").and_then(Value::as_str) {
            content.to_owned()
        } else if tool_name == "edit_file" {
            let mut proposed = std::fs::read_to_string(path).unwrap_or_default();
            let edits = arguments
                .get("edits")
                .and_then(Value::as_array)
                .ok_or_else(|| McpError::Tool("edit_file arguments omitted edits".to_owned()))?;
            for edit in edits {
                let old = edit
                    .get("oldText")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let new = edit
                    .get("newText")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                proposed = proposed.replacen(old, new, 1);
            }
            proposed
        } else {
            return Ok(None);
        };
        file_diff_preview(path, &proposed, &config.allowed_directories).map(Some)
    }
}

fn find_executable(name: &str) -> Result<PathBuf, McpError> {
    let candidates = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .chain([
            PathBuf::from(format!("/usr/local/bin/{name}")),
            PathBuf::from(format!("/opt/homebrew/bin/{name}")),
        ]);
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| McpError::Start(format!("{name} was not found; install Node.js first")))
}

fn enforce_domain_policy(value: &Value, allowed: &[String]) -> Result<(), McpError> {
    if allowed.is_empty() {
        return Ok(());
    }
    match value {
        Value::String(text) => {
            for token in text.split_whitespace() {
                let candidate = token.trim_matches(|character: char| {
                    matches!(character, '"' | '\'' | '(' | ')' | '[' | ']' | ',' | ';')
                });
                if let Ok(url) = url::Url::parse(candidate)
                    && matches!(url.scheme(), "http" | "https")
                {
                    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
                    if !allowed.iter().any(|domain| {
                        let domain = domain.to_ascii_lowercase();
                        host == domain || host.ends_with(&format!(".{domain}"))
                    }) {
                        return Err(McpError::Policy(format!(
                            "URL domain '{host}' is outside this server's allowlist"
                        )));
                    }
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                enforce_domain_policy(value, allowed)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                enforce_domain_policy(value, allowed)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn file_diff_preview(path: &str, proposed: &str, allowed: &[String]) -> Result<String, McpError> {
    let path = Path::new(path);
    let resolved = if path.exists() {
        std::fs::canonicalize(path)
    } else {
        let parent = path.parent().ok_or_else(|| {
            McpError::Policy("file change path has no parent directory".to_owned())
        })?;
        std::fs::canonicalize(parent)
            .map(|parent| parent.join(path.file_name().unwrap_or_default()))
    }
    .map_err(|error| McpError::Policy(format!("file change path cannot be resolved: {error}")))?;
    let permitted = allowed.iter().any(|root| {
        std::fs::canonicalize(root).is_ok_and(|root| resolved == root || resolved.starts_with(root))
    });
    if !permitted {
        return Err(McpError::Policy(
            "file change is outside the approved write directories".to_owned(),
        ));
    }
    let original = std::fs::read_to_string(&resolved).unwrap_or_default();
    Ok(similar::TextDiff::from_lines(original.as_str(), proposed)
        .unified_diff()
        .context_radius(3)
        .header(
            &format!("a/{}", path.display()),
            &format!("b/{}", path.display()),
        )
        .to_string())
}

async fn discover_tools(
    config: &McpServerConfig,
    service: &ClientService,
) -> Result<Vec<McpToolDescriptor>, McpError> {
    let remote = tokio::time::timeout(DISCOVERY_TIMEOUT, service.list_all_tools())
        .await
        .map_err(|_| McpError::Timeout)?
        .map_err(|error| McpError::Protocol(error.to_string()))?;
    if remote.len() > MAX_TOOLS_PER_SERVER {
        return Err(McpError::Policy(format!(
            "server exposed {} tools; limit is {MAX_TOOLS_PER_SERVER}",
            remote.len()
        )));
    }
    let mut schema_bytes = 0_usize;
    let tools = remote
        .into_iter()
        .map(|tool| {
            let descriptor = descriptor(config, tool)?;
            schema_bytes = schema_bytes
                .saturating_add(
                    serde_json::to_vec(&descriptor.input_schema)
                        .unwrap_or_default()
                        .len(),
                )
                .saturating_add(
                    descriptor
                        .output_schema
                        .as_ref()
                        .map(|schema| serde_json::to_vec(schema).unwrap_or_default().len())
                        .unwrap_or_default(),
                );
            Ok(descriptor)
        })
        .collect::<Result<Vec<_>, McpError>>()?;
    if schema_bytes > MAX_CATALOG_SCHEMA_BYTES {
        return Err(McpError::Policy(format!(
            "server tool schemas exceed the {MAX_CATALOG_SCHEMA_BYTES} byte catalog limit"
        )));
    }
    Ok(tools)
}

fn policy_for<'a>(config: &'a McpServerConfig, name: &str) -> Option<&'a McpToolPolicy> {
    config
        .tool_policies
        .iter()
        .find(|policy| policy.name == name)
}

fn credential_reference(server_id: &str) -> String {
    format!("mcp:{server_id}:authorization")
}

fn access_token(secret: &str) -> Result<String, McpError> {
    if let Ok(value) = serde_json::from_str::<Value>(secret)
        && let Some(token) = value
            .get("token_response")
            .and_then(|value| value.get("access_token"))
            .and_then(Value::as_str)
    {
        return Ok(token.to_owned());
    }
    let token = secret.trim();
    if token.is_empty() || token.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(McpError::Authorization(
            "stored token is invalid".to_owned(),
        ));
    }
    Ok(token.to_owned())
}

fn validate_oauth_redirect(redirect_uri: &str) -> Result<(), McpError> {
    let url = reqwest::Url::parse(redirect_uri)
        .map_err(|error| McpError::Authorization(format!("invalid OAuth redirect URI: {error}")))?;
    let loopback = matches!(
        url.host_str(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    );
    if url.scheme() != "http" || !loopback || url.query().is_some() || url.fragment().is_some() {
        return Err(McpError::Authorization(
            "OAuth redirect must be a plain loopback HTTP URL without query or fragment".to_owned(),
        ));
    }
    Ok(())
}

fn validate_oauth_callback(callback_url: &str, redirect_uri: &str) -> Result<(), McpError> {
    let callback = reqwest::Url::parse(callback_url)
        .map_err(|error| McpError::Authorization(format!("invalid OAuth callback URL: {error}")))?;
    let expected = reqwest::Url::parse(redirect_uri)
        .map_err(|error| McpError::Authorization(format!("invalid OAuth redirect URI: {error}")))?;
    if callback.scheme() != expected.scheme()
        || callback.host_str() != expected.host_str()
        || callback.port_or_known_default() != expected.port_or_known_default()
        || callback.path() != expected.path()
    {
        return Err(McpError::Authorization(
            "OAuth callback does not match the redirect URI".to_owned(),
        ));
    }
    Ok(())
}

fn descriptor(
    config: &McpServerConfig,
    tool: rmcp::model::Tool,
) -> Result<McpToolDescriptor, McpError> {
    let input_schema = Value::Object((*tool.input_schema).clone());
    reject_external_refs(&input_schema)?;
    jsonschema::validator_for(&input_schema).map_err(|error| {
        McpError::Protocol(format!(
            "tool '{}' has invalid input schema: {error}",
            tool.name
        ))
    })?;
    let output_schema = tool
        .output_schema
        .map(|schema| Value::Object((*schema).clone()));
    if let Some(schema) = &output_schema {
        reject_external_refs(schema)?;
        jsonschema::validator_for(schema).map_err(|error| {
            McpError::Protocol(format!(
                "tool '{}' has invalid output schema: {error}",
                tool.name
            ))
        })?;
    }
    let remote_name = tool.name.into_owned();
    let schema_hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&input_schema).unwrap_or_default())
    );
    let alias = tool_alias(&config.id, &remote_name, &schema_hash);
    Ok(McpToolDescriptor {
        server_id: config.id.clone(),
        remote_name,
        alias,
        title: tool.title,
        description: tool
            .description
            .map(|value| value.into_owned())
            .unwrap_or_else(|| "No description provided.".to_owned()),
        input_schema,
        output_schema,
        schema_hash,
        read_only_hint: tool
            .annotations
            .and_then(|annotations| annotations.read_only_hint),
    })
}

fn tool_alias(server_id: &str, tool_name: &str, schema_hash: &str) -> String {
    let mut prefix = format!("mcp_{server_id}_{tool_name}")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    prefix.truncate(52);
    let identity_hash = format!(
        "{:x}",
        Sha256::digest(format!("{server_id}\0{tool_name}\0{schema_hash}"))
    );
    format!("{prefix}_{}", &identity_hash[..8])
}

fn reject_external_refs(value: &Value) -> Result<(), McpError> {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str)
                && !reference.starts_with('#')
            {
                return Err(McpError::Protocol(format!(
                    "external schema reference '{reference}' is not allowed"
                )));
            }
            for child in object.values() {
                reject_external_refs(child)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_external_refs(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use axum::response::{IntoResponse, Response};
    use axum::{Json, Router, http::StatusCode, routing::post};
    use tokio::sync::RwLock;
    use tokio_util::sync::CancellationToken;

    use crate::credentials::{CredentialError, CredentialStore};
    use crate::domain::mcp::{
        McpAuthKind, McpPresetKind, McpServerConfig, McpSystemSettings, McpToolPolicy,
        McpTransportKind,
    };
    use crate::tools::ToolRisk;

    use super::{
        McpClientManager, McpConfigStore, enforce_domain_policy, file_diff_preview,
        reject_external_refs, tool_alias, validate_oauth_callback, validate_oauth_redirect,
    };

    #[derive(Default)]
    struct MemoryCredentials(RwLock<HashMap<String, String>>);

    #[async_trait]
    impl CredentialStore for MemoryCredentials {
        async fn contains(&self, reference: &str) -> Result<bool, CredentialError> {
            Ok(self.0.read().await.contains_key(reference))
        }

        async fn get(&self, reference: &str) -> Result<Option<String>, CredentialError> {
            Ok(self.0.read().await.get(reference).cloned())
        }

        async fn set(&self, reference: &str, secret: String) -> Result<(), CredentialError> {
            self.0.write().await.insert(reference.to_owned(), secret);
            Ok(())
        }

        async fn delete(&self, reference: &str) -> Result<(), CredentialError> {
            self.0.write().await.remove(reference);
            Ok(())
        }
    }

    #[test]
    fn provider_alias_is_stable_and_bounded() {
        let alias = tool_alias(
            "files",
            "read/a very long remote tool name that keeps going",
            &"a".repeat(64),
        );
        assert!(alias.len() <= 61);
        assert!(
            alias
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        );
    }

    #[test]
    fn external_schema_references_are_rejected() {
        assert!(
            reject_external_refs(&serde_json::json!({"$ref": "https://example.com/schema"}))
                .is_err()
        );
        assert!(reject_external_refs(&serde_json::json!({"$ref": "#/$defs/value"})).is_ok());
    }

    #[test]
    fn oauth_redirects_are_restricted_to_exact_loopback_callbacks() {
        let redirect = "http://127.0.0.1:8765/callback";
        assert!(validate_oauth_redirect(redirect).is_ok());
        assert!(validate_oauth_redirect("https://example.com/callback").is_err());
        assert!(
            validate_oauth_callback("http://127.0.0.1:8765/callback?code=ok&state=ok", redirect)
                .is_ok()
        );
        assert!(
            validate_oauth_callback("http://127.0.0.1:8766/callback?code=ok&state=ok", redirect)
                .is_err()
        );
    }

    #[test]
    fn domain_policy_accepts_exact_and_subdomains_only() {
        let allowed = vec!["example.com".to_owned()];
        assert!(
            enforce_domain_policy(
                &serde_json::json!({"url": "https://docs.example.com/page"}),
                &allowed,
            )
            .is_ok()
        );
        assert!(
            enforce_domain_policy(
                &serde_json::json!({"url": "https://example.com.attacker.test/page"}),
                &allowed,
            )
            .is_err()
        );
    }

    #[test]
    fn file_preview_is_scoped_and_contains_unified_diff() {
        let workspace = tempfile::tempdir().unwrap();
        let file = workspace.path().join("note.txt");
        std::fs::write(&file, "before\n").unwrap();
        let roots = vec![workspace.path().display().to_string()];
        let preview = file_diff_preview(file.to_str().unwrap(), "after\n", &roots).unwrap();
        assert!(preview.contains("-before"));
        assert!(preview.contains("+after"));

        let outside = tempfile::NamedTempFile::new().unwrap();
        assert!(file_diff_preview(outside.path().to_str().unwrap(), "blocked", &roots).is_err());
    }

    #[cfg(target_os = "macos")]
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "downloads and exercises the pinned official Filesystem MCP server"]
    async fn real_filesystem_preset_reads_only_the_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let note = workspace.join("acceptance.txt");
        std::fs::write(&note, "carrot-filesystem-acceptance").unwrap();
        let manager = McpClientManager::initialize(
            McpConfigStore::new(temp.path().join("config/mcp-servers.toml")),
            Arc::new(MemoryCredentials::default()),
        )
        .await
        .unwrap();
        let installed = manager
            .install_preset(
                McpPresetKind::WorkspaceFilesystem,
                Some(workspace.display().to_string()),
            )
            .await
            .unwrap();
        assert!(installed.servers[0].tools.iter().any(|tool| {
            tool.name == "read_text_file"
                && tool.enabled
                && tool.risk == crate::tools::ToolRisk::ReadOnly
        }));
        let note = std::fs::canonicalize(note).unwrap();
        let result = manager
            .call_tool(
                "workspace-files",
                "read_text_file",
                serde_json::json!({"path": note}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(result.to_string().contains("carrot-filesystem-acceptance"));
        let outside = tempfile::NamedTempFile::new().unwrap();
        let rejected = manager
            .call_tool(
                "workspace-files",
                "read_text_file",
                serde_json::json!({"path": outside.path()}),
                CancellationToken::new(),
            )
            .await;
        assert!(rejected.is_err());
        manager.disconnect_all().await;
    }

    #[cfg(target_os = "macos")]
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires CARROT_BRAVE_API_KEY and the live Brave Search service"]
    async fn real_brave_search_preset_returns_live_results() {
        let key = std::env::var("CARROT_BRAVE_API_KEY")
            .expect("set CARROT_BRAVE_API_KEY to run this acceptance test");
        let temp = tempfile::tempdir().unwrap();
        let manager = McpClientManager::initialize(
            McpConfigStore::new(temp.path().join("config/mcp-servers.toml")),
            Arc::new(MemoryCredentials::default()),
        )
        .await
        .unwrap();
        manager
            .install_preset(McpPresetKind::BraveSearch, None)
            .await
            .unwrap();
        manager.set_auth_secret("brave-search", key).await.unwrap();
        let result = manager
            .call_tool(
                "brave-search",
                "brave_web_search",
                serde_json::json!({"query": "Model Context Protocol", "count": 3}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(!result.to_string().is_empty());
        manager.disconnect_all().await;
    }

    #[tokio::test]
    async fn system_switches_persist_and_gate_remote_connections() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("mcp-servers.toml");
        let store = McpConfigStore::new(path.clone());
        store
            .save(&[McpServerConfig {
                id: "remote".to_owned(),
                label: "Remote".to_owned(),
                enabled: false,
                transport: McpTransportKind::StreamableHttp,
                executable: String::new(),
                arguments: Vec::new(),
                working_directory: None,
                url: Some("http://127.0.0.1:9/mcp".to_owned()),
                auth: McpAuthKind::None,
                oauth_client_id: None,
                oauth_scopes: Vec::new(),
                preset: None,
                secret_environment_variable: None,
                read_directories: Vec::new(),
                allowed_directories: Vec::new(),
                allowed_domains: Vec::new(),
                allow_network: false,
                tool_policies: Vec::new(),
            }])
            .await
            .unwrap();
        let manager = McpClientManager::initialize(store, Arc::new(MemoryCredentials::default()))
            .await
            .unwrap();
        let settings = McpSystemSettings {
            remote_http: false,
            ..McpSystemSettings::default()
        };
        let snapshot = manager.update_system_settings(settings).await.unwrap();
        assert!(!snapshot.system.remote_http);
        assert!(
            manager
                .connect("remote")
                .await
                .unwrap_err()
                .to_string()
                .contains("disabled")
        );
        let stored = McpConfigStore::new(path).load_all().await.unwrap();
        assert!(!stored.system.remote_http);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn discovers_and_calls_a_stdio_server() {
        let Some(node) = find_executable("node") else {
            return;
        };
        let temp = tempfile::tempdir().unwrap();
        let store = McpConfigStore::new(temp.path().join("mcp-servers.toml"));
        store
            .save(&[McpServerConfig {
                id: "fixture".to_owned(),
                label: "Fixture".to_owned(),
                enabled: true,
                transport: McpTransportKind::Stdio,
                executable: node.display().to_string(),
                arguments: vec![
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/fake-mcp-server.mjs")
                        .display()
                        .to_string(),
                ],
                working_directory: None,
                url: None,
                auth: McpAuthKind::None,
                oauth_client_id: None,
                oauth_scopes: Vec::new(),
                preset: None,
                secret_environment_variable: None,
                read_directories: Vec::new(),
                allowed_directories: Vec::new(),
                allowed_domains: Vec::new(),
                allow_network: false,
                tool_policies: vec![McpToolPolicy {
                    name: "read_note".to_owned(),
                    enabled: true,
                    risk: ToolRisk::ReadOnly,
                    idempotent: true,
                    reconcile: true,
                }],
            }])
            .await
            .unwrap();
        let manager = McpClientManager::initialize(store, Arc::new(MemoryCredentials::default()))
            .await
            .unwrap();
        let snapshot = manager.snapshot().await;
        assert_eq!(snapshot.servers[0].tools[0].name, "read_note");
        assert!(snapshot.servers[0].tools[0].enabled);

        let result = manager
            .call_tool(
                "fixture",
                "read_note",
                serde_json::json!({"name": "Carrot"}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result["structuredContent"]["note"], "hello Carrot");
        manager.disconnect_all().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn discovers_and_calls_a_streamable_http_server() {
        async fn mcp(Json(request): Json<serde_json::Value>) -> Response {
            let Some(id) = request.get("id").cloned() else {
                return StatusCode::ACCEPTED.into_response();
            };
            let result = match request["method"].as_str() {
                Some("initialize") => serde_json::json!({
                    "protocolVersion": request["params"]["protocolVersion"],
                    "capabilities": {"tools": {"listChanged": true}},
                    "serverInfo": {"name": "carrot-http-fixture", "version": "1.0.0"}
                }),
                Some("tools/list") => serde_json::json!({"tools": [{
                    "name": "read_http_note",
                    "description": "Return a deterministic HTTP note.",
                    "inputSchema": {"type": "object", "additionalProperties": false},
                    "annotations": {"readOnlyHint": true}
                }]}),
                Some("tools/call") => serde_json::json!({
                    "content": [{"type": "text", "text": "hello http"}],
                    "structuredContent": {"note": "hello http"},
                    "isError": false
                }),
                _ => return StatusCode::NOT_FOUND.into_response(),
            };
            Json(serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result})).into_response()
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/mcp", post(mcp)))
                .await
                .unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let store = McpConfigStore::new(temp.path().join("mcp-servers.toml"));
        store
            .save(&[McpServerConfig {
                id: "http-fixture".to_owned(),
                label: "HTTP Fixture".to_owned(),
                enabled: true,
                transport: McpTransportKind::StreamableHttp,
                executable: String::new(),
                arguments: Vec::new(),
                working_directory: None,
                url: Some(format!("http://{address}/mcp")),
                auth: McpAuthKind::None,
                oauth_client_id: None,
                oauth_scopes: Vec::new(),
                preset: None,
                secret_environment_variable: None,
                read_directories: Vec::new(),
                allowed_directories: Vec::new(),
                allowed_domains: Vec::new(),
                allow_network: false,
                tool_policies: vec![McpToolPolicy {
                    name: "read_http_note".to_owned(),
                    enabled: true,
                    risk: ToolRisk::ReadOnly,
                    idempotent: true,
                    reconcile: true,
                }],
            }])
            .await
            .unwrap();
        let manager = McpClientManager::initialize(store, Arc::new(MemoryCredentials::default()))
            .await
            .unwrap();
        let snapshot = manager.snapshot().await;
        assert_eq!(
            snapshot.servers[0].state,
            crate::domain::mcp::McpConnectionState::Ready
        );
        assert_eq!(snapshot.servers[0].tools[0].name, "read_http_note");
        let result = manager
            .call_tool(
                "http-fixture",
                "read_http_note",
                serde_json::json!({}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result["structuredContent"]["note"], "hello http");
        manager.disconnect_all().await;
        server.abort();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn list_changed_refreshes_catalog_revision() {
        let Some(node) = find_executable("node") else {
            return;
        };
        let temp = tempfile::tempdir().unwrap();
        let store = McpConfigStore::new(temp.path().join("mcp-servers.toml"));
        store
            .save(&[McpServerConfig {
                id: "dynamic".to_owned(),
                label: "Dynamic".to_owned(),
                enabled: true,
                transport: McpTransportKind::Stdio,
                executable: node.display().to_string(),
                arguments: vec![
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/fake-mcp-dynamic-server.mjs")
                        .display()
                        .to_string(),
                ],
                working_directory: None,
                url: None,
                auth: McpAuthKind::None,
                oauth_client_id: None,
                oauth_scopes: Vec::new(),
                preset: None,
                secret_environment_variable: None,
                read_directories: Vec::new(),
                allowed_directories: Vec::new(),
                allowed_domains: Vec::new(),
                allow_network: false,
                tool_policies: Vec::new(),
            }])
            .await
            .unwrap();
        let manager = McpClientManager::initialize(store, Arc::new(MemoryCredentials::default()))
            .await
            .unwrap();
        for _ in 0..40 {
            let snapshot = manager.snapshot().await;
            if snapshot.servers[0].tools.len() == 2 {
                assert!(snapshot.servers[0].catalog_revision.parse::<u64>().unwrap() > 1);
                manager.disconnect_all().await;
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("dynamic tool catalog did not refresh");
    }

    fn find_executable(name: &str) -> Option<PathBuf> {
        std::env::var_os("PATH")?
            .to_string_lossy()
            .split(':')
            .map(PathBuf::from)
            .map(|directory| directory.join(name))
            .find(|path| path.is_file() && path.is_absolute())
    }
}
