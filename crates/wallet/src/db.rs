use node::MintQuoteResponse;
use rusqlite::{Connection, Result};

pub fn create_tables(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    const CREATE_TABLE_MINT_QUOTE: &str = r#"
        CREATE TABLE IF NOT EXISTS mint_quote (
            id BLOB(16) PRIMARY KEY,
            method TEXT NOT NULL,
            amount INTEGER NOT NULL,
            unit TEXT NOT NULL,
            request TEXT NOT NULL,
            state INTEGER NOT NULL CHECK (state IN (1, 2, 3)),
            expiry INTEGER NOT NULL
        )"#;

    tx.execute(CREATE_TABLE_MINT_QUOTE, ())?;

    tx.commit()?;

    Ok(())
}

pub fn store_mint_quote(
    conn: &mut Connection,
    method: String,
    amount: u64,
    unit: String,
    response: &MintQuoteResponse,
) -> Result<()> {
    const INSERT_NEW_MINT_QUOTE: &str = r#"INSERT INTO mint_quote (id, method, amount, unit, request, state, expiry) VALUES ($1, $2, $3, $4, $5, $6, $7)"#;

    conn.execute(
        INSERT_NEW_MINT_QUOTE,
        (
            &response.quote,
            method,
            amount,
            unit,
            &response.request,
            response.state,
            response.expiry,
        ),
    )?;

    Ok(())
}
pub fn set_mint_quote_state(conn: &mut Connection, quote_id: String, state: i32) -> Result<()> {
    const INSERT_NEW_MINT_QUOTE: &str = r#"
        UPDATE mint_quote_state
        SET state = $2
        WHERE id = $1
    "#;

    conn.execute(INSERT_NEW_MINT_QUOTE, (&quote_id, state))?;

    Ok(())
}
