use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MotionPreference {
    System,
    #[default]
    Subtle,
    Off,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema_version: u32,
    pub git_path: Option<PathBuf>,
    pub recent_projects: Vec<PathBuf>,
    pub appearance: Appearance,
    pub motion: MotionPreference,
    pub history_display: HistoryDisplay,
    pub file_panel_width: f32,
    pub detail_panel_height: f32,
    pub updates: crate::update::Preferences,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryDisplay {
    pub graph: bool,
    pub hash: bool,
    pub branches: bool,
    pub tags: bool,
    pub metadata_after: bool,
}

impl Default for HistoryDisplay {
    fn default() -> Self {
        Self {
            graph: true,
            hash: false,
            branches: true,
            tags: true,
            metadata_after: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            git_path: None,
            recent_projects: Vec::new(),
            appearance: Appearance::System,
            motion: MotionPreference::default(),
            history_display: HistoryDisplay::default(),
            file_panel_width: 292.,
            detail_panel_height: 310.,
            updates: crate::update::Preferences::default(),
        }
    }
}

impl Config {
    pub fn remember_project(&mut self, path: &Path) {
        self.recent_projects.retain(|item| item != path);
        self.recent_projects.insert(0, path.to_path_buf());
        self.recent_projects.truncate(12);
    }
}

#[derive(Clone)]
pub struct ConfigStore {
    pub path: PathBuf,
}

impl ConfigStore {
    pub fn for_app() -> Result<Self> {
        let base = BaseDirs::new().context("无法定位 AppData 配置目录")?;
        Ok(Self {
            path: base.config_dir().join("githly").join("config.json"),
        })
    }

    pub fn load(&self) -> Result<Config> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Config::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("无法读取 {}", self.path.display()));
            }
        };
        let config: Config =
            serde_json::from_slice(&bytes).context("配置文件格式无效，原文件已保留")?;
        anyhow::ensure!(
            config.schema_version == 1,
            "不支持此配置文件版本，原文件已保留"
        );
        Ok(config)
    }

    pub fn save(&self, config: &Config) -> Result<()> {
        let directory = self.path.parent().context("配置目录无效")?;
        fs::create_dir_all(directory).context("无法创建 AppData 配置目录")?;
        let temporary = directory.join("config.json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(config)?).context("无法写入配置")?;
        fs::rename(&temporary, &self.path).context("无法保存配置文件")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_round_trip_and_recent_deduplication() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore {
            path: dir.path().join("githly/config.json"),
        };
        let mut config = store.load().unwrap();
        config.git_path = Some(PathBuf::from("D:/Tools/Git/cmd/git.exe"));
        config.remember_project(Path::new("E:/项目"));
        config.remember_project(Path::new("E:/项目"));
        store.save(&config).unwrap();
        config.appearance = Appearance::Dark;
        config.motion = MotionPreference::Off;
        store.save(&config).unwrap();
        let restored = store.load().unwrap();
        assert_eq!(restored.git_path, config.git_path);
        assert_eq!(restored.recent_projects.len(), 1);
        assert_eq!(restored.recent_projects, [PathBuf::from("E:/项目")]);
        assert_eq!(restored.appearance, Appearance::Dark);
        assert_eq!(restored.motion, MotionPreference::Off);
    }
    #[test]
    fn obsolete_horizontal_scroll_preference_is_ignored_and_removed_on_save() {
        let config: Config =
            serde_json::from_str(r#"{"diff_soft_wrap":false,"appearance":"dark"}"#).unwrap();
        assert_eq!(config.appearance, Appearance::Dark);
        assert!(
            !serde_json::to_value(config)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("diff_soft_wrap")
        );
    }
    #[test]
    fn malformed_config_is_not_replaced_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore {
            path: dir.path().join("config.json"),
        };
        fs::write(&store.path, b"invalid").unwrap();
        assert!(store.load().is_err());
        assert_eq!(fs::read(&store.path).unwrap(), b"invalid");
    }

    #[test]
    fn legacy_active_project_is_ignored_and_recents_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore {
            path: dir.path().join("config.json"),
        };
        fs::write(
            &store.path,
            br#"{"recent_projects":["E:/codes/bookmarks-manager"],"active_project":"E:/codes/bookmarks-manager"}"#,
        )
        .unwrap();
        let mut config = store.load().unwrap();
        assert_eq!(config.motion, MotionPreference::Subtle);
        config.history_display.hash = true;
        store.save(&config).unwrap();
        let restored = store.load().unwrap();
        assert_eq!(
            restored.recent_projects,
            [PathBuf::from("E:/codes/bookmarks-manager")]
        );
        assert!(
            serde_json::to_value(&restored)
                .unwrap()
                .get("active_project")
                .is_none()
        );
        assert!(restored.history_display.hash);
    }
}
