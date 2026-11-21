use crate::term::{DogMessenger, InstallStyle, TermStyleConfig, TermTheme};
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
    pub theme: TermTheme,
    pub timestamps: bool,
    pub prefix: String,
    pub debug_logs: bool,
    pub install_style: InstallStyle,
}

impl Default for DogConfig {
    fn default() -> Self {
        Self {
            personality: Personality::Sarcastic,
            runs_completed: 0,
            theme: TermTheme::Default,
            timestamps: false,
            prefix: "dog".to_string(),
            debug_logs: true,
            install_style: InstallStyle::Detailed,
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

    pub fn parse_str(content: &str) -> Self {
        let mut cfg = Self::default();
        for line in content.lines() {
            if let Some((key, val)) = line.split_once('=') {
                let v = val.trim();
                match key.trim() {
                    "personality" => {
                        if let Some(mode) = Personality::parse(v) {
                            cfg.personality = mode;
                        }
                    }
                    "runs_completed" => {
                        if let Ok(n) = v.parse::<u64>() {
                            cfg.runs_completed = n;
                        }
                    }
                    "theme" => {
                        if let Some(theme) = TermTheme::parse(v) {
                            cfg.theme = theme;
                        }
                    }
                    "timestamps" => {
                        if let Some(b) = parse_bool_flag(v) {
                            cfg.timestamps = b;
                        }
                    }
                    "prefix" => {
                        cfg.prefix = v.to_string();
                    }
                    "debug_logs" => {
                        if let Some(b) = parse_bool_flag(v) {
                            cfg.debug_logs = b;
                        }
                    }
                    "install_style" => {
                        if let Some(style) = InstallStyle::parse(v) {
                            cfg.install_style = style;
                        }
                    }
                    _ => {}
                }
            }
        }
        cfg
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            Self::parse_str(&content)
        } else {
            Self::default()
        }
    }

    pub fn render(&self) -> String {
        format!(
            "personality={}\nruns_completed={}\ntheme={}\ntimestamps={}\nprefix={}\ndebug_logs={}\ninstall_style={}\n",
            self.personality.as_str(),
            self.runs_completed,
            self.theme.as_str(),
            self.timestamps,
            self.prefix,
            self.debug_logs,
            self.install_style.as_str(),
        )
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create config directory: {e}"))?;
            }
        }
        fs::write(&path, self.render()).map_err(|e| format!("Failed to write config: {e}"))
    }

    pub fn record_run(&mut self) {
        self.runs_completed = self.runs_completed.saturating_add(1);
        let _ = self.save();
    }

    pub fn term_style(&self) -> TermStyleConfig {
        TermStyleConfig {
            theme: self.theme,
            timestamps: self.timestamps,
            prefix: self.prefix.clone(),
            debug: self.debug_logs,
            install_style: self.install_style,
            color_override: None,
            unicode_override: None,
        }
    }

    pub fn messenger(&self) -> DogMessenger {
        DogMessenger::detect(self.term_style())
    }
}

pub fn parse_bool_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" | "enable" | "enabled" => Some(true),
        "false" | "off" | "no" | "0" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}
