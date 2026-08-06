use std::fs;
use std::path::PathBuf;

pub struct RestoreTokenStorage {
    file_path: PathBuf,
}

impl RestoreTokenStorage {
    pub fn default_path() -> PathBuf {
        let config_base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config")
            });
        config_base.join("vectrace").join("portal_restore_token.txt")
    }

    pub fn new() -> Self {
        Self {
            file_path: Self::default_path(),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { file_path: path }
    }

    pub fn load_token(&self) -> Option<String> {
        fs::read_to_string(&self.file_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn save_token(&self, token: &str) -> bool {
        if let Some(parent) = self.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&self.file_path, token).is_ok()
    }

    pub fn clear_token(&self) {
        let _ = fs::remove_file(&self.file_path);
    }
}

impl Default for RestoreTokenStorage {
    fn default() -> Self {
        Self::new()
    }
}
