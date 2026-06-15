use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

#[derive(Debug, Clone)]
pub struct StorePaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub sessions_dir: PathBuf,
}

impl StorePaths {
    pub fn discover() -> Result<Self> {
        if let Ok(data_dir) = std::env::var("CFW_DATA_DIR") {
            let data_dir = PathBuf::from(data_dir);
            return Ok(Self {
                db_path: data_dir.join("context-firewall.db"),
                sessions_dir: data_dir.join("sessions"),
                data_dir,
            });
        }

        let project_dirs = ProjectDirs::from("dev", "context-firewall", "context-firewall")
            .context("could not resolve platform data directory")?;
        let data_dir = project_dirs.data_local_dir().to_path_buf();
        Ok(Self {
            db_path: data_dir.join("context-firewall.db"),
            sessions_dir: data_dir.join("sessions"),
            data_dir,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.sessions_dir)
            .with_context(|| format!("could not create {}", self.sessions_dir.display()))?;
        Ok(())
    }
}
