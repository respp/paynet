use anyhow::{Result, anyhow};
use node_client::NodeClient;
use nuts::nut04::MintQuoteState;
use nuts::nut05::MeltQuoteState;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use tonic::transport::Channel;
use wallet::types::NodeUrl;

const STARKNET_STR: &str = "starknet";

pub async fn sync_all_pending_operations(
    pool: Pool<SqliteConnectionManager>,
    _poll_interval: u64,
    _timeout: u64,
) -> Result<()> {
    let nodes = {
        let db_conn = pool.get()?;
        wallet::db::node::fetch_all(&db_conn)?
    };

    if nodes.is_empty() {
        println!("No nodes registered");
        return Ok(());
    }

    println!("Starting sync for {} nodes", nodes.len());

    // Process nodes sequentially to avoid Send trait issues with rusqlite::Connection
    for (node_id, node_url) in nodes {
        println!("Syncing node {} ({})", node_id, node_url);

        let (mut node_client, _) = connect_to_node(pool.clone(), node_id).await?;

        // Sync mint quotes
        if let Err(e) = sync_mint_quotes_for_node(pool.clone(), &mut node_client, node_id).await {
            eprintln!("Error syncing mint quotes for node {}: {}", node_id, e);
        }

        // Sync melt quotes
        if let Err(e) = sync_melt_quotes_for_node(pool.clone(), &mut node_client, node_id).await {
            eprintln!("Error syncing melt quotes for node {}: {}", node_id, e);
        }
    }

    println!("Sync completed for all nodes");
    Ok(())
}

async fn sync_mint_quotes_for_node(
    pool: Pool<SqliteConnectionManager>,
    node_client: &mut NodeClient<Channel>,
    node_id: u32,
) -> Result<()> {
    let pending_quotes = {
        let db_conn = pool.get()?;
        wallet::db::mint_quote::get_pendings(&db_conn)?
    };

    // Find quotes for this node
    let pending_mint_quotes = pending_quotes
        .into_iter()
        .find(|(id, _)| *id == node_id)
        .map(|(_, quotes)| quotes)
        .unwrap_or_default();

    for pending_mint_quote in pending_mint_quotes {
        let new_state = {
            let db_conn = pool.get()?;
            match wallet::mint::get_quote_state(
                &db_conn,
                node_client,
                pending_mint_quote.method,
                pending_mint_quote.id.clone(),
            )
            .await?
            {
                Some(new_state) => MintQuoteState::try_from(new_state)?,
                None => {
                    println!("Mint quote {} has expired", pending_mint_quote.id);
                    continue;
                }
            }
        };

        if pending_mint_quote.state == MintQuoteState::Unpaid && new_state == MintQuoteState::Paid {
            println!(
                "On-chain deposit received for mint quote {}",
                pending_mint_quote.id
            );

            // Redeem the quote
            if let Err(e) = wallet::mint::redeem_quote(
                pool.clone(),
                node_client,
                STARKNET_STR.to_string(),
                pending_mint_quote.id.clone(),
                node_id,
                &pending_mint_quote.unit,
                pending_mint_quote.amount,
            )
            .await
            {
                eprintln!(
                    "Failed to redeem mint quote {}: {}",
                    pending_mint_quote.id, e
                );
            } else {
                println!("Successfully redeemed mint quote {}", pending_mint_quote.id);
            }
        }
    }

    Ok(())
}

async fn sync_melt_quotes_for_node(
    pool: Pool<SqliteConnectionManager>,
    node_client: &mut NodeClient<Channel>,
    node_id: u32,
) -> Result<()> {
    let pending_quotes = {
        let db_conn = pool.get()?;
        wallet::db::melt_quote::get_pendings(&db_conn)?
    };

    // Find quotes for this node
    let pending_melt_quotes = pending_quotes
        .into_iter()
        .find(|(id, _)| *id == node_id)
        .map(|(_, quotes)| quotes)
        .unwrap_or_default();

    for pending_melt_quote in pending_melt_quotes {
        wallet::sync::melt_quote(
            pool.clone(),
            node_client,
            STARKNET_STR.to_string(),
            pending_melt_quote.id.clone(),
        )
        .await?;
    }

    Ok(())
}

pub async fn sync_melt_quote(
    pool: Pool<SqliteConnectionManager>,
    node_client: &mut NodeClient<Channel>,
    method: String,
    quote_id: String,
) -> Result<bool> {
    let melt_quote = wallet::sync::melt_quote(pool, node_client, method, quote_id.clone()).await?;

    let is_final = match melt_quote {
        Some((MeltQuoteState::Paid, tx_ids)) => {
            display_paid_melt_quote(quote_id, tx_ids);
            true
        }
        None => {
            println!("Melt quote {} has expired", quote_id);
            true
        }
        _ => false,
    };

    Ok(is_final)
}

pub fn display_paid_melt_quote(quote_id: String, tx_ids: Vec<String>) {
    println!("Melt quote {} completed successfully", quote_id);
    if !tx_ids.is_empty() {
        println!(
            "tx hashes: {}",
            format_melt_transfers_id_into_term_message(tx_ids)
        );
    }
}

async fn connect_to_node(
    pool: Pool<SqliteConnectionManager>,
    node_id: u32,
) -> Result<(NodeClient<Channel>, NodeUrl)> {
    let node_url = {
        let db_conn = pool.get()?;
        wallet::db::get_node_url(&db_conn, node_id)?
            .ok_or_else(|| anyhow!("Node {} not found", node_id))?
    };

    let node_client = wallet::connect_to_node(&node_url)
        .await
        .map_err(|e| anyhow!("Failed to connect to node {}: {}", node_url, e))?;

    Ok((node_client, node_url))
}

pub fn format_melt_transfers_id_into_term_message(transfer_ids: Vec<String>) -> String {
    let mut string_to_print = "Melt done. Withdrawal settled with tx".to_string();
    if transfer_ids.len() != 1 {
        string_to_print.push('s');
    }
    string_to_print.push_str(": ");
    let mut iterator = transfer_ids.into_iter();
    string_to_print.push_str(&iterator.next().unwrap());
    for tx_hash in iterator {
        string_to_print.push_str(", ");
        string_to_print.push_str(&tx_hash);
    }

    string_to_print
}
