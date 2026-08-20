use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::mcp::{
    McpAuthKind, McpPresetKind, McpServerConfig, McpSystemSettings, McpToolPolicy, McpTransportKind,
};
use crate::tools::ToolRisk;

const CURRENT_VERSION: u32 = 2;

#[derive(Debug, thiserror::Error)]
pub enum McpConfigError {
    #[error("MCP configuration could not be read: {0}")]
    Read(String),
    #[error("MCP configuration could not be written: {0}")]
    Write(String),
    #[error("MCP configuration is invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct McpConfigFile {
    version: u32,
    #[serde(default)]
    system: McpSystemSettings,
    #[serde(default)]
    servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone)]
pub struct McpStoredConfig {
    pub system: McpSystemSettings,
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyConfigFile {
    #[serde(rename = "version")]
    _version: u32,
    #[serde(default)]
    servers: Vec<LegacyServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyServerConfig {
    id: String,
    label: String,
    enabled: bool,
    executable: String,
    #[serde(default)]
    arguments: Vec<String>,
    working_directory: Option<String>,
    #[serde(default)]
    read_only_tools: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct McpConfigStore {
    path: PathBuf,
}

impl McpConfigStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn ensure_and_load(&self) -> Result<Vec<McpServerConfig>, McpConfigError> {
        Ok(self.ensure_and_load_all().await?.servers)
    }

    pub async fn ensure_and_load_all(&self) -> Result<McpStoredConfig, McpConfigError> {
        if tokio::fs::try_exists(&self.path)
            .await
            .map_err(|error| McpConfigError::Read(error.to_string()))?
        {
            return self.load_all().await;
        }
        self.save_all(&McpSystemSettings::default(), &[]).await
    }

    pub async fn load(&self) -> Result<Vec<McpServerConfig>, McpConfigError> {
        Ok(self.load_all().await?.servers)
    }

    pub async fn load_all(&self) -> Result<McpStoredConfig, McpConfigError> {
        let source = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|error| McpConfigError::Read(error.to_string()))?;
        Self::parse_all(&source)
    }

    pub async fn save(
        &self,
        servers: &[McpServerConfig],
    ) -> Result<Vec<McpServerConfig>, McpConfigError> {
        Ok(self
            .save_all(&McpSystemSettings::default(), servers)
            .await?
            .servers)
    }

    pub async fn save_all(
        &self,
        system: &McpSystemSettings,
        servers: &[McpServerConfig],
    ) -> Result<McpStoredConfig, McpConfigError> {
        validate_servers(servers)?;
        let file = McpConfigFile {
            version: CURRENT_VERSION,
            system: system.clone(),
            servers: servers.to_vec(),
        };
        let source = toml::to_string_pretty(&file)
            .map_err(|error| McpConfigError::Write(error.to_string()))?;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| McpConfigError::Write(error.to_string()))?;
        }
        let temporary = self.path.with_extension("toml.tmp");
        tokio::fs::write(&temporary, source)
            .await
            .map_err(|error| McpConfigError::Write(error.to_string()))?;
        tokio::fs::rename(&temporary, &self.path)
            .await
            .map_err(|error| McpConfigError::Write(error.to_string()))?;
        Ok(McpStoredConfig {
            system: system.clone(),
            servers: servers.to_vec(),
        })
    }

    #[cfg(test)]
    fn parse(source: &str) -> Result<Vec<McpServerConfig>, McpConfigError> {
        Ok(Self::parse_all(source)?.servers)
    }

    fn parse_all(source: &str) -> Result<McpStoredConfig, McpConfigError> {
        let version = toml::from_str::<toml::Value>(source)
            .map_err(|error| McpConfigError::Invalid(error.to_string()))?
            .get("version")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| McpConfigError::Invalid("version is required".to_owned()))?;
        let (system, servers) = match version {
            1 => {
                let file: LegacyConfigFile = toml::from_str(source)
                    .map_err(|error| McpConfigError::Invalid(error.to_string()))?;
                let servers = file
                    .servers
                    .into_iter()
                    .map(|server| McpServerConfig {
                        id: server.id,
                        label: server.label,
                        enabled: server.enabled,
                        transport: McpTransportKind::Stdio,
                        executable: server.executable,
                        arguments: server.arguments,
                        working_directory: server.working_directory,
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
                        tool_policies: server
                            .read_only_tools
                            .into_iter()
                            .map(|name| McpToolPolicy {
                                name,
                                enabled: true,
                                risk: ToolRisk::ReadOnly,
                                idempotent: true,
                                reconcile: true,
                            })
                            .collect(),
                    })
                    .collect();
                (McpSystemSettings::default(), servers)
            }
            value if value == i64::from(CURRENT_VERSION) => {
                let file = toml::from_str::<McpConfigFile>(source)
                    .map_err(|error| McpConfigError::Invalid(error.to_string()))?;
                (file.system, file.servers)
            }
            _ => {
                return Err(McpConfigError::Invalid(format!(
                    "unsupported version {version}; expected {CURRENT_VERSION}"
                )));
            }
        };
        validate_servers(&servers)?;
        Ok(McpStoredConfig { system, servers })
    }
}

fn validate_servers(servers: &[McpServerConfig]) -> Result<(), McpConfigError> {
    let mut ids = HashSet::new();
    for server in servers {
        if server.id.is_empty()
            || server.id.len() > 48
            || !server.id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte)
            })
        {
            return Err(McpConfigError::Invalid(format!(
                "server id '{}' must use 1-48 lowercase ASCII letters, digits, '-' or '_'",
                server.id
            )));
        }
        if !ids.insert(server.id.clone()) {
            return Err(McpConfigError::Invalid(format!(
                "server id '{}' is duplicated",
                server.id
            )));
        }
        if server.label.trim().is_empty() || server.label.chars().count() > 100 {
            return Err(McpConfigError::Invalid(format!(
                "server '{}' label must contain 1-100 characters",
                server.id
            )));
        }
        match server.transport {
            McpTransportKind::Stdio => {
                if !Path::new(&server.executable).is_absolute() {
                    return Err(McpConfigError::Invalid(format!(
                        "server '{}' executable must be an absolute path",
                        server.id
                    )));
                }
                if server.url.is_some() || server.auth == McpAuthKind::Oauth {
                    return Err(McpConfigError::Invalid(format!(
                        "stdio server '{}' cannot configure a URL or OAuth",
                        server.id
                    )));
                }
                if server.auth == McpAuthKind::Bearer
                    && server
                        .secret_environment_variable
                        .as_deref()
                        .is_none_or(str::is_empty)
                {
                    return Err(McpConfigError::Invalid(format!(
                        "stdio server '{}' requires a credential environment variable",
                        server.id
                    )));
                }
            }
            McpTransportKind::StreamableHttp => validate_http_server(server)?,
        }
        if server.arguments.len() > 64
            || server
                .arguments
                .iter()
                .any(|argument| argument.len() > 2_000)
        {
            return Err(McpConfigError::Invalid(format!(
                "server '{}' has too many or oversized arguments",
                server.id
            )));
        }
        if let Some(directory) = &server.working_directory
            && !Path::new(directory).is_absolute()
        {
            return Err(McpConfigError::Invalid(format!(
                "server '{}' working directory must be absolute",
                server.id
            )));
        }
        for (kind, configured) in [
            ("read", &server.read_directories),
            ("write", &server.allowed_directories),
        ] {
            let mut directories = HashSet::new();
            if configured.iter().any(|directory| {
                !Path::new(directory).is_absolute() || !directories.insert(directory)
            }) {
                return Err(McpConfigError::Invalid(format!(
                    "server '{}' has an invalid or duplicate {kind} directory",
                    server.id,
                )));
            }
        }
        let mut domains = HashSet::new();
        for domain in &server.allowed_domains {
            let normalized = domain.trim().to_ascii_lowercase();
            if normalized.is_empty()
                || normalized.contains(['/', ':', '@'])
                || !normalized
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
                || !domains.insert(normalized)
            {
                return Err(McpConfigError::Invalid(format!(
                    "server '{}' has an invalid or duplicate allowed domain",
                    server.id
                )));
            }
        }
        if let Some(variable) = &server.secret_environment_variable
            && (variable.is_empty()
                || !variable
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'))
        {
            return Err(McpConfigError::Invalid(format!(
                "server '{}' has an invalid credential environment variable",
                server.id
            )));
        }
        if server.preset == Some(McpPresetKind::WorkspaceFilesystem)
            && (server.transport != McpTransportKind::Stdio
                || server.allow_network
                || server.read_directories.is_empty()
                || server.arguments != server.read_directories)
        {
            return Err(McpConfigError::Invalid(format!(
                "trusted filesystem server '{}' must pass its read roots as the exact server arguments and keep network disabled",
                server.id
            )));
        }
        let mut tools = HashSet::new();
        if server
            .tool_policies
            .iter()
            .any(|policy| policy.name.is_empty() || !tools.insert(&policy.name))
        {
            return Err(McpConfigError::Invalid(format!(
                "server '{}' has an invalid or duplicate read-only tool",
                server.id
            )));
        }
    }
    Ok(())
}

fn validate_http_server(server: &McpServerConfig) -> Result<(), McpConfigError> {
    if !server.executable.is_empty()
        || !server.arguments.is_empty()
        || server.working_directory.is_some()
        || !server.read_directories.is_empty()
        || !server.allowed_directories.is_empty()
        || server.secret_environment_variable.is_some()
    {
        return Err(McpConfigError::Invalid(format!(
            "HTTP server '{}' cannot configure local process fields",
            server.id
        )));
    }
    let raw = server.url.as_deref().ok_or_else(|| {
        McpConfigError::Invalid(format!("HTTP server '{}' requires a URL", server.id))
    })?;
    let url = url::Url::parse(raw)
        .map_err(|error| McpConfigError::Invalid(format!("invalid MCP URL: {error}")))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(McpConfigError::Invalid(
            "MCP URL cannot contain credentials, query, or fragment".to_owned(),
        ));
    }
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(McpConfigError::Invalid(
            "remote MCP URL must use HTTPS; HTTP is limited to loopback".to_owned(),
        ));
    }
    if server.auth == McpAuthKind::Oauth
        && server
            .oauth_client_id
            .as_deref()
            .is_some_and(|client_id| client_id.trim().is_empty())
    {
        return Err(McpConfigError::Invalid(
            "OAuth client ID cannot be blank".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::McpConfigStore;

    #[test]
    fn rejects_relative_executables() {
        let error = McpConfigStore::parse(
            "version = 1\n[[servers]]\nid = 'demo'\nlabel = 'Demo'\nenabled = false\nexecutable = 'node'\n",
        )
        .unwrap_err();
        assert!(error.to_string().contains("absolute path"));
    }

    #[test]
    fn parses_empty_catalog() {
        assert!(
            McpConfigStore::parse("version = 2\nservers = []\n")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn system_capabilities_default_on_and_parse_persisted_switches() {
        let defaults = McpConfigStore::parse_all("version = 2\nservers = []\n").unwrap();
        assert!(defaults.system.controlled_local_tools);
        assert!(defaults.system.remote_http);
        assert!(defaults.system.secure_auth);
        assert!(defaults.system.dynamic_updates);

        let configured = McpConfigStore::parse_all(
            "version = 2\nservers = []\n[system]\ncontrolledLocalTools = false\nremoteHttp = true\nsecureAuth = false\ndynamicUpdates = false\n",
        )
        .unwrap();
        assert!(!configured.system.controlled_local_tools);
        assert!(configured.system.remote_http);
        assert!(!configured.system.secure_auth);
        assert!(!configured.system.dynamic_updates);
    }

    #[test]
    fn upgrades_v1_read_only_policies() {
        let servers = McpConfigStore::parse(
            "version = 1\n[[servers]]\nid='demo'\nlabel='Demo'\nenabled=false\nexecutable='/bin/echo'\nread_only_tools=['read']\n",
        )
        .unwrap();
        assert_eq!(
            servers[0].tool_policies[0].risk,
            crate::tools::ToolRisk::ReadOnly
        );
    }

    #[test]
    fn limits_cleartext_http_to_loopback() {
        let error = McpConfigStore::parse(
            "version = 2\n[[servers]]\nid='remote'\nlabel='Remote'\nenabled=false\ntransport='streamable_http'\nurl='http://example.com/mcp'\n",
        )
        .unwrap_err();
        assert!(error.to_string().contains("HTTPS"));
    }
}
