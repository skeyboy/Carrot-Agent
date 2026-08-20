use std::path::Path;

use crate::domain::mcp::{McpServerConfig, McpTransportKind};
use crate::tools::ToolRisk;

use super::manager::McpError;

pub async fn isolated_command(
    config: &McpServerConfig,
) -> Result<tokio::process::Command, McpError> {
    if config.transport != McpTransportKind::Stdio {
        return Err(McpError::Configuration(
            "process isolation is only valid for stdio servers".to_owned(),
        ));
    }
    let requires_isolation = !config.read_directories.is_empty()
        || config
            .tool_policies
            .iter()
            .any(|policy| policy.enabled && policy.risk != ToolRisk::ReadOnly);
    if !requires_isolation {
        return Ok(direct_command(config));
    }
    if config.allowed_directories.is_empty() && config.read_directories.is_empty() {
        return Err(McpError::Policy(
            "write and script tools require at least one allowed directory".to_owned(),
        ));
    }
    #[cfg(target_os = "macos")]
    {
        let sandbox = Path::new("/usr/bin/sandbox-exec");
        if !tokio::fs::try_exists(sandbox)
            .await
            .map_err(|error| McpError::Start(error.to_string()))?
        {
            return Err(McpError::Policy(
                "macOS sandbox-exec is unavailable; risky MCP tools remain disabled".to_owned(),
            ));
        }
        let profile = sandbox_profile(config)?;
        let mut command = tokio::process::Command::new(sandbox);
        command
            .arg("-p")
            .arg(profile)
            .arg(&config.executable)
            .args(&config.arguments);
        configure_environment(&mut command, config);
        Ok(command)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(McpError::Policy(
            "risky local MCP tools require a platform ProcessIsolation adapter".to_owned(),
        ))
    }
}

fn direct_command(config: &McpServerConfig) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(&config.executable);
    command.args(&config.arguments);
    configure_environment(&mut command, config);
    command
}

fn configure_environment(command: &mut tokio::process::Command, config: &McpServerConfig) {
    command.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    command.env("HOME", "/var/empty");
    command.env("TMPDIR", "/tmp");
    if let Some(directory) = &config.working_directory {
        command.current_dir(directory);
    }
}

#[cfg(target_os = "macos")]
fn sandbox_profile(config: &McpServerConfig) -> Result<String, McpError> {
    let mut rules = vec![
        "(version 1)".to_owned(),
        "(deny default)".to_owned(),
        "(allow process*)".to_owned(),
        "(allow signal (target same-sandbox))".to_owned(),
        "(allow sysctl-read)".to_owned(),
        "(allow mach-lookup)".to_owned(),
    ];
    // Node and other script runtimes load system and package files from many roots. The trusted
    // Filesystem preset enforces canonical read roots itself; sandbox-exec independently prevents
    // mutation and network access outside the policy below.
    rules.push("(allow file-read*)".to_owned());
    for directory in &config.allowed_directories {
        let resolved = std::fs::canonicalize(directory).map_err(|error| {
            McpError::Policy(format!(
                "allowed directory '{directory}' cannot be resolved: {error}"
            ))
        })?;
        if !resolved.is_dir() {
            return Err(McpError::Policy(format!(
                "allowed path '{directory}' is not a directory"
            )));
        }
        let directory = escape_profile_path(&resolved)?;
        rules.push(format!("(allow file-write* (subpath \"{directory}\"))"));
    }
    if config.allow_network {
        rules.push("(allow network-outbound)".to_owned());
    }
    Ok(rules.join("\n"))
}

#[cfg(target_os = "macos")]
fn escape_profile_path(path: &Path) -> Result<String, McpError> {
    let value = path.to_string_lossy();
    if value.contains(['\n', '\r', '\0']) {
        return Err(McpError::Policy(
            "sandbox path contains unsupported control characters".to_owned(),
        ));
    }
    Ok(value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use crate::domain::mcp::{McpAuthKind, McpServerConfig, McpToolPolicy, McpTransportKind};
    use crate::tools::ToolRisk;
    use std::path::PathBuf;

    #[tokio::test]
    async fn risky_server_requires_an_allowed_directory() {
        let config = McpServerConfig {
            id: "writer".to_owned(),
            label: "Writer".to_owned(),
            enabled: true,
            transport: McpTransportKind::Stdio,
            executable: "/usr/bin/true".to_owned(),
            arguments: Vec::new(),
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
                name: "write".to_owned(),
                enabled: true,
                risk: ToolRisk::LocalWrite,
                idempotent: false,
                reconcile: false,
            }],
        };
        assert!(super::isolated_command(&config).await.is_err());
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn macos_sandbox_allows_scoped_write_and_rejects_escape() {
        if !PathBuf::from("/usr/bin/sandbox-exec").is_file() {
            return;
        }
        let allowed = tempfile::tempdir().unwrap();
        let blocked = tempfile::tempdir().unwrap();
        let allowed_path = std::fs::canonicalize(allowed.path()).unwrap();
        let blocked_path = std::fs::canonicalize(blocked.path()).unwrap();
        let allowed_file = allowed_path.join("allowed.txt");
        let blocked_file = blocked_path.join("blocked.txt");
        let base = |target: &std::path::Path| McpServerConfig {
            id: "writer".to_owned(),
            label: "Writer".to_owned(),
            enabled: true,
            transport: McpTransportKind::Stdio,
            executable: "/bin/sh".to_owned(),
            arguments: vec![
                "-c".to_owned(),
                "printf test > \"$1\"".to_owned(),
                "carrot-sandbox".to_owned(),
                target.display().to_string(),
            ],
            working_directory: None,
            url: None,
            auth: McpAuthKind::None,
            oauth_client_id: None,
            oauth_scopes: Vec::new(),
            preset: None,
            secret_environment_variable: None,
            read_directories: Vec::new(),
            allowed_directories: vec![allowed_path.display().to_string()],
            allowed_domains: Vec::new(),
            allow_network: false,
            tool_policies: vec![McpToolPolicy {
                name: "write".to_owned(),
                enabled: true,
                risk: ToolRisk::Dangerous,
                idempotent: false,
                reconcile: false,
            }],
        };

        let allowed_output = super::isolated_command(&base(&allowed_file))
            .await
            .unwrap()
            .output()
            .await
            .unwrap();
        assert!(
            allowed_output.status.success(),
            "sandbox status {:?}, stderr: {}",
            allowed_output.status.code(),
            String::from_utf8_lossy(&allowed_output.stderr)
        );
        assert!(allowed_file.is_file());

        let blocked_status = super::isolated_command(&base(&blocked_file))
            .await
            .unwrap()
            .status()
            .await
            .unwrap();
        assert!(!blocked_status.success());
        assert!(!blocked_file.exists());
    }
}
