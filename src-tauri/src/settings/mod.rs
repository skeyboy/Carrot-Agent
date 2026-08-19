use std::path::{Path, PathBuf};

use tokio::sync::RwLock;

use crate::domain::settings::AppSettings;

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("settings could not be read: {0}")]
    Read(String),
    #[error("settings could not be written: {0}")]
    Write(String),
    #[error("settings are invalid: {0}")]
    Invalid(String),
}

pub struct SettingsStore {
    path: PathBuf,
    value: RwLock<AppSettings>,
}

impl SettingsStore {
    pub async fn load(path: PathBuf) -> Result<Self, SettingsError> {
        let value = if tokio::fs::try_exists(&path)
            .await
            .map_err(|error| SettingsError::Read(error.to_string()))?
        {
            let source = tokio::fs::read_to_string(&path)
                .await
                .map_err(|error| SettingsError::Read(error.to_string()))?;
            let value: AppSettings = toml::from_str(&source)
                .map_err(|error| SettingsError::Invalid(error.to_string()))?;
            value.validate().map_err(SettingsError::Invalid)?;
            value
        } else {
            AppSettings::default()
        };

        let store = Self {
            path,
            value: RwLock::new(value),
        };
        if !tokio::fs::try_exists(&store.path)
            .await
            .map_err(|error| SettingsError::Read(error.to_string()))?
        {
            let initial = store.value.read().await.clone();
            store.write(&initial).await?;
        }
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn get(&self) -> AppSettings {
        self.value.read().await.clone()
    }

    pub async fn update(&self, value: AppSettings) -> Result<AppSettings, SettingsError> {
        value.validate().map_err(SettingsError::Invalid)?;
        self.write(&value).await?;
        *self.value.write().await = value.clone();
        Ok(value)
    }

    async fn write(&self, value: &AppSettings) -> Result<(), SettingsError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| SettingsError::Write(error.to_string()))?;
        }
        let source = toml::to_string_pretty(value)
            .map_err(|error| SettingsError::Write(error.to_string()))?;
        let temporary = self.path.with_extension("toml.tmp");
        tokio::fs::write(&temporary, source)
            .await
            .map_err(|error| SettingsError::Write(error.to_string()))?;
        tokio::fs::rename(&temporary, &self.path)
            .await
            .map_err(|error| SettingsError::Write(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsStore;
    use crate::domain::settings::{AppSettings, RunStrategy, ThemePreference};

    #[tokio::test]
    async fn persists_valid_settings() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.toml");
        let store = SettingsStore::load(path.clone()).await.unwrap();
        let changed = AppSettings {
            request_timeout_seconds: 90,
            max_model_steps: 12,
            attachment_max_megabytes: 8,
            default_strategy: RunStrategy::Quality,
            theme: ThemePreference::Dark,
        };
        store.update(changed.clone()).await.unwrap();

        assert_eq!(
            SettingsStore::load(path).await.unwrap().get().await,
            changed
        );
    }

    #[tokio::test]
    async fn upgrades_settings_without_a_theme_to_system() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.toml");
        tokio::fs::write(
            &path,
            "requestTimeoutSeconds = 120\nmaxModelSteps = 8\nattachmentMaxMegabytes = 20\ndefaultStrategy = \"auto\"\n",
        )
        .await
        .unwrap();

        let settings = SettingsStore::load(path).await.unwrap().get().await;
        assert_eq!(settings.theme, ThemePreference::System);
    }
}
