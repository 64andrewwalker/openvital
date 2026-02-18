use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub profile: Profile,
    #[serde(default)]
    pub units: Units,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    #[serde(default)]
    pub alerts: Alerts,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Profile {
    pub height_cm: Option<f64>,
    pub birth_year: Option<u16>,
    pub gender: Option<String>,
    #[serde(default)]
    pub conditions: Vec<String>,
    pub primary_exercise: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Units {
    #[serde(default = "default_system")]
    pub system: String,
    pub weight: String,
    pub height: String,
    pub water: String,
    pub temperature: String,
}

fn default_system() -> String {
    "metric".to_string()
}

impl Default for Units {
    fn default() -> Self {
        Self {
            system: "metric".to_string(),
            weight: "kg".to_string(),
            height: "cm".to_string(),
            water: "ml".to_string(),
            temperature: "celsius".to_string(),
        }
    }
}

impl Units {
    pub fn imperial() -> Self {
        Self {
            system: "imperial".to_string(),
            weight: "lbs".to_string(),
            height: "ft".to_string(),
            water: "fl_oz".to_string(),
            temperature: "fahrenheit".to_string(),
        }
    }

    pub fn is_imperial(&self) -> bool {
        self.system == "imperial"
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Alerts {
    pub pain_threshold: u8,
    pub pain_consecutive_days: u8,
}

impl Default for Alerts {
    fn default() -> Self {
        Self {
            pain_threshold: 5,
            pain_consecutive_days: 3,
        }
    }
}

impl Config {
    /// Load config from the standard path, or return defaults.
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&contents)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to the standard path.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    /// Resolve an alias to a metric type, or return the input unchanged.
    pub fn resolve_alias(&self, input: &str) -> String {
        self.aliases
            .get(input)
            .cloned()
            .unwrap_or_else(|| input.to_string())
    }

    /// Default aliases from the spec.
    pub fn default_aliases() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("w".into(), "weight".into());
        m.insert("bf".into(), "body_fat".into());
        m.insert("c".into(), "cardio".into());
        m.insert("s".into(), "strength".into());
        m.insert("sl".into(), "sleep_hours".into());
        m.insert("sq".into(), "sleep_quality".into());
        m.insert("wa".into(), "water".into());
        m.insert("p".into(), "pain".into());
        m.insert("so".into(), "soreness".into());
        m.insert("cal".into(), "calories_in".into());
        m.insert("st".into(), "screen_time".into());
        m
    }

    pub fn data_dir() -> PathBuf {
        if let Ok(home) = std::env::var("OPENVITAL_HOME") {
            return PathBuf::from(home);
        }
        dirs::home_dir()
            .expect("cannot resolve home directory")
            .join(".openvital")
    }

    pub fn path() -> PathBuf {
        Self::data_dir().join("config.toml")
    }

    pub fn db_path() -> PathBuf {
        Self::data_dir().join("data.db")
    }
}
