use dotenv::dotenv;
use invoice_payment_indexer::{index_stream, init_apibara_stream};
use starknet_types_core::felt::Felt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let dna_token = dotenv::var("APIBARA_TOKEN").expect("missing `APIBARA_TOKEN` env variable");

    let starknet_token_address = Felt::from_hex_unchecked(
        "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d",
    );
    let our_recipient_account = Felt::from_hex_unchecked(
        "0x07487f6e8fc8c60049e82cf8b6593211aeefef7efd0021db585c7e78cc29ac9a",
    );

    let stream = init_apibara_stream(
        dna_token,
        vec![(our_recipient_account, starknet_token_address)],
    )
    .await?;

    let conn = rusqlite::Connection::open_in_memory()?;
    index_stream(conn, stream).await?;

    Ok(())
}
