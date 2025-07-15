use rusqlite::{Connection, Result, params};
use nuts::nut01::PublicKey;
use crate::types::compact_wad::CompactWad;
use nuts::traits::Unit;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum WadType {
    Incoming,
    Outgoing,
}

impl std::fmt::Display for WadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WadType::Incoming => write!(f, "incoming"),
            WadType::Outgoing => write!(f, "outgoing"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum WadStatus {
    Pending,
    Cancelled,
    Finished,
    Failed,
}

impl std::fmt::Display for WadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WadStatus::Pending => write!(f, "Pending"),
            WadStatus::Cancelled => write!(f, "Cancelled"),
            WadStatus::Finished => write!(f, "Finished"),
            WadStatus::Failed => write!(f, "Failed"),
        }
    }
}

impl std::str::FromStr for WadStatus {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(WadStatus::Pending),
            "Cancelled" => Ok(WadStatus::Cancelled),
            "Finished" => Ok(WadStatus::Finished),
            "Failed" => Ok(WadStatus::Failed),
            _ => Err(format!("Invalid WadStatus: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WadRecord {
    pub uuid: String,
    pub wad_type: WadType,
    pub status: WadStatus,
    pub wad_data: String, // JSON string of CompactWad
    pub total_amount_json: String, // JSON array of unit/amount pairs
    pub memo: Option<String>,
    pub created_at: u64,
    pub modified_at: u64,
}

pub fn insert_wad<U: Unit + serde::Serialize>(
    conn: &Connection,
    uuid: &str,
    wad_type: WadType,
    wad: &CompactWad<U>,
    proof_ys: &[PublicKey],
) -> Result<()> {
    let wad_data = serde_json::to_string(wad)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    
    let total_amount = wad.value()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    
    let total_amount_json = serde_json::to_string(&[(wad.unit.to_string(), total_amount)])
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    const INSERT_WAD: &str = r#"
        INSERT INTO wad 
            (uuid, type, status, wad_data, total_amount_json, memo, created_at, modified_at)
        VALUES 
            (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
    "#;

    conn.execute(
        INSERT_WAD,
        params![
            uuid,
            wad_type.to_string(),
            WadStatus::Pending.to_string(),
            wad_data,
            total_amount_json,
            wad.memo,
            now,
            now,
        ],
    )?;

    // Insert WAD-proof relationships
    const INSERT_WAD_PROOF: &str = r#"
        INSERT INTO wad_proof (wad_uuid, proof_y)
        VALUES (?1, ?2)
    "#;

    for proof_y in proof_ys {
        conn.execute(INSERT_WAD_PROOF, params![uuid, proof_y])?;
    }

    Ok(())
}

fn parse_wad_record(row: &rusqlite::Row) -> rusqlite::Result<WadRecord> {
    let status_str: String = row.get(2)?;
    let status = status_str.parse()
        .map_err(|e: String| rusqlite::Error::FromSqlConversionFailure(
            2, 
            rusqlite::types::Type::Text, 
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        ))?;
    
    let wad_type = match row.get::<_, String>(1)?.as_str() {
        "incoming" => WadType::Incoming,
        "outgoing" => WadType::Outgoing,
        _ => return Err(rusqlite::Error::FromSqlConversionFailure(
            1, 
            rusqlite::types::Type::Text, 
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid wad type"))
        )),
    };

    Ok(WadRecord {
        uuid: row.get(0)?,
        wad_type,
        status,
        wad_data: row.get(3)?,
        total_amount_json: row.get(4)?,
        memo: row.get(5)?,
        created_at: row.get(6)?,
        modified_at: row.get(7)?,
    })
}

pub fn get_recent_wads(conn: &Connection, limit: u32) -> Result<Vec<WadRecord>> {
    const GET_RECENT_WADS: &str = r#"
        SELECT uuid, type, status, wad_data, total_amount_json, memo, created_at, modified_at
        FROM wad 
        ORDER BY created_at DESC 
        LIMIT ?1
    "#;

    let mut stmt = conn.prepare(GET_RECENT_WADS)?;
    let rows = stmt.query_map([limit], parse_wad_record)?;

    rows.collect()
}

pub fn update_wad_status(conn: &Connection, uuid: &str, status: WadStatus) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    const UPDATE_WAD_STATUS: &str = r#"
        UPDATE wad 
        SET status = ?2, modified_at = ?3 
        WHERE uuid = ?1
    "#;

    conn.execute(UPDATE_WAD_STATUS, params![uuid, status.to_string(), now])?;

    Ok(())
}

pub fn get_pending_wads(conn: &Connection) -> Result<Vec<WadRecord>> {
    const GET_PENDING_WADS: &str = r#"
        SELECT uuid, type, status, wad_data, total_amount_json, memo, created_at, modified_at
        FROM wad 
        WHERE status = 'Pending'
        ORDER BY created_at ASC
    "#;

    let mut stmt = conn.prepare(GET_PENDING_WADS)?;
    let rows = stmt.query_map([], parse_wad_record)?;

    rows.collect()
}

pub fn get_wad_proofs(conn: &Connection, wad_uuid: &str) -> Result<Vec<PublicKey>> {
    const GET_WAD_PROOFS: &str = r#"
        SELECT proof_y FROM wad_proof WHERE wad_uuid = ?1
    "#;

    let mut stmt = conn.prepare(GET_WAD_PROOFS)?;
    let rows = stmt.query_map([wad_uuid], |row| {
        let y_bytes: Vec<u8> = row.get(0)?;
        PublicKey::from_slice(&y_bytes)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))
    })?;

    let mut proof_ys = Vec::new();
    for row in rows {
        proof_ys.push(row?);
    }

    Ok(proof_ys)
} 