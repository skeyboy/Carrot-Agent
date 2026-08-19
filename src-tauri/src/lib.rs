mod agent;
mod application;
mod commands;
mod credentials;
mod domain;
mod error;
mod persistence;
mod providers;
mod sync;
mod tools;

use std::path::PathBuf;

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
        commands::provider::provider_profile_reload
    ])
}

fn bindings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts")
}

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

    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let app_config_dir = app.path().app_config_dir()?;
            let service = tauri::async_runtime::block_on(application::CarrotService::initialize(
                app_data_dir.join("carrot.sqlite3"),
                app_config_dir.join("providers.toml"),
            ))?;
            app.manage(service);
            Ok(())
        })
        .invoke_handler(builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn export_typescript_bindings() {
        super::export_typescript_bindings(&super::ipc_builder());
        assert!(super::bindings_path().is_file());
    }
}
