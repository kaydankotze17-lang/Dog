use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Personality {
    Normal,
    Sarcastic,
    Chaotic,
    Professional,
    Silent,
}

impl Personality {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "sarcastic" => Some(Self::Sarcastic),
            "chaotic" => Some(Self::Chaotic),
            "professional" => Some(Self::Professional),
            "silent" => Some(Self::Silent),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Sarcastic => "sarcastic",
            Self::Chaotic => "chaotic",
            Self::Professional => "professional",
            Self::Silent => "silent",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DogConfig {
    pub personality: Personality,
    pub runs_completed: u64,
}

impl Default for DogConfig {
    fn default() -> Self {
        Self {
            personality: Personality::Sarcastic,
            runs_completed: 0,
        }
    }
}

impl DogConfig {
    pub fn home_dir() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".dog")
        } else {
            PathBuf::from(".dog")
        }
    }

    pub fn config_path() -> PathBuf {
        Self::home_dir().join("config")
    }

    pub fn load() -> Self {
        let mut cfg = Self::default();
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                if let Some((key, val)) = line.split_once('=') {
                    match key.trim() {
                        "personality" => {
                            if let Some(mode) = Personality::parse(val) {
                                cfg.personality = mode;
                            }
                        }
                        "runs_completed" => {
                            if let Ok(n) = val.trim().parse::<u64>() {
                                cfg.runs_completed = n;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create config directory: {e}"))?;
            }
        }
        let body = format!(
            "personality={}\nruns_completed={}\n",
            self.personality.as_str(),
            self.runs_completed
        );
        fs::write(&path, body).map_err(|e| format!("Failed to write config: {e}"))
    }

    pub fn record_run(&mut self) {
        self.runs_completed = self.runs_completed.saturating_add(1);
        let _ = self.save();
    }
}
