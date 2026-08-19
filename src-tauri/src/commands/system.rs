use serde::Serialize;
use specta::Type;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub app_name: String,
    pub app_version: String,
    pub platform: String,
    pub phase: String,
}

#[tauri::command]
#[specta::specta]
pub fn health_check() -> Result<HealthStatus, AppError> {
    Ok(HealthStatus {
        app_name: "Carrot".to_owned(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: std::env::consts::OS.to_owned(),
        phase: "P2 provider runtime".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_the_application_baseline() {
        let status = super::health_check().expect("health check should succeed");

        assert_eq!(status.app_name, "Carrot");
        assert_eq!(status.phase, "P2 provider runtime");
        assert!(!status.app_version.is_empty());
        assert!(!status.platform.is_empty());
    }
}
