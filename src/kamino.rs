use anyhow::{Context, Result};
use carbon_kamino_lending_decoder::{KaminoLendingDecoder, PROGRAM_ID};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::instruction::Instruction;

use crate::health::estimate_health;

/// Minimal liquidation candidate data needed for instruction building.
pub struct LiquidationCandidate {
    pub obligation: Pubkey,
    pub market: Pubkey,
    pub repay_reserve: Pubkey,
    pub withdraw_reserve: Pubkey,
}

/// Scan Kamino program accounts and return liquidatable obligations for a given market.
pub async fn find_liquidation_candidates(rpc: &RpcClient, market_addr: &str) -> Result<Vec<LiquidationCandidate>> {
    let market: Pubkey = market_addr.parse()?;
    let decoder = KaminoLendingDecoder::default();

    // Fetch all accounts owned by the program and filter obligations
    let accs = rpc
        .get_program_accounts(&PROGRAM_ID)
        .context("Failed to get Kamino program accounts")?;

    let mut reserves = Vec::new();
    let mut obligations = Vec::new();

    for (pk, acc) in accs.iter() {
        // Try decode reserve first
        if let Ok(reserve) = decoder.decode_reserve(&acc.data) {
            reserves.push((pk, reserve));
            continue;
        }
        // Try decode obligation
        if let Ok(obligation) = decoder.decode_obligation(&acc.data) {
            obligations.push((pk, obligation));
            continue;
        }
    }

    // Index reserves by pubkey for health computation
    use std::collections::HashMap;
    let mut reserve_map = HashMap::new();
    for (pk, r) in reserves.into_iter() {
        reserve_map.insert(*pk, r);
    }

    let mut candidates = Vec::new();
    for (pk, obl) in obligations.into_iter() {
        // Filter by market
        if obl.lending_market != market { continue; }

        // Estimate health
        if let Ok(h) = estimate_health(&obl, &reserve_map, rpc) {
            if h < 1.0 {
                // Choose largest borrow and largest collateral
                let repay_reserve = obl.borrows.iter().max_by_key(|b| b.amount).map(|b| b.reserve).unwrap_or_default();
                let withdraw_reserve = obl.deposits.iter().max_by_key(|d| d.amount).map(|d| d.reserve).unwrap_or_default();
                if repay_reserve != Pubkey::default() && withdraw_reserve != Pubkey::default() {
                    candidates.push(LiquidationCandidate {
                        obligation: *pk,
                        market,
                        repay_reserve,
                        withdraw_reserve,
                    });
                }
            }
        }
    }

    Ok(candidates)
}

/// Build a liquidation instruction for the given candidate.
pub async fn build_liquidation_ix(rpc: &RpcClient, cand: &LiquidationCandidate) -> Result<Instruction> {
    let decoder = KaminoLendingDecoder::default();

    // Fetch obligation account data to determine amounts
    let obl_acc = rpc.get_account(&cand.obligation).context("Failed to fetch obligation")?;
    let obl = decoder.decode_obligation(&obl_acc.data).context("Failed to decode obligation")?;

    // Repay 20% of largest borrow
    let largest_borrow = obl.borrows.iter().max_by_key(|b| b.amount).context("No borrows")?;
    let repay_amount = largest_borrow.amount / 5; // 20%

    // Withdraw reserve is chosen from largest deposit
    let largest_deposit = obl.deposits.iter().max_by_key(|d| d.amount).context("No deposits")?;

    // Construct instruction using decoder-generated builders
    let accounts = carbon_kamino_lending_decoder::instructions::liquidate_obligation::LiquidateObligationInstructionAccounts {
        lending_market: cand.market,
        obligation: cand.obligation,
        repay_reserve: cand.repay_reserve,
        withdraw_reserve: cand.withdraw_reserve,
        // Placeholder accounts which decoder resolves internally if optional; they may be filled below if required
        owner: obl.owner,
        token_program: spl_token::ID,
    };

    let args = carbon_kamino_lending_decoder::instructions::liquidate_obligation::LiquidateObligationInstructionArgs {
        liquidity_amount: repay_amount,
        min_out: 0u64, // accept any collateral out, price protected by HF check
    };

    let ix = carbon_kamino_lending_decoder::instructions::liquidate_obligation::build(accounts, args)?;

    Ok(ix)
}

