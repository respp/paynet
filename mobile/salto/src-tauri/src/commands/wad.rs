use std::{cmp::Ordering, str::FromStr};

use nuts::Amount;
use starknet_types::{Asset, AssetFromStrError, AssetToUnitConversionError, Unit};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
use wallet::types::compact_wad::{self, CompactWads};

use crate::{
    AppState,
    parse_asset_amount::{ParseAmountStringError, parse_asset_amount},
};

use super::BalanceChange;

#[derive(Debug, thiserror::Error)]
pub enum CreateWadsError {
    #[error(transparent)]
    R2D2(#[from] r2d2::Error),
    #[error(transparent)]
    Rusqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Wallet(#[from] wallet::errors::Error),
    #[error(transparent)]
    Asset(#[from] AssetFromStrError),
    #[error("invalid amount: {0}")]
    Amount(#[from] ParseAmountStringError),
    #[error(transparent)]
    AssetToUnitConversion(#[from] AssetToUnitConversionError),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error("not enought funds, asked {0}, missing {1}")]
    NotEnoughFunds(Amount, Amount),
    #[error("not enought funds in node {0}")]
    NotEnoughFundsInNode(u32),
    #[error(transparent)]
    NodeConnect(#[from] wallet::ConnectToNodeError),
}

impl serde::Serialize for CreateWadsError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[tauri::command]
pub async fn create_wads(
    app: AppHandle,
    state: State<'_, AppState>,
    amount: String,
    asset: String,
) -> Result<String, CreateWadsError> {
    let asset = Asset::from_str(&asset)?;
    let unit = asset.find_best_unit();
    let amount = parse_asset_amount(&amount, asset, unit)?;

    let amount_to_use_per_node = {
        let db_conn = state.pool.get()?;
        let balances = wallet::db::balance::get_for_all_nodes_by_unit(&db_conn, unit)?;

        let mut used_node = vec![];
        let mut rem_amount = amount;
        for balance in balances {
            match rem_amount.cmp(&balance.amount) {
                Ordering::Less | Ordering::Equal => {
                    used_node.push((balance.id, balance.url, rem_amount));
                    rem_amount = Amount::ZERO;
                    break;
                }
                Ordering::Greater => {
                    rem_amount -= balance.amount;
                    used_node.push((balance.id, balance.url, balance.amount));
                }
            }
        }

        if rem_amount != Amount::ZERO {
            return Err(CreateWadsError::NotEnoughFunds(amount, rem_amount));
        }

        used_node
    };

    let mut wads = Vec::with_capacity(amount_to_use_per_node.len());
    let mut balance_decrease_events = Vec::with_capacity(amount_to_use_per_node.len());
    for (node_id, node_url, amount_to_use) in amount_to_use_per_node {
        let mut node_client = wallet::connect_to_node(&node_url).await?;

        let proofs_ids = wallet::fetch_inputs_ids_from_db_or_node(
            state.pool.clone(),
            &mut node_client,
            node_id,
            amount_to_use,
            unit,
        )
        .await?
        .ok_or(CreateWadsError::NotEnoughFundsInNode(node_id))?;

        let db_conn = state.pool.get()?;
        let proofs = wallet::load_tokens_from_db(&db_conn, &proofs_ids)?;
        let wad = wallet::create_wad_from_proofs(node_url, unit, None, proofs, state.pool.clone())
            .await?;
        wads.push(wad);
        balance_decrease_events.push(BalanceChange {
            node_id,
            unit: unit.as_str().to_string(),
            amount: amount_to_use.into(),
        });
    }
    for event in balance_decrease_events {
        app.emit("balance-decrease", event)?;
    }

    Ok(CompactWads(wads).to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum ReceiveWadsError {
    #[error(transparent)]
    R2D2(#[from] r2d2::Error),
    #[error(transparent)]
    Rusqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Wallet(#[from] wallet::errors::Error),
    #[error(transparent)]
    Asset(#[from] AssetFromStrError),
    #[error("invalid amount: {0}")]
    Amount(#[from] ParseAmountStringError),
    #[error(transparent)]
    AssetToUnitConversion(#[from] AssetToUnitConversionError),
    #[error("invalid string for compacted wad")]
    WadString(#[from] compact_wad::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error("this is a json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    RegisterNode(#[from] wallet::RegisterNodeError),
}

impl serde::Serialize for ReceiveWadsError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[tauri::command]
pub async fn receive_wads(
    app: AppHandle,
    state: State<'_, AppState>,
    wads: String,
) -> Result<(), ReceiveWadsError> {
    let wads: CompactWads<Unit> = wads.parse()?;

    for wad in wads.0 {
        let (mut node_client, node_id) =
            wallet::register_node(state.pool.clone(), &wad.node_url).await?;

        let amount_received =
            wallet::receive_wad(state.pool.clone(), &mut node_client, node_id, &wad).await?;

        app.emit(
            "balance-increase",
            BalanceChange {
                node_id,
                unit: wad.unit.as_str().to_string(),
                amount: amount_received.into(),
            },
        )?;
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WadHistoryItem {
    pub id: String,
    pub wad_type: String,
    pub status: String,
    pub total_amount_json: String,
    pub memo: Option<String>,
    pub created_at: u64,
    pub modified_at: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum GetWadHistoryError {
    #[error(transparent)]
    R2D2(#[from] r2d2::Error),
    #[error(transparent)]
    Rusqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

impl serde::Serialize for GetWadHistoryError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[tauri::command]
pub async fn get_wad_history(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<WadHistoryItem>, GetWadHistoryError> {
    let db_conn = state.pool.get()?;
    let wad_records = wallet::db::wad::get_recent_wads(&db_conn, limit.unwrap_or(20))?;

    let mut history_items = Vec::with_capacity(wad_records.len());
    for record in wad_records {
        let total_amount_json = serde_json::to_string(
            &wallet::db::wad::get_amounts_by_id::<Unit>(&db_conn, record.id)?,
        )?;

        history_items.push(WadHistoryItem {
            id: record.id.to_string(),
            wad_type: record.wad_type.to_string(),
            status: record.status.to_string(),
            total_amount_json,
            memo: record.memo,
            created_at: record.created_at,
            modified_at: record.modified_at,
        })
    }

    Ok(history_items)
}

#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WadStatusUpdate {
    pub wad_id: Uuid,
    pub new_status: String,
}

#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncError {
    pub wad_id: Uuid,
    pub error: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncWadsError {
    #[error(transparent)]
    R2D2(#[from] r2d2::Error),
    #[error(transparent)]
    Rusqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Wallet(#[from] wallet::errors::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl serde::Serialize for SyncWadsError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[tauri::command]
pub async fn sync_wads(app: AppHandle, state: State<'_, AppState>) -> Result<(), SyncWadsError> {
    // Use the lib wallet function instead of duplicating code
    let wad_results = wallet::sync::sync_pending_wads(state.pool.clone()).await?;

    // Emit events for UI updates
    for result in wad_results {
        match result.result {
            Ok(Some(status)) => {
                // Emit status update event
                app.emit(
                    "wad-status-updated",
                    WadStatusUpdate {
                        wad_id: result.wad_id,
                        new_status: status.to_string(),
                    },
                )?;
            }
            Ok(None) => {
                // No status change, no event needed
            }
            Err(e) => {
                // Emit error event
                app.emit(
                    "sync-error",
                    SyncError {
                        wad_id: result.wad_id,
                        error: e.to_string(),
                    },
                )?;
            }
        }
    }

    Ok(())
}
