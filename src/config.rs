use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use dotenvy::dotenv;
use solana_sdk::signature::{read_keypair_file, Keypair};

/// Runtime configuration loaded from environment and CLI.
pub struct Config {
    pub rpc_url: String,
    pub payer_path: PathBuf,
    pub payer: Keypair,
}

impl Config {
    /// Load configuration from environment variables and optional CLI overrides.
    pub fn from_env(rpc_url_cli: Option<String>, payer_cli: Option<PathBuf>) -> Result<Self> {
        dotenv().ok();

        let rpc_url = rpc_url_cli
            .or_else(|| std::env::var("RPC_URL").ok())
            .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());

        let payer_path = payer_cli
            .or_else(|| std::env::var("PAYER").ok().map(PathBuf::from))
            .unwrap_or_else(|| dirs::home_dir().unwrap().join(".config/solana/id.json"));

        let payer = read_keypair_file(&payer_path)
            .with_context(|| format!("Failed to load payer keypair from {}", payer_path.display()))?;

        Ok(Self { rpc_url, payer_path, payer })
    }
}

