use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::hash::Hash;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::{Message, VersionedMessage};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::transaction::VersionedTransaction;

/// Fetch latest blockhash from RPC.
pub fn fetch_latest_blockhash(rpc: &RpcClient) -> Result<Hash> {
    let bh = rpc.get_latest_blockhash().context("Failed to fetch blockhash")?;
    Ok(bh)
}

/// Build a versioned transaction with compute budget and a Jito tip transfer.
pub fn build_tx_with_tip(
    payer: &Keypair,
    blockhash: Hash,
    mut ixs: Vec<Instruction>,
    cu_limit: u32,
    cu_price: u64,
    tip_account: solana_sdk::pubkey::Pubkey,
    tip_lamports: u64,
) -> Result<VersionedTransaction> {
    // Compute budget tuning
    let budget_ixs = vec![
        ComputeBudgetInstruction::set_compute_unit_limit(cu_limit),
        ComputeBudgetInstruction::set_compute_unit_price(cu_price),
    ];

    // Tip transfer to Jito account
    let tip_ix = system_instruction::transfer(&payer.pubkey(), &tip_account, tip_lamports);

    // Compose instructions
    let mut full_ixs = Vec::with_capacity(2 + ixs.len() + 1);
    full_ixs.extend(budget_ixs);
    full_ixs.extend(ixs.drain(..));
    full_ixs.push(tip_ix);

    // Build and sign
    let msg = Message::new(&full_ixs, Some(&payer.pubkey()));
    let vmsg = VersionedMessage::V0(msg);
    let tx = VersionedTransaction::try_new(vmsg, &[payer])
        .context("Failed to sign versioned transaction")?;

    // Update recent blockhash
    let mut tx = tx;
    tx.message.set_recent_blockhash(blockhash);
    Ok(tx)
}

