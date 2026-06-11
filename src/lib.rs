pub use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use evdev::KeyCode;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::fs;

pub type SharedMap = Arc<Mutex<HashMap<KeyCode, KeyLog>>>;

pub const PATH: &str = "/var/lib/keyheatmap/map";
pub const PATH_BAK: &str = "/var/lib/keyheatmap/map.bak";

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct KeyLog {
    pub count: u64,
    pub time_ms: u128,
}

pub async fn save_hashmap(map: &SharedMap) {
    let map = map.lock().unwrap();
    let json = serde_json::to_string(&*map).expect("failed to serialize hashmap");
    match fs::write(PATH, json).await {
        Ok(v) => v,
        Err(e) => {
            warn!("failed to write to save file: {}", e);
        }
    };
    info!("saved to file!");
}

pub async fn load_hashmap() -> HashMap<KeyCode, KeyLog> {
    let read_backup_hashmap = async || -> Option<String> {
        match fs::read_to_string(format!("{}.bak", PATH_BAK)).await {
            Ok(v) => {
                warn!("read backup OK, overwriting main");
                if let Err(e) = fs::copy(PATH_BAK, PATH).await {
                    warn!("failed to copy save file: {}", e.to_string());
                }
                Some(v)
            }
            Err(e) => {
                warn!(
                    "Failed to open backup file: {}, starting empty session",
                    e.to_string()
                );
                None
            }
        }
    };
    let json = match fs::read_to_string(PATH).await {
        Ok(v) => v,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                warn!("No save file found, starting empty session");
                if let Err(e) = fs::create_dir(PATH.strip_suffix("/map").unwrap()).await {
                    if e.kind() != std::io::ErrorKind::AlreadyExists {
                        error!(
                            "Failed to create storage directory: {}, starting empty session",
                            e.to_string()
                        );
                        std::process::exit(1)
                    }
                }
                return HashMap::default();
            }
            warn!("failed to open save file: {}, trying backup", e.to_string());
            let Some(v) = read_backup_hashmap().await else {
                return HashMap::default();
            };
            v
        }
    };
    match serde_json::from_str(&json) {
        Ok(v) => {
            info!("read save OK, overwriting backup");
            if let Err(e) = fs::copy(PATH, PATH_BAK).await {
                warn!("failed to copy save file: {}, trying backup", e.to_string());
            }
            v
        }
        Err(e) => {
            warn!("Failed to load save file: {}, trying backup", e.to_string());
            let Some(json) = read_backup_hashmap().await else {
                return HashMap::default();
            };
            match serde_json::from_str(&json) {
                Ok(v) => v,
                Err(e) => {
                    warn!(
                        "Failed to load backup file: {}, starting empty session",
                        e.to_string()
                    );
                    HashMap::new()
                }
            }
        }
    }
}
