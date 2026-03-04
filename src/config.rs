use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::models::AppConfig;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "Mergen";
const APPLICATION: &str = "MergenADE";

pub fn config_path() -> io::Result<PathBuf> {
    let dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "App data directory not available"))?;

    let config_dir = dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("config.toml"))
}

pub fn load_config(path: &Path) -> io::Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let text = fs::read_to_string(path)?;
    let parsed = toml::from_str::<AppConfig>(&text)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;

    Ok(parsed)
}

pub fn save_config(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension("toml.tmp");
    let data = toml::to_string_pretty(config)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;

    fs::write(&tmp_path, data)?;

    if path.exists() {
        fs::remove_file(path)?;
    }

    fs::rename(tmp_path, path)?;
    Ok(())
}
