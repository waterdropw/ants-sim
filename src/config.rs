//! Config — TOML-loaded genome + world + sim parameters. Phase 2 (evolution)
//! and the overnight sweeps both drive the sim through this struct so a run
//! is fully reproducible from a file.

use crate::genome::Genome;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldCfg {
    pub width: usize,
    pub height: usize,
    pub nest_x: f32,
    pub nest_y: f32,
    pub nest_radius: f32,
}

impl Default for WorldCfg {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            nest_x: 128.0,
            nest_y: 128.0,
            nest_radius: 6.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimCfg {
    pub seed: u64,
    pub colony_size: usize,
    pub ticks_per_frame: u32,
}

impl Default for SimCfg {
    fn default() -> Self {
        Self {
            seed: 42,
            colony_size: 40,
            ticks_per_frame: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub genome: Genome,
    pub world: WorldCfg,
    pub sim: SimCfg,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            genome: Genome::default(),
            world: WorldCfg::default(),
            sim: SimCfg::default(),
        }
    }
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let txt = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading config {path}: {e}"))?;
        let cfg: Config = toml::from_str(&txt)?;
        Ok(cfg)
    }

    /// Serialize to a TOML string (for generating `config/default.toml`).
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }
}
