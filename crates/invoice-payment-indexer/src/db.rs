use rusqlite::{Connection, Result};

#[derive(Debug)]
pub struct PaymentEvent {
    payee: String,
    asset: String,
    invoice_id: String,
    payer: String,
    amount: String,
}

#[derive(Debug)]
pub struct Block {
    pub id: String,
    pub number: u64,
}

impl From<&apibara_core::starknet::v1alpha2::BlockHeader> for Block {
    fn from(value: &apibara_core::starknet::v1alpha2::BlockHeader) -> Self {
        Self {
            id: value.block_hash.as_ref().unwrap().to_string(),
            number: value.block_number,
        }
    }
}

impl From<&apibara_core::starknet::v1alpha2::Event> for PaymentEvent {
    fn from(value: &apibara_core::starknet::v1alpha2::Event) -> Self {
        Self {
            payee: value.keys[1].to_string(),
            asset: value.keys[2].to_string(),
            invoice_id: value.data[0].to_string(),
            payer: value.data[1].to_string(),
            amount: value.data[2].to_string(),
        }
    }
}

pub fn create_tables(conn: &Connection) -> Result<()> {
    const CREATE_TABLE_BLOCK: &str = r#"
        CREATE TABLE IF NOT EXISTS block (
            id TEXT PRIMARY KEY,
            number INTEGER NOT NULL
        )"#;

    const CREATE_TABLE_PAYMENT_EVENT: &str = r#"
        CREATE TABLE IF NOT EXISTS payment_event (
            id INTEGER PRIMARY KEY,
            block_id TEXT NOT NULL REFERENCES block(id) ON DELETE CASCADE,
            payee TEXT NOT NULL,
            asset TEXT NOT NULL,
            invoice_id TEXT NOT NULL,
            payer TEXT NOT NULL,
            amount TEXT NOT NULL
        )"#;

    conn.execute(CREATE_TABLE_BLOCK, ())?;
    conn.execute(CREATE_TABLE_PAYMENT_EVENT, ())?;

    Ok(())
}

pub fn insert_new_block(conn: &Connection, block: &Block) -> Result<()> {
    const INSERT_NEW_BLOCK: &str = r#"INSERT INTO block (id, number) VALUES ($1, $2)"#;

    conn.execute(INSERT_NEW_BLOCK, (&block.id, block.number))?;

    Ok(())
}

pub fn insert_payment_event(
    conn: &Connection,
    block_id: &str,
    payment_event: PaymentEvent,
) -> Result<()> {
    const INSERT_PAYMENT_EVENT: &str = r#"
        INSERT INTO payment_event
            (block_id, payee, asset, invoice_id, payer, amount)
        VALUES
            ($1, $2, $3, $4, $5, $6)"#;

    conn.execute(
        INSERT_PAYMENT_EVENT,
        (
            &block_id,
            payment_event.payee,
            payment_event.asset,
            payment_event.invoice_id,
            payment_event.payer,
            payment_event.amount,
        ),
    )?;

    Ok(())
}

pub fn invalidate(conn: &Connection, height: u64) -> Result<()> {
    const INVALIDATE: &str = r#"DELETE * FROM payent_event WHERE number > $1"#;

    conn.execute(INVALIDATE, (height,))?;

    Ok(())
}
