use derive_more::From;

/// Result type for this module
pub type Result<A> = core::result::Result<A, Error>;

/// Error type for this module
#[derive(Debug, From)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Serde(serde_json::Error),
}

impl From<Error> for edfsm_machine::error::Error {
    fn from(_value: Error) -> Self {
        edfsm_machine::error::Error::ChannelClosed
    }
}
