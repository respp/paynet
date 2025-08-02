use node_client::NodeClient;
use nuts::nut05::MeltQuoteState;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use tonic::transport::Channel;
use uuid::Uuid;

use crate::{
    PreMint, SplitTarget, acknowledge, build_outputs_from_premints, db, errors::Error,
    get_active_keyset_for_unit, hash_swap_request, store_new_tokens, types::ProofState,
};
use nuts::{Amount, nut01::PublicKey};

pub async fn melt_quote(
    pool: Pool<SqliteConnectionManager>,
    node_client: &mut NodeClient<Channel>,
    method: String,
    quote_id: String,
) -> Result<Option<(MeltQuoteState, Vec<String>)>, Error> {
    let response = node_client
        .melt_quote_state(node_client::MeltQuoteStateRequest {
            method,
            quote: quote_id.clone(),
        })
        .await;

    match response {
        Err(status) if status.code() == tonic::Code::DeadlineExceeded => {
            let db_conn = pool.get()?;
            db::melt_quote::delete(&db_conn, &quote_id)?;
            Ok(None)
        }
        Ok(response) => {
            let response = response.into_inner();
            let state =
                MeltQuoteState::try_from(node_client::MeltQuoteState::try_from(response.state)?)?;

            let mut db_conn = pool.get()?;
            let tx = db_conn.transaction()?;
            match state {
                MeltQuoteState::Unpaid => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    if now >= response.expiry {
                        db::melt_quote::delete(&tx, &quote_id)?;
                        return Ok(None);
                    }
                }
                MeltQuoteState::Pending => {}
                MeltQuoteState::Paid => {
                    if !response.transfer_ids.is_empty() {
                        let transfer_ids_to_store = serde_json::to_string(&response.transfer_ids)?;
                        db::melt_quote::register_transfer_ids(
                            &tx,
                            &quote_id,
                            &transfer_ids_to_store,
                        )?;
                    }
                }
            }

            db::melt_quote::update_state(&tx, &quote_id, response.state)?;
            tx.commit()?;

            Ok(Some((state, response.transfer_ids)))
        }
        Err(e) => Err(e)?,
    }
}

pub async fn sync_pending_wads(
    pool: Pool<SqliteConnectionManager>,
) -> Result<Vec<WadSyncResult>, Error> {
    let pending_wads = {
        let db_conn = pool.get()?;
        db::wad::get_pending_wads(&db_conn)?
    };

    let mut results = Vec::with_capacity(pending_wads.len());

    for wad_id in pending_wads {
        let result = sync_single_wad(pool.clone(), wad_id).await;
        results.push(WadSyncResult {
            wad_id,
            result: result.map_err(|e| e.to_string()),
        });
    }

    Ok(results)
}

pub async fn sync_single_wad(
    pool: Pool<SqliteConnectionManager>,
    wad_id: Uuid,
) -> Result<Option<db::wad::WadStatus>, Error> {
    use node_client::{CheckStateRequest, ProofState};

    let proof_ys = {
        let db_conn = pool.get()?;
        db::wad::get_wad_proofs(&db_conn, wad_id)?
    };

    if proof_ys.is_empty() {
        return Ok(None);
    }

    let compact_wad: crate::types::compact_wad::CompactWad<starknet_types::Unit> =
        serde_json::from_str(&wad_id.wad_data)?;

    let mut node_client = crate::connect_to_node(&compact_wad.node_url).await?;

    let check_request = CheckStateRequest {
        ys: proof_ys.iter().map(|y| y.to_bytes().to_vec()).collect(),
    };

    let response = node_client.check_state(check_request).await?;
    let states = response.into_inner().states;

    let all_spent = states
        .iter()
        .all(|state| match ProofState::try_from(state.state) {
            Ok(ProofState::PsSpent) => true,
            Ok(ProofState::PsUnspent | ProofState::PsPending) => false,
            Ok(_unexpected_state) => false,
            Err(_) => false,
        });

    for state in &states {
        ProofState::try_from(state.state).map_err(|_| {
            Error::UnexpectedProofState(format!(
                "Invalid proof state encountered for WAD {}: {:?}",
                wad_id.id, state.state
            ))
        })?;
    }

    let new_status = match wad_id.wad_type {
        db::wad::WadType::OUT => {
            if all_spent {
                Some(db::wad::WadStatus::Finished)
            } else {
                match spend_out_wad_proofs(pool.clone(), &mut node_client, &compact_wad, &proof_ys)
                    .await
                {
                    Ok(()) => Some(db::wad::WadStatus::Finished),
                    Err(_) => None,
                }
            }
        }
        db::wad::WadType::IN => {
            if all_spent {
                Some(db::wad::WadStatus::Finished)
            } else {
                None
            }
        }
    };

    if let Some(status) = &new_status {
        let db_conn = pool.get()?;
        db::wad::update_wad_status(&db_conn, wad_id.id, *status)?;
    }

    Ok(new_status)
}

#[derive(Debug, Clone)]
pub struct WadSyncResult {
    pub wad_id: Uuid,
    pub result: Result<Option<db::wad::WadStatus>, String>,
}
