use crate::store;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Config {
    pub current: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            current: "default".to_string(),
        }
    }
}

pub fn load() -> Result<Config> {
    let path = store::config_file()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = fs::read_to_string(&path)?;
    let config: Config = serde_json::from_str(&text)?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = store::config_file()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(config)?;
    fs::write(&path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_sane_values() {
        let c = Config::default();
        assert_eq!(c.current, "default");
    }

    #[test]
    fn round_trips_through_json() {
        let c = Config {
            current: "japanese".to_string(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }
}
