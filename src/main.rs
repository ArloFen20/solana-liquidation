use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, Subcommand};
use tracing::{error, info, warn};

mod config;
mod kamino;
mod health;
mod jito;
mod util;

use crate::config::Config;
use crate::kamino::{find_liquidation_candidates, build_liquidation_ix};
use crate::jito::{JitoSender, TipAccount};
use crate::util::{build_tx_with_tip, fetch_latest_blockhash};

/// Kamino liquidation bot entrypoint.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// RPC URL for Solana cluster
    #[arg(long, env = "RPC_URL")]
    rpc_url: Option<String>,

    /// Path to payer keypair
    #[arg(long, env = "PAYER", value_name = "FILE")]
    payer: Option<PathBuf>,

    /// Kamino Lending market address
    #[arg(long, env = "MARKET", default_value = "7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF")]
    market: String,

    /// Tip lamports to include per liquidation tx
    #[arg(long, env = "TIP_LAMPORTS", default_value_t = 5_000)]
    tip_lamports: u64,

    /// Compute unit price (micro-lamports per CU)
    #[arg(long, env = "CU_PRICE", default_value_t = 2_000)]
    cu_price: u64,

    /// Compute unit limit override
    #[arg(long, env = "CU_LIMIT", default_value_t = 300_000)]
    cu_limit: u32,

    /// Use dry-run (simulate only, no send)
    #[arg(long, action = ArgAction::SetTrue)]
    dry_run: bool,

    /// Run one iteration then exit
    #[arg(long, action = ArgAction::SetTrue)]
    once: bool,

    /// Jito gRPC timeout in seconds
    #[arg(long, env = "JITO_TIMEOUT", default_value_t = 2)]
    jito_timeout: u64,

    /// Optional explicit Jito region endpoint
    #[arg(long, env = "JITO_ENDPOINT")]
    jito_endpoint: Option<String>,

    /// Optional explicit tip account to use
    #[arg(long, env = "TIP_ACCOUNT")]
    tip_account: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    let cfg = Config::from_env(cli.rpc_url.clone(), cli.payer.clone())?;

    info!(rpc = %cfg.rpc_url, payer = %cfg.payer_path.display(), "Starting Kamino liquidation bot");

    // Initialize RPC client and jito sender
    let rpc = solana_client::rpc_client::RpcClient::new(cfg.rpc_url.clone());
    let mut jito = JitoSender::new(cli.jito_endpoint.clone(), Some(cli.jito_timeout)).await?;

    // Select tip account
    let tip_acc = if let Some(acc) = cli.tip_account.as_ref() {
        TipAccount::from_str(acc)?
    } else {
        TipAccount::random()
    };

    // Main loop
    loop {
        // Fetch latest blockhash for transaction building
        let blockhash = fetch_latest_blockhash(&rpc)?;

        // Find candidates
        let candidates = find_liquidation_candidates(&rpc, &cli.market).await?;
        if candidates.is_empty() {
            info!("No liquidatable obligations found");
        }

        for cand in candidates.iter() {
            match build_liquidation_ix(&rpc, cand).await {
                Ok(ix) => {
                    // Build and optionally send transaction via Jito
                    match build_tx_with_tip(
                        &cfg.payer,
                        blockhash,
                        vec![ix],
                        cli.cu_limit,
                        cli.cu_price,
                        tip_acc.pubkey,
                        cli.tip_lamports,
                    ) {
                        Ok(versioned_tx) => {
                            if cli.dry_run {
                                info!(
                                    obligation = %cand.obligation.to_string(),
                                    "Dry-run: built liquidation tx"
                                );
                            } else {
                                match jito.send(&[versioned_tx]).await {
                                    Ok(uuid) => {
                                        info!(
                                            obligation = %cand.obligation.to_string(),
                                            jito_uuid = %uuid,
                                            tip = cli.tip_lamports,
                                            "Bundle submitted"
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            obligation = %cand.obligation.to_string(),
                                            error = %e,
                                            "Failed to submit bundle"
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => warn!(error = %e, "Failed to build liquidation transaction"),
                    }
                }
                Err(e) => warn!(error = %e, "Failed to build liquidation instruction"),
            }
        }

        if cli.once { break; }

        // Sleep briefly before next scan
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    Ok(())
}

