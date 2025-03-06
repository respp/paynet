#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("a Tauri state mutex has been poisoned")]
    StateMutexPoisoned,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
