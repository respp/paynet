use bitcoin::bip32::Xpriv;
use cashu_signer::{
    DeclareKeysetRequest, DeclareKeysetResponse, Method, SignBlindedMessagesRequest,
    SignBlindedMessagesResponse, SignerServer, VerifyProofsRequest, VerifyProofsResponse,
};
use nuts::{
    dhke::{sign_message, verify_message},
    nut01::PublicKey,
    nut02::{KeysetId, MintKeySet},
    Amount,
};
use server_errors::Error;
use state::{SharedKeySetCache, SharedRootKey};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, RwLock},
};
use tonic::{transport::Server, Request, Response, Status};

mod server_errors;
mod state;

const ROOT_KEY_ENV_VAR: &str = "ROOT_KEY";

#[derive(Debug)]
pub struct CashuSignerService {
    root_key: SharedRootKey,
    keyset_cache: SharedKeySetCache,
}

#[tonic::async_trait]
impl cashu_signer::Signer for CashuSignerService {
    async fn declare_keyset(
        &self,
        declare_keyset_request: Request<DeclareKeysetRequest>,
    ) -> Result<Response<DeclareKeysetResponse>, Status> {
        let declare_keyset_request = declare_keyset_request.get_ref();

        let method = Method::from_str(&declare_keyset_request.method).map_err(|_| {
            Status::invalid_argument(
                Error::UnknownMethod(&declare_keyset_request.method).to_string(),
            )
        })?;
        let unit = cashu_starknet::Unit::from_str(&declare_keyset_request.unit).map_err(|_| {
            Status::invalid_argument(Error::UnknownUnit(&declare_keyset_request.unit).to_string())
        })?;

        let keyset = match method {
            Method::Starknet => {
                let keyset = create_new_starknet_keyset(
                    self.root_key.clone(),
                    unit,
                    declare_keyset_request.index,
                    declare_keyset_request
                        .max_order
                        .try_into()
                        .map_err(|_| Status::invalid_argument(Error::MaxOrderTooBig.to_string()))?,
                );

                self.keyset_cache
                    .insert(keyset.id, keyset.keys.clone())
                    .map_err(|e| Status::internal(e.to_string()))?;

                keyset
            }
        };

        Ok(Response::new(DeclareKeysetResponse {
            keyset_id: keyset.id.to_bytes().to_vec(),
        }))
    }
    async fn sign_blinded_messages(
        &self,
        sign_blinded_messages_request: Request<SignBlindedMessagesRequest>,
    ) -> Result<Response<SignBlindedMessagesResponse>, Status> {
        let blinded_messages = sign_blinded_messages_request.into_inner().messages;

        let mut signatures = Vec::with_capacity(blinded_messages.len());

        let keyset_cache_read_lock = self
            .keyset_cache
            .0
            .read()
            .map_err(|_| Status::internal(Error::LockPoisoned))?;

        for blinded_message in blinded_messages {
            let keyset_id = KeysetId::from_bytes(&blinded_message.keyset_id)
                .map_err(|_| Status::invalid_argument(Error::BadKeysetId))?;
            let amount = Amount::from(blinded_message.amount);

            let key_pair = {
                let keyset = keyset_cache_read_lock
                    .get(&keyset_id)
                    .ok_or(Status::not_found(Error::KeysetNotFound(keyset_id)))?;
                keyset
                    .get(&amount)
                    .ok_or(Status::not_found(Error::AmountNotFound(amount, keyset_id)))?
            };

            let blind_secret = PublicKey::from_slice(&blinded_message.blinded_secret)
                .map_err(|_| Status::invalid_argument(Error::BadSecret))?;

            let c = sign_message(&key_pair.secret_key, &blind_secret)
                .map_err(|e| Status::internal(Error::Dhke(e)))?;

            signatures.push(c.to_bytes().to_vec());
        }

        Ok(Response::new(SignBlindedMessagesResponse { signatures }))
    }

    async fn verify_proofs(
        &self,
        verify_proofs_request: Request<VerifyProofsRequest>,
    ) -> Result<Response<VerifyProofsResponse>, Status> {
        for proof in &verify_proofs_request.get_ref().proofs {
            let keyset_id = KeysetId::from_bytes(&proof.keyset_id)
                .map_err(|_| Status::invalid_argument(Error::BadKeysetId))?;
            let amount = Amount::from(proof.amount);

            let secret_key = {
                let keyset_cache_read_lock = self
                    .keyset_cache
                    .0
                    .read()
                    .map_err(|_| Status::internal(Error::LockPoisoned))?;

                let keyset = keyset_cache_read_lock
                    .get(&keyset_id)
                    .ok_or(Status::not_found(Error::KeysetNotFound(keyset_id)))?;
                keyset
                    .get(&amount)
                    .ok_or(Status::not_found(Error::AmountNotFound(amount, keyset_id)))?
                    .secret_key
                    .clone()
            };

            let c = PublicKey::from_slice(&proof.unblind_signature)
                .map_err(|_| Status::invalid_argument(Error::BadC))?;

            if !verify_message(&secret_key, c, proof.secret.as_bytes())
                .map_err(|e| Status::internal(Error::Dhke(e)))?
            {
                return Ok(Response::new(VerifyProofsResponse { is_valid: false }));
            };
        }

        Ok(Response::new(VerifyProofsResponse { is_valid: true }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:10000".parse().unwrap();
    let root_private_key = {
        let root_key_env_var: String =
            std::env::var(ROOT_KEY_ENV_VAR).expect("env var `ROOT_KEY` should be set");
        Xpriv::from_str(&root_key_env_var)
            .expect("content of `ROOT_KEY` env var should be a valid private key")
    };

    let route_guide = CashuSignerService {
        root_key: SharedRootKey(Arc::new(root_private_key)),
        keyset_cache: SharedKeySetCache(Arc::new(RwLock::new(HashMap::new()))),
    };

    let svc = SignerServer::new(route_guide);

    Server::builder().add_service(svc).serve(addr).await?;

    Ok(())
}

fn create_new_starknet_keyset(
    root_key: SharedRootKey,
    unit: cashu_starknet::Unit,
    index: u32,
    max_order: u8,
) -> MintKeySet<cashu_starknet::Unit> {
    root_key.generate_keyset(unit, index, max_order)
}
