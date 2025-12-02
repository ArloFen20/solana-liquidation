use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use rand::thread_rng;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::VersionedTransaction;

/// Known Jito tip accounts (mainnet-beta).
pub const JITO_TIP_ACCOUNTS: [&str; 8] = [
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

/// Simple holder for selected tip account.
pub struct TipAccount {
    pub pubkey: solana_sdk::pubkey::Pubkey,
}

impl TipAccount {
    /// Choose a random tip account.
    pub fn random() -> Self {
        let mut rng = thread_rng();
        let acc = JITO_TIP_ACCOUNTS.choose(&mut rng).unwrap();
        Self { pubkey: acc.parse().unwrap() }
    }

    /// Parse from string.
    pub fn from_str(s: &str) -> Result<Self> {
        Ok(Self { pubkey: s.parse()? })
    }
}

/// Wrapper around jito-grpc-client to submit bundles and get UUIDs.
pub struct JitoSender {
    client: jito_grpc_client::JitoClient,
}

impl JitoSender {
    /// Create with dynamic region selection or explicit endpoint.
    pub async fn new(endpoint: Option<String>, timeout_secs: Option<u64>) -> Result<Self> {
        let client = if let Some(ep) = endpoint {
            jito_grpc_client::JitoClient::new(Box::leak(ep.into_boxed_str()), timeout_secs)
                .await
                .context("Failed to initialize Jito client")?
        } else {
            jito_grpc_client::JitoClient::new_dynamic_region(timeout_secs)
                .await
                .context("Failed to initialize Jito dynamic client")?
        };
        Ok(Self { client })
    }

    /// Send bundle and return UUID string.
    pub async fn send(&mut self, txs: &[VersionedTransaction]) -> Result<String> {
        let uuid = self.client.send(txs).await.context("Jito send failed")?;
        Ok(uuid)
    }
}

