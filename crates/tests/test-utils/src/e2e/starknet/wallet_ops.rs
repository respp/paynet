use anyhow::anyhow;
use itertools::Itertools;
use node_client::NodeClient;
use nuts::Amount;
use primitive_types::U256;
use r2d2_sqlite::SqliteConnectionManager;
use starknet_types::{Asset, STARKNET_STR, Unit};
use starknet_types_core::felt::Felt;
use tonic::transport::Channel;
use wallet::{
    self,
    types::{
        NodeUrl,
        compact_wad::{CompactKeysetProofs, CompactProof, CompactWad},
    },
};

use crate::common::{
    error::{Error, Result},
    utils::{EnvVariables, starknet::pay_invoices},
};

pub struct WalletOps {
    db_pool: r2d2::Pool<SqliteConnectionManager>,
    node_id: u32,
    node_client: NodeClient<Channel>,
}

impl WalletOps {
    pub fn new(
        db_pool: r2d2::Pool<SqliteConnectionManager>,
        node_id: u32,
        node_client: NodeClient<Channel>,
    ) -> Self {
        WalletOps {
            db_pool,
            node_id,
            node_client,
        }
    }

    pub async fn mint(&mut self, amount: U256, asset: Asset, env: EnvVariables) -> Result<()> {
        let amount = amount
            .checked_mul(asset.scale_factor())
            .ok_or(anyhow!("amount too big"))?;
        let (amount, unit, _remainder) = asset
            .convert_to_amount_and_unit(amount)
            .map_err(|e| Error::Other(e.into()))?;

        let quote = wallet::mint::create_quote(
            self.db_pool.clone(),
            &mut self.node_client,
            self.node_id,
            STARKNET_STR.to_string(),
            amount,
            unit,
        )
        .await?;

        let calls: [starknet_types::Call; 2] = serde_json::from_str(&quote.request)?;
        pay_invoices(calls.to_vec(), env).await?;

        match wallet::mint::wait_for_quote_payment(
            &*self.db_pool.get()?,
            &mut self.node_client,
            STARKNET_STR.to_string(),
            quote.quote.clone(),
        )
        .await?
        {
            wallet::mint::QuotePaymentIssue::Expired => {
                println!("quote {} has expired", quote.quote);
                return Ok(());
            }
            wallet::mint::QuotePaymentIssue::Paid => {}
        }

        wallet::mint::redeem_quote(
            self.db_pool.clone(),
            &mut self.node_client,
            STARKNET_STR.to_string(),
            quote.quote,
            self.node_id,
            unit.as_str(),
            amount,
        )
        .await?;

        Ok(())
    }

    pub async fn send(
        &mut self,
        node_url: NodeUrl,
        amount: U256,
        asset: Asset,
        memo: Option<String>,
    ) -> Result<CompactWad<Unit>> {
        let amount = amount
            .checked_mul(asset.scale_factor())
            .ok_or(anyhow!("amount too big"))?;
        let (amount, unit, _) = asset
            .convert_to_amount_and_unit(amount)
            .map_err(|e| Error::Other(e.into()))?;
        let proofs_ids = wallet::fetch_inputs_ids_from_db_or_node(
            self.db_pool.clone(),
            &mut self.node_client,
            self.node_id,
            amount,
            unit,
        )
        .await?
        .ok_or(anyhow!("not enough funds"))?;

        let proofs = wallet::load_tokens_from_db(&*self.db_pool.get()?, &proofs_ids)?;
        let compact_proofs = proofs
            .into_iter()
            .chunk_by(|p| p.keyset_id)
            .into_iter()
            .map(|(keyset_id, proofs)| CompactKeysetProofs {
                keyset_id,
                proofs: proofs
                    .map(|p| CompactProof {
                        amount: p.amount,
                        secret: p.secret,
                        c: p.c,
                    })
                    .collect(),
            })
            .collect();

        Ok(CompactWad {
            node_url,
            unit,
            memo,
            proofs: compact_proofs,
        })
    }

    pub async fn receive(&mut self, wad: &CompactWad<Unit>) -> Result<()> {
        wallet::receive_wad::<Unit>(
            self.db_pool.clone(),
            &mut self.node_client,
            self.node_id,
            wad,
        )
        .await?;
        Ok(())
    }

    pub async fn melt(&mut self, amount: U256, asset: Asset, to: String) -> Result<()> {
        let method = STARKNET_STR.to_string();
        let payee_address = Felt::from_hex(&to).map_err(|e| Error::Other(e.into()))?;
        if !starknet_types::is_valid_starknet_address(&payee_address) {
            return Err(Error::Other(anyhow!(
                "Invalid starknet address: {}",
                payee_address
            )));
        }

        let amount = amount
            .checked_mul(asset.scale_factor())
            .ok_or(anyhow!("amount too big"))?;
        let request = serde_json::to_string(&starknet_liquidity_source::MeltPaymentRequest {
            payee: payee_address,
            asset: starknet_types::Asset::Strk,
            amount: amount.into(),
        })?;

        let unit = asset.find_best_unit();

        let melt_quote_response = wallet::melt::create_quote(
            self.db_pool.clone(),
            &mut self.node_client,
            self.node_id,
            method.clone(),
            unit,
            request,
        )
        .await?;

        let _melt_response = wallet::melt::pay_quote(
            self.db_pool.clone(),
            &mut self.node_client,
            self.node_id,
            melt_quote_response.quote.clone(),
            Amount::from(melt_quote_response.amount),
            method.clone(),
            unit,
        )
        .await?;

        if wallet::melt::wait_for_payment(
            self.db_pool.clone(),
            &mut self.node_client,
            method,
            melt_quote_response.quote,
        )
        .await?
        .is_none()
        {
            panic!("quote expired")
        }

        Ok(())
    }

    pub async fn sync_single_wad(
        pool: r2d2::Pool<SqliteConnectionManager>,
        wad_record: &wallet::db::wad::WadRecord,
    ) -> Result<Option<wallet::db::wad::WadStatus>> {
        use node_client::{CheckStateRequest, ProofState};

        // Get proof public keys for this WAD
        let proof_ys = {
            let db_conn = pool.get()?;
            wallet::db::wad::get_wad_proofs(&db_conn, wad_record.id)?
        };

        if proof_ys.is_empty() {
            log::warn!(
                "Empty WAD found (ID: {}), this should not occur",
                wad_record.id
            );
            return Ok(None);
        }

        // Parse the WAD data to get node information
        let compact_wad: wallet::types::compact_wad::CompactWad<starknet_types::Unit> =
            serde_json::from_str(&wad_record.wad_data)?;

        // Connect to the node
        let mut node_client = wallet::connect_to_node(&compact_wad.node_url).await?;

        // Check proof states using NUT-07
        let check_request = CheckStateRequest {
            ys: proof_ys.iter().map(|y| y.to_bytes().to_vec()).collect(),
        };

        let response = node_client.check_state(check_request).await?;
        let states = response.into_inner().states;

        // Analyze proof states to determine WAD status
        let mut all_spent = true;
        let mut any_spent = false;

        for state in states {
            match ProofState::try_from(state.state)? {
                ProofState::PsUnspent | ProofState::PsPending => {
                    all_spent = false;
                }
                ProofState::PsSpent => {
                    any_spent = true;
                }
                _ => {
                    return Err(anyhow!(
                        "Unexpected proof state encountered for WAD {}: {:?}",
                        wad_record.id,
                        state.state
                    )
                    .into());
                }
            }
        }

        // Determine new status based on proof states
        let new_status = match wad_record.wad_type {
            wallet::db::wad::WadType::OUT => {
                // For outgoing WADs, finished when all proofs are spent
                if all_spent {
                    log::info!(
                        "WAD {} all proofs spent, marking as Finished",
                        wad_record.id
                    );
                    Some(wallet::db::wad::WadStatus::Finished)
                } else if any_spent {
                    log::info!(
                        "WAD {} some proofs spent, marking as Partial",
                        wad_record.id
                    );
                    Some(wallet::db::wad::WadStatus::Partial)
                } else {
                    log::info!("WAD {} no proofs spent yet", wad_record.id);
                    None
                }
            }
            wallet::db::wad::WadType::IN => {
                // For incoming WADs, finished when all proofs are received (spent in our wallet)
                if all_spent {
                    log::info!(
                        "WAD {} all proofs received, marking as Finished",
                        wad_record.id
                    );
                    Some(wallet::db::wad::WadStatus::Finished)
                } else if any_spent {
                    log::info!(
                        "WAD {} some proofs received, marking as Partial",
                        wad_record.id
                    );
                    Some(wallet::db::wad::WadStatus::Partial)
                } else {
                    log::info!("WAD {} not all proofs received yet", wad_record.id);
                    None
                }
            }
        };

        if let Some(status) = new_status {
            let db_conn = pool.get()?;
            wallet::db::wad::update_wad_status(&db_conn, wad_record.id, status)?;
        }

        Ok(new_status)
    }
}
