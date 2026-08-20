# MCP production acceptance (macOS)

This checklist covers integrations that require live credentials or third-party services and are
therefore not part of the default test run.

## Curated packages

Carrot installs curated MCP packages into its private configuration directory. Package names and
versions are fixed in the application binary:

- `@modelcontextprotocol/server-filesystem@2026.7.10`
- `@brave/brave-search-mcp-server@2.1.0`

Installation uses `npm install --ignore-scripts --save-exact`. The generated configuration uses an
absolute executable path. Filesystem roots and search result domains remain explicit policy fields
in `mcp-servers.toml`.

## Official Filesystem MCP

Run the live server acceptance test on macOS:

```bash
cd src-tauri
cargo test real_filesystem_preset_reads_only_the_workspace -- --ignored --nocapture
```

The test installs the pinned package, connects over stdio, reads a marker inside a temporary
workspace, and verifies that a path outside that workspace is rejected.

For Provider + Filesystem MCP acceptance in the application:

The automated live path requires three environment variables:

```bash
cd src-tauri
CARROT_REAL_PROVIDER_API_KEY='...' \
CARROT_REAL_PROVIDER_BASE_URL='https://api.openai.com/v1' \
CARROT_REAL_PROVIDER_MODEL='your-tool-capable-model' \
  cargo test real_provider_and_filesystem_mcp_complete_a_tool_loop -- --ignored --nocapture
```

The equivalent release UI check is:

1. Configure a real Provider and store its API key in Keychain.
2. In MCP settings, install **Workspace Files** for a disposable repository.
3. Enable `read_text_file`, `list_directory`, and `search_files` as read-only tools.
4. Ask the Provider for a fact that can only be found in a marker file in that repository.
5. Verify the run contains the MCP tool execution and the answer contains the marker.
6. Enable `write_file` as a local-write tool and add the disposable repository as an allowed write
   directory. Verify the approval banner shows the unified diff before approval.
7. Repeat with a path outside the configured root and verify the tool is rejected.

## Brave Search MCP

Use a real Brave Search API key without persisting it in the repository:

```bash
cd src-tauri
CARROT_BRAVE_API_KEY='...' \
  cargo test real_brave_search_preset_returns_live_results -- --ignored --nocapture
```

In the application, install **Brave Search**, save the key to Keychain, then optionally configure
allowed result domains. Exact hosts and their subdomains are accepted; lookalike suffixes and all
other URL hosts are rejected before the result reaches the model.

## Streamable HTTP OAuth

Use a disposable account and a loopback callback. Record the server URL, authorization server,
client ID, scopes, expected audience, and test account in the release evidence.

1. Add the real HTTPS MCP endpoint and select OAuth.
2. Start authorization and verify the request uses PKCE, a fresh state value, and the MCP endpoint
   as the resource/audience.
3. Complete the exact loopback callback and verify the token is stored in Keychain.
4. Connect and call a read-only tool. Confirm no token appears in logs or `mcp-servers.toml`.
5. Expire the access token and verify refresh succeeds without another authorization prompt.
6. Change the MCP endpoint and verify the previous credential is deleted and cannot be reused.
7. Attempt a redirect, URL credential, mismatched state, mismatched callback port/path, and wrong
   audience. Each case must fail closed.

Attach sanitized server logs and the Carrot catalog revision/state transitions to release evidence.
