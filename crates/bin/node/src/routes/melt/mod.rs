mod errors;
mod starknet;
use cashu_starknet::{MeltPaymentRequest, Unit};
use nuts::nut00::Proof;
use nuts::nut05::MeltQuoteResponse;
use uuid::Uuid;

use crate::{grpc_service::GrpcState, methods::Method};

use errors::Error;

use starknet::starknet_melt;

impl GrpcState {
    pub async fn inner_melt(
        &self,
        method: Method,
        unit: Unit,
        melt_payment_request: MeltPaymentRequest,
        inputs: &[Proof],
    ) -> Result<MeltQuoteResponse<Uuid>, Error> {
        // Release the lock asap
        let settings = {
            let read_nuts_settings_lock = self.nuts.read().await;

            if read_nuts_settings_lock.nut05.disabled {
                Err(Error::MeltDisabled)?;
            }

            read_nuts_settings_lock
                .nut05
                .get_settings(method, unit)
                .ok_or(Error::UnitNotSupported(unit, method))?
        };

        let response = match method {
            Method::Starknet => {
                starknet_melt(
                    self.pg_pool.clone(),
                    self.signer.clone(),
                    self.keyset_cache.clone(),
                    settings,
                    self.quote_ttl.melt_ttl(),
                    melt_payment_request,
                    inputs,
                )
                .await?
            }
        };

        Ok(response)
    }
}
