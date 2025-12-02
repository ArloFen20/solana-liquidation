use anyhow::Result;
use carbon_kamino_lending_decoder::types;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;

/// Estimate health factor of an obligation.
/// Returns a value < 1.0 for liquidatable positions.
/// Note: This is a simplified off-chain approximation intended to act as a pre-filter.
pub fn estimate_health(
    obligation: &types::Obligation,
    reserves: &HashMap<Pubkey, types::Reserve>,
    _rpc: &RpcClient,
) -> Result<f64> {
    // Fallback approximation: treat any position with borrows > 0 and deposits == 0 as unhealthy
    let total_borrow = obligation.borrows.iter().map(|b| b.amount as f64).sum::<f64>();
    let total_deposit = obligation.deposits.iter().map(|d| d.amount as f64).sum::<f64>();

    if total_borrow > 0.0 && total_deposit == 0.0 {
        return Ok(0.0);
    }

    // Otherwise compute a naive ratio; assume equal prices for rough filtering
    let hf = if total_borrow == 0.0 {
        f64::INFINITY
    } else {
        (total_deposit * 0.75) / total_borrow
    };

    Ok(hf)
}

