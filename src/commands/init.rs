//! `init`.

use std::fs;
use std::path::Path;

use crate::config::{DEFAULT_CONFIG, global_config_path, store_path};
use crate::error::Result;

pub fn run(dir: Option<&Path>) -> Result<()> {
    let root = store_path(dir);
    let existed = root.is_dir();
    fs::create_dir_all(root.join(".templates"))?;
    if existed {
        eprintln!("doen: the store exists: {}", root.display());
    } else {
        println!("created store {}", root.display());
    }
    let cfg = global_config_path();
    if !cfg.exists() {
        if let Some(dir) = cfg.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&cfg, DEFAULT_CONFIG)?;
        println!("created config {}", cfg.display());
    }
    Ok(())
}
