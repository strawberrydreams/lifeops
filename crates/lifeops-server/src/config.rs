use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const CONFIG_FILE: &str = "config.json";

static NEXT_CONFIG_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BindScope {
    #[default]
    Localhost,
    Lan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub bind_scope: BindScope,
    #[serde(default)]
    pub backup_dir: Option<PathBuf>,
    #[serde(default = "default_backup_keep")]
    pub backup_keep: usize,
}

fn default_backup_keep() -> usize {
    7
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_scope: BindScope::Localhost,
            backup_dir: None,
            backup_keep: default_backup_keep(),
        }
    }
}

impl AppConfig {
    pub fn bind_ip(&self) -> IpAddr {
        match self.bind_scope {
            BindScope::Localhost => IpAddr::V4(Ipv4Addr::LOCALHOST),
            BindScope::Lan => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        }
    }

    pub fn resolved_backup_dir(&self, data_dir: &Path) -> PathBuf {
        match &self.backup_dir {
            Some(path) if path.is_absolute() => path.clone(),
            Some(path) => data_dir.join(path),
            None => data_dir.join("backups"),
        }
    }
}

pub fn load_config(data_dir: &Path) -> AppConfig {
    let path = data_dir.join(CONFIG_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return AppConfig::default(),
        Err(error) => {
            tracing::warn!("config.json 읽기 실패, 기본값 사용: {error}");
            return AppConfig::default();
        }
    };
    let mut config: AppConfig = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        tracing::warn!("config.json 파싱 실패, 기본값 사용: {error}");
        AppConfig::default()
    });
    if config.backup_keep == 0 {
        tracing::warn!("config.json의 backup_keep=0은 유효하지 않아 기본값 7 사용");
        config.backup_keep = default_backup_keep();
    }
    config
}

pub fn save_config(data_dir: &Path, config: &AppConfig) -> io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let json = serde_json::to_vec_pretty(config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let (temp, mut file) = loop {
        let id = NEXT_CONFIG_TEMP.fetch_add(1, Ordering::Relaxed);
        let temp = data_dir.join(format!(".{CONFIG_FILE}.tmp-{}-{id}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };

    let write_result = file.write_all(&json).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temp, data_dir.join(CONFIG_FILE)) {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 파일_없으면_기본값() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_config(dir.path());
        assert_eq!(config.bind_scope, BindScope::Localhost);
        assert_eq!(config.backup_dir, None);
        assert_eq!(config.backup_keep, 7);
    }

    #[test]
    fn 손상되면_기본값() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), b"{ not json").unwrap();
        assert_eq!(load_config(dir.path()), AppConfig::default());
    }

    #[test]
    fn 부분필드는_기본값과_병합() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), br#"{"bind_scope":"lan"}"#).unwrap();
        let config = load_config(dir.path());
        assert_eq!(config.bind_scope, BindScope::Lan);
        assert_eq!(config.backup_keep, 7);
    }

    #[test]
    fn 저장하고_다시_로드하면_동일() {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            bind_scope: BindScope::Lan,
            backup_dir: Some(PathBuf::from("/tmp/backups")),
            backup_keep: 3,
        };
        save_config(dir.path(), &config).unwrap();
        assert_eq!(load_config(dir.path()), config);
    }

    #[test]
    fn bind_ip_매핑() {
        assert_eq!(
            AppConfig {
                bind_scope: BindScope::Localhost,
                ..Default::default()
            }
            .bind_ip(),
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(
            AppConfig {
                bind_scope: BindScope::Lan,
                ..Default::default()
            }
            .bind_ip(),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        );
    }

    #[test]
    fn resolved_backup_dir_기본은_data_backups() {
        assert_eq!(
            AppConfig::default().resolved_backup_dir(Path::new("/d")),
            Path::new("/d/backups")
        );
        let configured = AppConfig {
            backup_dir: Some(PathBuf::from("/cloud")),
            ..Default::default()
        };
        assert_eq!(
            configured.resolved_backup_dir(Path::new("/d")),
            Path::new("/cloud")
        );
        let relative = AppConfig {
            backup_dir: Some(PathBuf::from("cloud/backups")),
            ..Default::default()
        };
        assert_eq!(
            relative.resolved_backup_dir(Path::new("/d")),
            Path::new("/d/cloud/backups")
        );
    }

    #[test]
    fn backup_keep_0은_로드할_때_기본값으로_정규화() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), br#"{"backup_keep":0}"#).unwrap();
        assert_eq!(load_config(dir.path()).backup_keep, 7);
    }
}
