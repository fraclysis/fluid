#[derive(Debug, Deserialize)]
pub struct Config {
    pub output: String,
    pub theme: String,

    pub debug: Option<DebugConfig>,

    #[serde(flatten)]
    pub other: Value,
}

impl Config {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let config: Config = serde_yaml::from_str(&fs::read_to_string(&path).exit(path)).unwrap();

        if let Some(debug) = &config.debug {
            if let Value::Mapping(map) = &debug.other {
                if !map.is_empty() {
                    println!("Warning: unused value");
                    println!("{map:?}");
                }
            }
        }

        config
    }
}

#[derive(Debug, Deserialize)]
pub struct DebugConfig {
    #[serde(default)]
    #[serde(rename = "live-reload")]
    pub live_reload: bool,
    #[serde(default = "def_port")]
    pub port: i32,
    #[serde(default = "def_host")]
    pub host: String,

    #[serde(flatten)]
    pub other: Value,
}

use std::{fs, path::Path};

use serde::Deserialize;
use serde_yaml::Value;

use crate::helper::IoError;

fn def_port() -> i32 {
    3000
}

fn def_host() -> String {
    "127.0.0.1".to_string()
}
