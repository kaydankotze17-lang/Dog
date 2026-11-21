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
}

impl Default for DogConfig {
    fn default() -> Self {
        Self {
            personality: Personality::Sarcastic,
        }
    }
}

impl DogConfig {
    pub fn config_path() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".dog").join("config")
        } else {
            PathBuf::from(".dogrc")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                if let Some((key, val)) = line.split_once('=') {
                    if key.trim() == "personality" {
                        if let Some(mode) = Personality::parse(val) {
                            return Self { personality: mode };
                        }
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create config directory: {e}"))?;
            }
        }
        let body = format!("personality={}\n", self.personality.as_str());
        fs::write(&path, body).map_err(|e| format!("Failed to write config: {e}"))
    }
}
