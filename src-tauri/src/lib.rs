mod agent;
mod application;
mod commands;
mod credentials;
mod domain;
mod error;
pub mod mcp;
mod persistence;
mod providers;
mod settings;
mod sync;
mod tools;

#[cfg(any(debug_assertions, test))]
use std::path::PathBuf;

#[cfg(any(debug_assertions, test))]
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{Builder, collect_commands};

fn ipc_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::system::health_check,
        commands::conversation::conversation_list,
        commands::conversation::conversation_get,
        commands::conversation::conversation_create,
        commands::conversation::conversation_update,
        commands::conversation::conversation_delete,
        commands::provider::provider_profile_list,
        commands::provider::provider_profile_reload,
        commands::provider::provider_profile_create,
        commands::provider::provider_profile_update,
        commands::provider::provider_profile_delete,
        commands::provider::provider_profile_set_default,
        commands::provider::provider_model_sync,
        commands::settings::settings_get,
        commands::settings::settings_update,
        commands::settings::credential_status_list,
        commands::settings::credential_set,
        commands::settings::credential_delete,
        commands::mcp::mcp_catalog_get,
        commands::mcp::mcp_system_settings_update,
        commands::mcp::mcp_preset_install,
        commands::mcp::mcp_server_create,
        commands::mcp::mcp_server_update,
        commands::mcp::mcp_server_delete,
        commands::mcp::mcp_server_connect,
        commands::mcp::mcp_server_disconnect,
        commands::mcp::mcp_server_refresh,
        commands::mcp::mcp_tool_policy_set,
        commands::mcp::mcp_auth_set,
        commands::mcp::mcp_auth_clear,
        commands::mcp::mcp_oauth_begin,
        commands::mcp::mcp_oauth_complete,
        commands::attachment::attachment_list,
        commands::attachment::attachment_pick_and_import,
        commands::attachment::attachment_delete,
        commands::chat::chat_start,
        commands::chat::chat_cancel,
        commands::chat::chat_pause,
        commands::chat::chat_resume,
        commands::chat::chat_input,
        commands::chat::chat_branch,
        commands::chat::chat_tool_approval,
        commands::chat::chat_tool_recovery,
        commands::chat::chat_snapshot
    ])
}

#[cfg(any(debug_assertions, test))]
fn bindings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts")
}

#[cfg(any(debug_assertions, test))]
fn export_typescript_bindings(builder: &Builder<tauri::Wry>) {
    builder
        .export(Typescript::default(), bindings_path())
        .expect("failed to export TypeScript bindings");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = ipc_builder();

    #[cfg(debug_assertions)]
    export_typescript_bindings(&builder);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let app_config_dir = app.path().app_config_dir()?;
            let service = tauri::async_runtime::block_on(application::CarrotService::initialize(
                app_data_dir.join("carrot.sqlite3"),
                app_config_dir.join("providers.toml"),
                app_config_dir.join("settings.toml"),
                app_data_dir.join("attachments"),
            ))?;
            app.manage(service);
            Ok(())
        })
        .invoke_handler(builder.invoke_handler())
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            let service = app_handle.state::<application::CarrotService>();
            let _ = tauri::async_runtime::block_on(service.prepare_for_suspend());
        }
        tauri::RunEvent::Resumed => {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let service = app_handle.state::<application::CarrotService>();
                service.resume_from_suspend().await;
            });
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn export_typescript_bindings() {
        super::export_typescript_bindings(&super::ipc_builder());
        assert!(super::bindings_path().is_file());
    }
}
