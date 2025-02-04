use apibara_core::node::v1alpha2::DataFinality;
use apibara_core::starknet::v1alpha2::{Block, FieldElement, Filter, HeaderFilter};
use apibara_sdk::{ClientBuilder, Configuration, Uri};
use futures::TryStreamExt;
use rusqlite::Connection;
use starknet_core::types::Felt;
use thiserror::Error;

mod db;

const INVOICE_PAYMENT_CONTRACT_ADDRESS: &str =
    "0x03a94f47433e77630f288054330fb41377ffcc49dacf56568eeba84b017aa633";
const REMITTANCE_EVENT_KEY: &str =
    "0x027a12f554d018764f982295090da45b4ff0734785be0982b62c329b9ac38033";

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid value for field element: {0}")]
    InvalidFieldElement(String),
    #[error("DNA client error")]
    ApibaraClient,
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
}

pub async fn init_apibara_stream(
    apibara_bearer_token: String,
    target_asset_and_recipient_pairs: Vec<(Felt, Felt)>,
) -> Result<apibara_sdk::ImmutableDataStream<Block>, Error> {
    let config = Configuration::<Filter>::default()
        .with_starting_block(458_645)
        .with_finality(DataFinality::DataStatusAccepted)
        .with_filter(|mut filter| {
            let invoice_payment_contract_address =
                FieldElement::from_hex(INVOICE_PAYMENT_CONTRACT_ADDRESS).unwrap();
            let remittance_event_key = FieldElement::from_hex(REMITTANCE_EVENT_KEY).unwrap();

            target_asset_and_recipient_pairs
                .iter()
                .for_each(|(recipient, asset)| {
                    filter
                        .with_header(HeaderFilter::weak())
                        .add_event(|event| {
                            event
                                .with_from_address(invoice_payment_contract_address.clone())
                                .with_keys(vec![
                                    remittance_event_key.clone(),
                                    FieldElement::from_hex(&recipient.to_hex_string()).unwrap(),
                                    FieldElement::from_hex(&asset.to_hex_string()).unwrap(),
                                ])
                        })
                        .build();
                });

            filter
        });

    let uri = Uri::from_static("https://sepolia.starknet.a5a.ch");
    let stream = ClientBuilder::default()
        .with_bearer_token(Some(apibara_bearer_token))
        .connect(uri)
        .await
        .map_err(|_| Error::ApibaraClient)?
        .start_stream_immutable::<Filter, Block>(config)
        .await
        .map_err(|_| Error::ApibaraClient)?;

    Ok(stream)
}

pub async fn index_stream(
    conn: Connection,
    mut stream: apibara_sdk::ImmutableDataStream<Block>,
) -> Result<(), Error> {
    db::create_tables(&conn)?;

    while let Some(response) = stream.try_next().await.unwrap() {
        match &response {
            apibara_sdk::DataMessage::Data {
                cursor: _,
                end_cursor: _,
                finality: _,
                batch,
            } => {
                for block in batch.iter() {
                    let block_infos: db::Block = block.header.as_ref().unwrap().into();
                    db::insert_new_block(&conn, &block_infos)?;

                    for event in block.events.iter() {
                        db::insert_payment_event(
                            &conn,
                            &block_infos.id,
                            event.event.as_ref().unwrap().into(),
                        )?;
                    }
                }
            }
            apibara_sdk::DataMessage::Invalidate { cursor } => {
                db::invalidate(&conn, cursor.as_ref().unwrap().order_key)?
            }
            apibara_sdk::DataMessage::Heartbeat => {}
        }
    }

    Ok(())
}
