use derive_more::From;
use tokio::sync::mpsc::error::SendError;

/// Result type for this module
pub type Result<A> = std::result::Result<A, Error>;

/// Error type for this module
#[derive(Debug, Clone, From)]
pub enum Error {
    ChannelClosed,
}

impl<E> From<SendError<E>> for Error {
    fn from(_: SendError<E>) -> Self {
        Error::ChannelClosed
    }
}
